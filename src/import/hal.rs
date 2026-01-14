use crate::{
    dmatex::Dmatex,
    format_mapping::{
        drm_fourcc_to_vk_format, get_drm_image_modifier_info, get_drm_modifiers, vk_format_to_srgb,
        vulkan_to_wgpu,
    },
    import::{
        get_imported_descriptor, DmaImage, DmatexUsage, DropCallback, ImageQueueTransfer, ImportError,
        ImportedDmatexs, ImportedTexture,
    }
};
use std::os::fd::IntoRawFd as _;

use ash::vk::{
    self, CommandBufferBeginInfo, DeviceMemory, FormatFeatureFlags2,
    ImagePlaneMemoryRequirementsInfo, MemoryDedicatedRequirements, MemoryRequirements2,
    SubresourceLayout,
};
use bevy::render::{
    render_resource::Texture,
    renderer::RenderDevice,
};
use drm_fourcc::DrmFourcc;
use tracing::{debug_span, error};
use wgpu::{
    hal::{api::Vulkan, vulkan::Device as VkDevice, MemoryFlags, TextureDescriptor}, TextureUses,
    TextureViewDescriptor,
};

pub fn memory_barrier(
    device: &RenderDevice,
    dmatexs: &ImportedDmatexs,
    queue_transfer_direction: ImageQueueTransfer,
) {
    unsafe {
        #[allow(clippy::unwrap_used)] // Validation
        let dev = device.wgpu_device().as_hal::<Vulkan>().unwrap();
        let vk_dev = dev.raw_device();
        let Ok(command_pool) = vk_dev
            .create_command_pool(
                &vk::CommandPoolCreateInfo {
                    flags: vk::CommandPoolCreateFlags::TRANSIENT,
                    queue_family_index: dev.queue_family_index(),
                    ..Default::default()
                },
                None,
            )
            .inspect_err(|e| error!("Unable to create command pool: {e}"))
        else {
            return;
        };

        let Ok(Some(buffer)) = dev
            .raw_device()
            .allocate_command_buffers(&vk::CommandBufferAllocateInfo {
                command_pool,
                level: vk::CommandBufferLevel::PRIMARY,
                command_buffer_count: 1,
                ..Default::default()
            })
            .inspect_err(|e| error!("Unable to allocate command buffer: {e}"))
            .map(|v| v.into_iter().next())
        else {
            vk_dev.destroy_command_pool(command_pool, None);
            return;
        };
        let Ok(texes) = dmatexs
            .0
            .lock()
            .inspect_err(|e| error!("Unable to lock dmatexs: {e}"))
        else {
            vk_dev.destroy_command_pool(command_pool, None);
            return;
        };

        if vk_dev
            .begin_command_buffer(
                buffer,
                &CommandBufferBeginInfo {
                    flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
                    ..Default::default()
                },
            )
            .inspect_err(|err| error!("failed to begin command buffer: {err}"))
            .is_err()
        {
            vk_dev.destroy_command_pool(command_pool, None);
            return;
        }

        let vk_submit_span = debug_span!("VK dmatex image acquire").entered();
        for image in texes
            .iter()
            .filter_map(|v| match v.1 {
                DmaImage::UnImported(_, _, _) => None,
                DmaImage::Imported(imported_texture) => Some(imported_texture),
            })
            .filter_map(|imported| {
                imported
                    .texture
                    .as_hal::<Vulkan>()
                    .map(|vk_texture| vk_texture.raw_handle())
            })
        {
            vk_dev.cmd_pipeline_barrier(
                buffer,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier {
                    src_access_mask: vk::AccessFlags::NONE,
                    dst_access_mask: vk::AccessFlags::NONE,
                    old_layout: vk::ImageLayout::GENERAL,
                    new_layout: vk::ImageLayout::GENERAL,
                    // TODO: might want to use vk::QUEUE_FAMILY_FOREIGN_EXT instead
                    src_queue_family_index: match queue_transfer_direction {
                        ImageQueueTransfer::Acquire => vk::QUEUE_FAMILY_EXTERNAL,
                        ImageQueueTransfer::Release => dev.queue_family_index(),
                    },
                    dst_queue_family_index: match queue_transfer_direction {
                        ImageQueueTransfer::Acquire => dev.queue_family_index(),
                        ImageQueueTransfer::Release => vk::QUEUE_FAMILY_EXTERNAL,
                    },
                    image,
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                    ..Default::default()
                }],
            );
        }
        drop(vk_submit_span);
        if vk_dev
            .end_command_buffer(buffer)
            .inspect_err(|err| error!("failed to end command buffer: {err}"))
            .is_err()
        {
            vk_dev.destroy_command_pool(command_pool, None);
            return;
        }

        let mut timeline_info =
            vk::SemaphoreTypeCreateInfo::default().semaphore_type(vk::SemaphoreType::TIMELINE);
        let Ok(timeline_semaphore) = vk_dev
            .create_semaphore(
                &vk::SemaphoreCreateInfo::default().push_next(&mut timeline_info),
                None,
            )
            .inspect_err(|err| error!("failed to create timeline semaphore: {err}"))
        else {
            vk_dev.destroy_command_pool(command_pool, None);
            return;
        };
        let mut timeline_info =
            vk::TimelineSemaphoreSubmitInfo::default().signal_semaphore_values(&[2]);
        if vk_dev
            .queue_submit(
                dev.raw_queue(),
                &[vk::SubmitInfo::default()
                    .command_buffers(&[buffer])
                    .signal_semaphores(&[timeline_semaphore])
                    .push_next(&mut timeline_info)],
                vk::Fence::null(),
            )
            .inspect_err(|err| error!("failed to submit queue: {err}"))
            .is_err()
        {
            vk_dev.destroy_command_pool(command_pool, None);
            vk_dev.destroy_semaphore(timeline_semaphore, None);
            return;
        };
        if vk_dev
            .wait_semaphores(
                &vk::SemaphoreWaitInfo::default()
                    .values(&[2])
                    .semaphores(&[timeline_semaphore]),
                u64::MAX,
            )
            .inspect_err(|err| error!("failed to wait for semaphore: {err}"))
            .is_err()
        {
            vk_dev.destroy_command_pool(command_pool, None);
            vk_dev.destroy_semaphore(timeline_semaphore, None);
            return;
        };
        vk_dev.destroy_semaphore(timeline_semaphore, None);
        vk_dev.destroy_command_pool(command_pool, None);
    };
}

