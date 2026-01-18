use crate::import::ExternalImageUsage;
use crate::{
    dmatex::{Dmatex, DmatexPlane},
    format_mapping::vulkan_to_wgpu,
    import::render_world::{ImportError, ImportedTexture},
};
use ash::vk;
use drm_fourcc::DrmModifier;
use std::os::fd::IntoRawFd;
use wgpu::hal::{api::Vulkan, vulkan::Device as VkDevice};

mod formats;

pub fn import_dmabuf_as_texture(
    device: &wgpu::Device,
    dma: Dmatex,
    usage: ExternalImageUsage,
) -> Result<ImportedTexture, ImportError> {
    let vk_device = unsafe { device.as_hal::<Vulkan>().ok_or(ImportError::NotVulkan) }?;

    let vk_format = choose_vk_format(&dma)?;
    let wgpu_format = vulkan_to_wgpu(vk_format).ok_or(ImportError::WgpuIncompatibleFormat)?;
    let wgpu_desc = wgpu_texture_desc(&dma, wgpu_format, usage);

    ensure_modifier_supported(&vk_device, vk_format, dma.format.modifier)?;

    let disjoint = needs_disjoint(&dma);
    let image = create_vk_image(&vk_device, &dma, vk_format, disjoint)?;
    let mem_props = get_mem_props(&vk_device);

    let hal_tex_desc = hal_texture_desc(&dma, wgpu_format, usage);

    let plane_binds = import_and_bind_image_memory(&vk_device, dma, image, disjoint, &mem_props)?;

    let drop_callback = {
        let device = device.clone();
        Box::new(move || {
            // SAFETY: By reaching this point we already know we're using vulkan.
            let vk_device = unsafe { device.as_hal::<Vulkan>().unwrap_unchecked() };
            unsafe {
                for b in plane_binds {
                    vk_device.raw_device().free_memory(b.memory, None);
                }
                vk_device.raw_device().destroy_image(image, None);
            }
        })
    };

    let hal_texture =
        unsafe { vk_device.texture_from_raw(image, &hal_tex_desc, Some(drop_callback)) };

    let wgpu_texture = unsafe { device.create_texture_from_hal::<Vulkan>(hal_texture, &wgpu_desc) };
    let view = wgpu_texture.create_view(&wgpu::TextureViewDescriptor::default());

    Ok(ImportedTexture {
        texture: wgpu_texture.into(),
        texture_view: view.into(),
    })
}

fn choose_vk_format(dma: &Dmatex) -> Result<vk::Format, ImportError> {
    let base =
        formats::get_vk_format(dma.format.code).ok_or(ImportError::VulkanIncompatibleFormat)?;
    let fmt = if dma.srgb {
        formats::to_srgb(base).unwrap_or(base)
    } else {
        base
    };
    Ok(fmt)
}

fn needs_disjoint(desc: &Dmatex) -> bool {
    desc.planes.len() > 1
}

fn ensure_modifier_supported(
    hal: &VkDevice,
    format: vk::Format,
    modifier: DrmModifier,
) -> Result<(), ImportError> {
    let modifier = u64::from(modifier);

    let supported_modifiers = get_supported_modifiers_for_format(
        hal.shared_instance().raw_instance(),
        hal.raw_physical_device(),
        format,
    );

    if !supported_modifiers
        .iter()
        .any(|m| m.drm_format_modifier == modifier)
    {
        return Err(ImportError::ModifierInvalid);
    }

    Ok(())
}

fn default_usage_flags() -> vk::ImageUsageFlags {
    vk::ImageUsageFlags::SAMPLED
        | vk::ImageUsageFlags::TRANSFER_SRC
        | vk::ImageUsageFlags::TRANSFER_DST
        | vk::ImageUsageFlags::COLOR_ATTACHMENT
}