#[tracing::instrument(level = "debug", skip(render_device, on_drop))]
pub fn import_texture(
    render_device: &RenderDevice,
    buf: Dmatex,
    on_drop: DropCallback,
    usage: DmatexUsage,
) -> Result<ImportedTexture, ImportError> {
    if buf.planes.is_empty() {
        return Err(ImportError::NoPlanes);
    }

    let vulkan_format = drm_fourcc_to_vk_format(
        DrmFourcc::try_from(buf.format).map_err(ImportError::UnrecognizedFourcc)?,
    )
        .ok_or(ImportError::VulkanIncompatibleFormat)?;
    let vulkan_format = buf
        .srgb
        .then(|| vk_format_to_srgb(vulkan_format))
        .flatten()
        .unwrap_or(vulkan_format);
    let wgpu_desc = get_imported_descriptor(&buf)?;

    let vk_device = unsafe {
        render_device
            .wgpu_device()
            .as_hal::<Vulkan>()
            .ok_or(ImportError::NotVulkan)?
    };

    let (_format_properties, drm_format_properties) = get_drm_modifiers(
        vk_device.shared_instance().raw_instance(),
        vk_device.raw_physical_device(),
        vulkan_format,
    );

    let vk_drm_modifier = drm_format_properties
        .iter()
        .find(|v| v.drm_format_modifier == buf.modifier)
        .ok_or(ImportError::ModifierInvalid)?;

    let size = wgpu::Extent3d {
        width: buf.res.x,
        height: buf.res.y,
        depth_or_array_layers: 1,
    };

    let (image, mems) = {
        let mut disjoint = false;
        for _plane in buf.planes.iter() {
            disjoint |= vk_drm_modifier
                .drm_format_modifier_tiling_features
                .contains(FormatFeatureFlags2::DISJOINT_KHR);
        }
        let image_type = vk::ImageType::TYPE_2D;
        let usage_flags = vk::ImageUsageFlags::COLOR_ATTACHMENT
            | vk::ImageUsageFlags::SAMPLED
            | vk::ImageUsageFlags::TRANSFER_SRC
            | vk::ImageUsageFlags::TRANSFER_DST;
        let create_flags = match disjoint {
            true => vk::ImageCreateFlags::DISJOINT,
            false => vk::ImageCreateFlags::empty(),
        };

        let _format_info = get_drm_image_modifier_info(
            vk_device.shared_instance().raw_instance(),
            vk_device.raw_physical_device(),
            vulkan_format,
            image_type,
            usage_flags,
            create_flags,
            buf.modifier,
        )
            .ok_or(ImportError::ModifierInvalid)?;

        let plane_layouts = buf
            .planes
            .iter()
            .map(|p| SubresourceLayout {
                offset: p.offset as _,
                row_pitch: p.stride as _,
                array_pitch: 0,
                depth_pitch: 0,
                // per spec this has to be ignored by the impl
                size: 0,
            })
            .collect::<Vec<_>>();

        let mut drm_explicit_create_info = (buf.planes.len() == 1).then(|| {
            vk::ImageDrmFormatModifierExplicitCreateInfoEXT::default()
                .drm_format_modifier(buf.modifier)
                .plane_layouts(&plane_layouts)
        });

        let modifiers = [buf.modifier];
        let mut drm_list_create_info = (buf.planes.len() > 1).then(|| {
            vk::ImageDrmFormatModifierListCreateInfoEXT::default().drm_format_modifiers(&modifiers)
        });

        let mut external_memory_info = vk::ExternalMemoryImageCreateInfo::default()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);

        let mut image_create_info = vk::ImageCreateInfo::default()
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .image_type(image_type)
            .usage(usage_flags)
            .flags(create_flags)
            .format(vulkan_format)
            .extent(vk::Extent3D {
                width: buf.res.x,
                height: buf.res.y,
                depth: 1,
            })
            .samples(vk::SampleCountFlags::TYPE_1)
            .array_layers(1)
            .mip_levels(1)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
            .push_next(&mut external_memory_info);
        if let Some(info) = drm_explicit_create_info.as_mut() {
            image_create_info = image_create_info.push_next(info);
        }
        if let Some(info) = drm_list_create_info.as_mut() {
            image_create_info = image_create_info.push_next(info);
        }
        let image = unsafe {
            vk_device
                .raw_device()
                .create_image(&image_create_info, None)
                .map_err(ImportError::VulkanImageCreationFailed)?
        };

        let mem_properties = unsafe {
            vk_device
                .shared_instance()
                .raw_instance()
                .get_physical_device_memory_properties(vk_device.raw_physical_device())
        };

        let memory_types = &mem_properties.memory_types_as_slice();
        let valid_memory_types = memory_types
            .iter()
            .enumerate()
            .fold(u32::MAX, |u, (i, mem)| {
                if (vk::MemoryPropertyFlags::RDMA_CAPABLE_NV
                    | vk::MemoryPropertyFlags::DEVICE_COHERENT_AMD
                    | vk::MemoryPropertyFlags::PROTECTED
                    | vk::MemoryPropertyFlags::LAZILY_ALLOCATED)
                    .intersects(mem.property_flags)
                {
                    u & !(1 << i)
                } else {
                    u
                }
            });
        let memory_type_idx = memory_types
            .iter()
            .zip(0u32..)
            .find(|(t, _)| {
                t.property_flags
                    .intersects(vk::MemoryPropertyFlags::from_raw(valid_memory_types))
            })
            .ok_or(ImportError::NoValidMemoryTypes)?
            .1;

        let mut mems = if disjoint {
            import_disjoint(buf, render_device, image, memory_type_idx)?
        } else {
            vec![import_non_disjoint(
                buf,
                render_device,
                image,
                memory_type_idx,
            )?]
        };

        let bind_infos = mems
            .iter_mut()
            .map(|(mem, info)| match info {
                Some(info) => vk::BindImageMemoryInfo::default()
                    .image(image)
                    .memory(*mem)
                    .push_next(info),
                None => vk::BindImageMemoryInfo::default().image(image).memory(*mem),
            })
            .collect::<Vec<_>>();
        unsafe {
            vk_device
                .raw_device()
                .bind_image_memory2(&bind_infos)
                .map_err(ImportError::VulkanImageMemoryBindFailed)?;
        }

        (image, mems)
    };

    let descriptor = TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: vulkan_to_wgpu(vulkan_format).ok_or(ImportError::WgpuIncompatibleFormat)?,
        usage: TextureUses::COLOR_TARGET | TextureUses::PRESENT,
        memory_flags: MemoryFlags::empty(),
        view_formats: vec![],
    };

    let texture = unsafe {
        vk_device.texture_from_raw(
            image,
            &descriptor,
            Some({
                let dev: RenderDevice = render_device.clone();
                Box::new(move || {
                    let _on_drop = on_drop;
                    if let Some(dev) = dev.wgpu_device().as_hal::<Vulkan>() {
                        for (mem, _) in mems {
                            dev.raw_device().free_memory(mem, None);
                        }
                        dev.raw_device().destroy_image(image, None);
                    }
                })
            }),
        )
    };

    let wgpu_texture = unsafe {
        render_device
            .wgpu_device()
            .create_texture_from_hal::<Vulkan>(texture, &wgpu_desc)
    };
    let texture = Texture::from(wgpu_texture);
    let texture_view = texture.create_view(&TextureViewDescriptor {
        label: None,
        format: Some(texture.format()),
        dimension: Some(wgpu::TextureViewDimension::D2),
        usage: Some(texture.usage()),
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: Some(texture.mip_level_count()),
        base_array_layer: 0,
        array_layer_count: Some(texture.depth_or_array_layers()),
    });
    Ok(ImportedTexture {
        texture,
        texture_view,
        _usage: usage,
    })
}