fn create_vk_image(
    hal: &VkDevice,
    dma: &Dmatex,
    format: vk::Format,
    disjoint: bool,
) -> Result<vk::Image, ImportError> {
    let mut external = vk::ExternalMemoryImageCreateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);

    let plane_layouts = plane_layouts(dma);
    let mut drm_explicit = vk::ImageDrmFormatModifierExplicitCreateInfoEXT::default()
        .drm_format_modifier(dma.format.modifier.into())
        .plane_layouts(&plane_layouts);

    let create_flags = if disjoint {
        vk::ImageCreateFlags::DISJOINT
    } else {
        vk::ImageCreateFlags::empty()
    };

    let info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(vk::Extent3D {
            width: dma.res.x,
            height: dma.res.y,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
        .usage(default_usage_flags())
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .flags(create_flags)
        .push_next(&mut external)
        .push_next(&mut drm_explicit);

    let image = unsafe {
        hal.raw_device()
            .create_image(&info, None)
            .map_err(ImportError::VulkanImageCreationFailed)?
    };

    Ok(image)
}

fn plane_layouts(dma: &Dmatex) -> Vec<vk::SubresourceLayout> {
    dma.planes
        .iter()
        .map(|plane| vk::SubresourceLayout {
            offset: plane.offset as vk::DeviceSize,
            row_pitch: plane.stride as vk::DeviceSize,
            array_pitch: 0,
            depth_pitch: 0,
            size: 0,
        })
        .collect()
}

fn get_mem_props(hal: &VkDevice) -> vk::PhysicalDeviceMemoryProperties {
    unsafe {
        hal.shared_instance()
            .raw_instance()
            .get_physical_device_memory_properties(hal.raw_physical_device())
    }
}

fn import_and_bind_image_memory(
    hal: &VkDevice,
    dma: Dmatex,
    image: vk::Image,
    disjoint: bool,
    mem_props: &vk::PhysicalDeviceMemoryProperties,
) -> Result<Vec<PlaneBind>, ImportError> {
    let mut binds = Vec::with_capacity(dma.planes.len());

    if !disjoint {
        let plane = dma.planes.into_iter().next().ok_or(ImportError::NoPlanes)?;
        let req = query_image_requirements(hal, image, None);
        let mem = import_plane_memory(hal, image, plane, req, mem_props)?;
        binds.push(PlaneBind {
            memory: mem,
            aspect_flags: None,
        });
        bind_image_memory(hal, image, &binds)?;
        return Ok(binds);
    }

    for (i, plane) in dma.planes.into_iter().enumerate() {
        let aspect = memory_plane_aspect(i)?;
        let req = query_image_requirements(hal, image, Some(aspect));
        let mem = import_plane_memory(hal, image, plane, req, mem_props)?;
        binds.push(PlaneBind {
            memory: mem,
            aspect_flags: Some(aspect),
        });
    }

    bind_image_memory(hal, image, &binds)?;
    Ok(binds)
}

fn memory_plane_aspect(i: usize) -> Result<vk::ImageAspectFlags, ImportError> {
    Ok(match i {
        0 => vk::ImageAspectFlags::MEMORY_PLANE_0_EXT,
        1 => vk::ImageAspectFlags::MEMORY_PLANE_1_EXT,
        2 => vk::ImageAspectFlags::MEMORY_PLANE_2_EXT,
        3 => vk::ImageAspectFlags::MEMORY_PLANE_3_EXT,
        _ => return Err(ImportError::IncorrectNumberOfPlanes),
    })
}

struct PlaneReq {
    size: vk::DeviceSize,
    memory_type_bits: u32,
    needs_dedicated: bool,
}

fn query_image_requirements(
    hal: &VkDevice,
    image: vk::Image,
    plane: Option<vk::ImageAspectFlags>,
) -> PlaneReq {
    let mut dedicated = vk::MemoryDedicatedRequirements::default();

    let mut plane_info =
        plane.map(|aspect| vk::ImagePlaneMemoryRequirementsInfo::default().plane_aspect(aspect));

    let mut req_info = vk::ImageMemoryRequirementsInfo2::default().image(image);
    if let Some(p) = plane_info.as_mut() {
        req_info = req_info.push_next(p);
    }

    let mut reqs2 = vk::MemoryRequirements2::default().push_next(&mut dedicated);
    unsafe {
        hal.raw_device()
            .get_image_memory_requirements2(&req_info, &mut reqs2);
    }

    PlaneReq {
        size: reqs2.memory_requirements.size,
        memory_type_bits: reqs2.memory_requirements.memory_type_bits,
        needs_dedicated: dedicated.requires_dedicated_allocation != 0,
    }
}

fn import_plane_memory(
    hal: &VkDevice,
    image: vk::Image,
    plane: DmatexPlane,
    req: PlaneReq,
    mem_props: &vk::PhysicalDeviceMemoryProperties,
) -> Result<vk::DeviceMemory, ImportError> {
    let mem_type_index = select_memory_type_index(req.memory_type_bits, mem_props)
        .ok_or(ImportError::NoValidMemoryTypes)?;

    let fd = plane.dmabuf_fd.into_raw_fd();

    let mut import_fd = vk::ImportMemoryFdInfoKHR::default()
        .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
        .fd(fd);

    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);

    let mut alloc = vk::MemoryAllocateInfo::default()
        .allocation_size(req.size)
        .memory_type_index(mem_type_index)
        .push_next(&mut import_fd);

    if req.needs_dedicated {
        alloc = alloc.push_next(&mut dedicated);
    }

    let mem = unsafe {
        hal.raw_device()
            .allocate_memory(&alloc, None)
            .map_err(ImportError::VulkanMemoryAllocFailed)?
    };

    Ok(mem)
}