fn import_disjoint<'a>(
    buf: Dmatex,
    dev: &RenderDevice,
    image: vk::Image,
    memory_type_idx: u32,
) -> Result<Vec<(DeviceMemory, Option<vk::BindImagePlaneMemoryInfo<'a>>)>, ImportError> {
    let dev = unsafe {
        dev.wgpu_device()
            .as_hal::<Vulkan>()
            .ok_or(ImportError::NotVulkan)?
    };

    let mut plane_mems = Vec::with_capacity(4);
    for (i, v) in buf.planes.into_iter().enumerate() {
        let fd = v.dmabuf_fd;
        let aspect_flags = match i {
            0 => vk::ImageAspectFlags::MEMORY_PLANE_0_EXT,
            1 => vk::ImageAspectFlags::MEMORY_PLANE_1_EXT,
            2 => vk::ImageAspectFlags::MEMORY_PLANE_2_EXT,
            3 => vk::ImageAspectFlags::MEMORY_PLANE_3_EXT,
            _ => return Err(ImportError::IncorrectNumberOfPlanes),
        };

        let mut dedicated_req = MemoryDedicatedRequirements::default();
        let mut plane_req_info =
            ImagePlaneMemoryRequirementsInfo::default().plane_aspect(aspect_flags);
        let mem_req_info = vk::ImageMemoryRequirementsInfo2::default()
            .image(image)
            .push_next(&mut plane_req_info);
        let mut mem_reqs = MemoryRequirements2::default().push_next(&mut dedicated_req);
        unsafe {
            dev.raw_device()
                .get_image_memory_requirements2(&mem_req_info, &mut mem_reqs)
        };
        let needs_dedicated = dedicated_req.requires_dedicated_allocation != 0;
        let layout = unsafe {
            dev.raw_device().get_image_subresource_layout(
                image,
                vk::ImageSubresource::default().aspect_mask(aspect_flags),
            )
        };

        let mut external_fd_info = vk::ImportMemoryFdInfoKHR::default()
            .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
            .fd(fd.into_raw_fd());

        let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let mut alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(layout.size)
            .memory_type_index(memory_type_idx)
            .push_next(&mut external_fd_info);
        if needs_dedicated {
            alloc_info = alloc_info.push_next(&mut dedicated);
        }

        let mem = allocate_image(image, &dev, &mut alloc_info)?;

        plane_mems.push((
            mem,
            Some(vk::BindImagePlaneMemoryInfo::default().plane_aspect(aspect_flags)),
        ));
    }
    Ok(plane_mems)
}