fn select_memory_type_index(
    memory_type_bits: u32,
    mem_props: &vk::PhysicalDeviceMemoryProperties,
) -> Option<u32> {
    let forbidden = vk::MemoryPropertyFlags::PROTECTED | vk::MemoryPropertyFlags::LAZILY_ALLOCATED;

    let count = mem_props.memory_type_count;
    for i in 0..count {
        if (memory_type_bits & (1 << i)) == 0 {
            continue;
        }
        let flags = mem_props.memory_types[i as usize].property_flags;
        if flags.intersects(forbidden) {
            continue;
        }
        return Some(i);
    }
    None
}

struct PlaneBind {
    memory: vk::DeviceMemory,
    aspect_flags: Option<vk::ImageAspectFlags>,
}

fn bind_image_memory(
    hal: &VkDevice,
    image: vk::Image,
    binds: &[PlaneBind],
) -> Result<(), ImportError> {
    let mut plane_mem_infos =
        Vec::<Option<vk::BindImagePlaneMemoryInfo>>::with_capacity(binds.len());
    for plane_bind in binds {
        let mem_info = plane_bind
            .aspect_flags
            .map(|aspect_flags| vk::BindImagePlaneMemoryInfo::default().plane_aspect(aspect_flags));
        plane_mem_infos.push(mem_info);
    }

    let mut bind_infos = Vec::<vk::BindImageMemoryInfo>::with_capacity(binds.len());
    for (plane_bind, mem_info) in binds
        .iter()
        .zip(plane_mem_infos.iter_mut().map(|option| option.as_mut()))
    {
        let mut bind_info = vk::BindImageMemoryInfo::default()
            .image(image)
            .memory(plane_bind.memory);

        if let Some(mem_info) = mem_info {
            bind_info = bind_info.push_next(mem_info);
        }

        bind_infos.push(bind_info);
    }

    unsafe {
        hal.raw_device()
            .bind_image_memory2(&bind_infos)
            .map_err(ImportError::VulkanImageMemoryBindFailed)?;
    }
    Ok(())
}

fn hal_texture_desc(
    dma: &Dmatex,
    format: wgpu::TextureFormat,
    usage: ExternalImageUsage,
) -> wgpu::hal::TextureDescriptor<'static> {
    use wgpu::hal::MemoryFlags;
    use wgpu::{Extent3d, TextureDimension, TextureUses};

    let usage = match usage {
        ExternalImageUsage::Sampling { .. } => TextureUses::RESOURCE | TextureUses::COPY_SRC,
        ExternalImageUsage::RenderTarget { .. } => {
            TextureUses::COLOR_TARGET | TextureUses::COPY_DST
        }
    };

    wgpu::hal::TextureDescriptor {
        label: Some("imported-dmabuf-texture"),
        size: Extent3d {
            width: dma.res.x,
            height: dma.res.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage,
        memory_flags: MemoryFlags::empty(),
        view_formats: vec![],
    }
}

fn wgpu_texture_desc(
    dma: &Dmatex,
    format: wgpu::TextureFormat,
    usage: ExternalImageUsage,
) -> wgpu::TextureDescriptor<'static> {
    use wgpu::{Extent3d, TextureDimension, TextureUsages};

    let usage = match usage {
        ExternalImageUsage::Sampling { .. } => {
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC
        }
        ExternalImageUsage::RenderTarget { .. } => {
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_DST
        }
    };

    wgpu::TextureDescriptor {
        label: None,
        size: Extent3d {
            width: dma.res.x,
            height: dma.res.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    }
}

fn get_supported_modifiers_for_format(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    format: vk::Format,
) -> Vec<vk::DrmFormatModifierProperties2EXT> {
    let modifier_count = {
        let mut list_len = vk::DrmFormatModifierPropertiesList2EXT::default();
        unsafe {
            instance.get_physical_device_format_properties2(
                physical_device,
                format,
                &mut vk::FormatProperties2::default().push_next(&mut list_len),
            );
        }
        let count = list_len.drm_format_modifier_count as usize;
        if count == 0 {
            return vec![];
        }
        count
    };

    let mut supported_formats =
        vec![vk::DrmFormatModifierProperties2EXT::default(); modifier_count];

    let drm_format_modifier_count = {
        let mut list = vk::DrmFormatModifierPropertiesList2EXT::default()
            .drm_format_modifier_properties(&mut supported_formats);

        let mut props2 = vk::FormatProperties2::default().push_next(&mut list);
        unsafe {
            instance.get_physical_device_format_properties2(physical_device, format, &mut props2);
        }
        list.drm_format_modifier_count as usize
    };

    supported_formats.truncate(drm_format_modifier_count);
    supported_formats
}