fn import_non_disjoint<'a>(
    buf: Dmatex,
    dev: &RenderDevice,
    image: vk::Image,
    memory_type_idx: u32,
) -> Result<(DeviceMemory, Option<vk::BindImagePlaneMemoryInfo<'a>>), ImportError> {
    let dev = unsafe {
        dev.wgpu_device()
            .as_hal::<Vulkan>()
            .ok_or(ImportError::NotVulkan)?
    };

    let fd = buf
        .planes
        .into_iter()
        .next()
        .ok_or(ImportError::NoPlanes)?
        .dmabuf_fd;
    let mut dedicated_req = MemoryDedicatedRequirements::default();
    let mut mem_reqs = MemoryRequirements2::default().push_next(&mut dedicated_req);
    let mem_req_info = vk::ImageMemoryRequirementsInfo2::default().image(image);
    unsafe {
        dev.raw_device()
            .get_image_memory_requirements2(&mem_req_info, &mut mem_reqs)
    };
    let size = mem_reqs.memory_requirements.size;

    let needs_dedicated = dedicated_req.requires_dedicated_allocation != 0;

    let mut external_fd_info = vk::ImportMemoryFdInfoKHR::default()
        .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
        .fd(fd.into_raw_fd());
    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
    let mut alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(size)
        .memory_type_index(memory_type_idx)
        .push_next(&mut external_fd_info);
    if needs_dedicated {
        alloc_info = alloc_info.push_next(&mut dedicated);
    }
    let mem = allocate_image(image, &dev, &mut alloc_info)?;
    Ok((mem, None))
}

fn allocate_image(
    image: vk::Image,
    dev: &VkDevice,
    alloc_info: &mut vk::MemoryAllocateInfo,
) -> Result<DeviceMemory, ImportError> {
    let mem = unsafe {
        dev.raw_device()
            .allocate_memory(alloc_info, None)
            .inspect_err(|_| dev.raw_device().destroy_image(image, None))
            .map_err(ImportError::VulkanMemoryAllocFailed)?
    };
    Ok(mem)
}
