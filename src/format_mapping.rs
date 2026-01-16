use ash::vk::{self, FormatProperties, FormatProperties2, ImageFormatProperties};
use tracing::error;

pub fn get_drm_modifiers(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    format: vk::Format,
) -> (FormatProperties, Vec<vk::DrmFormatModifierProperties2EXT>) {
    let mut drm_modifier_list_len = vk::DrmFormatModifierPropertiesList2EXT::default();
    unsafe {
        instance.get_physical_device_format_properties2(
            physical_device,
            format,
            &mut FormatProperties2::default().push_next(&mut drm_modifier_list_len),
        );
    }
    let buf_len = drm_modifier_list_len
        .drm_format_modifier_count
        .try_into()
        .unwrap_or(usize::MAX);
    let mut buf = vec![vk::DrmFormatModifierProperties2EXT::default(); buf_len];

    let mut drm_modifier_list =
        vk::DrmFormatModifierPropertiesList2EXT::default().drm_format_modifier_properties(&mut buf);
    let mut format_properties = FormatProperties2::default().push_next(&mut drm_modifier_list);
    unsafe {
        instance.get_physical_device_format_properties2(
            physical_device,
            format,
            &mut format_properties,
        );
    }
    let format_properties = format_properties.format_properties;
    let written_buf_len = drm_modifier_list
        .drm_format_modifier_count
        .try_into()
        .unwrap_or(usize::MAX);
    buf.truncate(written_buf_len);
    (format_properties, buf)
}

pub fn get_drm_image_modifier_info(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    format: vk::Format,
    image_type: vk::ImageType,
    usage: vk::ImageUsageFlags,
    flags: vk::ImageCreateFlags,
    modifier: u64,
) -> Option<ImageFormatProperties> {
    let mut drm_info = vk::PhysicalDeviceImageDrmFormatModifierInfoEXT::default()
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .drm_format_modifier(modifier);
    let image_format_info = vk::PhysicalDeviceImageFormatInfo2::default()
        .format(format)
        .ty(image_type)
        .usage(usage)
        .flags(flags)
        .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
        .push_next(&mut drm_info);
    let mut properties = vk::ImageFormatProperties2::default();
    unsafe {
        match instance.get_physical_device_image_format_properties2(
            physical_device,
            &image_format_info,
            &mut properties,
        ) {
            Ok(_) => {}
            Err(vk::Result::ERROR_FORMAT_NOT_SUPPORTED) => return None,
            Err(err) => {
                error!("failed to get format properties: {err}");
                return None;
            }
        };
    }

    Some(properties.image_format_properties)
}

pub fn drm_fourcc_to_vk_format(drm_format: drm_fourcc::DrmFourcc) -> Option<vk::Format> {
    use drm_fourcc::DrmFourcc as D;
    use vk::Format as F;
    Some(match drm_format {
        D::Abgr1555 | D::Xbgr1555 => F::R5G5B5A1_UNORM_PACK16,
        D::Abgr2101010 | D::Xbgr2101010 => F::A2B10G10R10_UNORM_PACK32,
        D::Abgr4444 | D::Xbgr4444 => F::A4B4G4R4_UNORM_PACK16,
        D::Abgr8888 | D::Xbgr8888 => F::R8G8B8A8_UNORM,
        D::Argb1555 | D::Xrgb1555 => F::A1R5G5B5_UNORM_PACK16,
        D::Argb2101010 | D::Xrgb2101010 => F::A2R10G10B10_UNORM_PACK32,
        D::Argb4444 | D::Xrgb4444 => F::B4G4R4A4_UNORM_PACK16,
        D::Argb8888 | D::Xrgb8888 => F::B8G8R8A8_UNORM,
        D::Bgr565 => F::B5G6R5_UNORM_PACK16,
        D::Bgr888 => F::B8G8R8_UNORM,
        D::Bgr888_a8 => F::B8G8R8A8_UNORM,
        D::Bgra4444 | D::Bgrx4444 => F::B4G4R4A4_UNORM_PACK16,
        D::Bgra5551 | D::Bgrx5551 => F::B5G5R5A1_UNORM_PACK16,
        D::Bgra8888 | D::Bgrx8888 => F::B8G8R8A8_UNORM,
        D::R16 => F::R16_UNORM,
        D::R8 => F::R8_UNORM,
        D::Rg1616 => F::R16G16_UNORM,
        D::Rg88 => F::R8G8_UNORM,
        D::Rgb565 => F::R5G6B5_UNORM_PACK16,
        D::Rgb888 => F::R8G8B8_UNORM,
        D::Rgb888_a8 => F::R8G8B8A8_UNORM,
        D::Rgba4444 | D::Rgbx4444 => F::R4G4B4A4_UNORM_PACK16,
        D::Rgba5551 | D::Rgbx5551 => F::R5G5B5A1_UNORM_PACK16,
        D::Rgba8888 | D::Rgbx8888 => F::R8G8B8A8_UNORM,
        _ => return None,
    })
}

pub fn vk_format_to_srgb(vk_format: vk::Format) -> Option<vk::Format> {
    use vk::Format as F;
    Some(match vk_format {
        F::R8_UNORM => F::R8_SRGB,
        F::R8G8_UNORM => F::R8G8_SRGB,
        F::R8G8B8_UNORM => F::R8G8B8_SRGB,
        F::B8G8R8_UNORM => F::B8G8R8_SRGB,
        F::R8G8B8A8_UNORM => F::R8G8B8A8_SRGB,
        F::B8G8R8A8_UNORM => F::B8G8R8A8_SRGB,
        F::A8B8G8R8_UNORM_PACK32 => F::A8B8G8R8_SRGB_PACK32,
        F::BC1_RGB_UNORM_BLOCK => F::BC1_RGB_SRGB_BLOCK,
        F::BC1_RGBA_UNORM_BLOCK => F::BC1_RGBA_SRGB_BLOCK,
        F::BC2_UNORM_BLOCK => F::BC2_SRGB_BLOCK,
        F::BC3_UNORM_BLOCK => F::BC3_SRGB_BLOCK,
        F::BC7_UNORM_BLOCK => F::BC7_SRGB_BLOCK,
        F::ETC2_R8G8B8_UNORM_BLOCK => F::ETC2_R8G8B8_SRGB_BLOCK,
        F::ETC2_R8G8B8A1_UNORM_BLOCK => F::ETC2_R8G8B8A1_SRGB_BLOCK,
        F::ETC2_R8G8B8A8_UNORM_BLOCK => F::ETC2_R8G8B8A8_SRGB_BLOCK,
        _ => return None,
    })
}

pub fn vulkan_to_wgpu(format: vk::Format) -> Option<wgpu::TextureFormat> {
    use ash::vk::Format as F;
    use wgpu::TextureFormat as Tf;
    use wgpu::{AstcBlock, AstcChannel};
    Some(match format {
        F::R8_UNORM => Tf::R8Unorm,
        F::R8_SNORM => Tf::R8Snorm,
        F::R8_UINT => Tf::R8Uint,
        F::R8_SINT => Tf::R8Sint,
        F::R16_UINT => Tf::R16Uint,
        F::R16_SINT => Tf::R16Sint,
        F::R16_UNORM => Tf::R16Unorm,
        F::R16_SNORM => Tf::R16Snorm,
        F::R16_SFLOAT => Tf::R16Float,
        F::R8G8_UNORM => Tf::Rg8Unorm,
        F::R8G8_SNORM => Tf::Rg8Snorm,
        F::R8G8_UINT => Tf::Rg8Uint,
        F::R8G8_SINT => Tf::Rg8Sint,
        F::R16G16_UNORM => Tf::Rg16Unorm,
        F::R16G16_SNORM => Tf::Rg16Snorm,
        F::R32_UINT => Tf::R32Uint,
        F::R32_SINT => Tf::R32Sint,
        F::R32_SFLOAT => Tf::R32Float,
        F::R16G16_UINT => Tf::Rg16Uint,
        F::R16G16_SINT => Tf::Rg16Sint,
        F::R16G16_SFLOAT => Tf::Rg16Float,
        F::R8G8B8A8_UNORM => Tf::Rgba8Unorm,
        F::R8G8B8A8_SRGB => Tf::Rgba8UnormSrgb,
        F::B8G8R8A8_SRGB => Tf::Bgra8UnormSrgb,
        F::R8G8B8A8_SNORM => Tf::Rgba8Snorm,
        F::B8G8R8A8_UNORM => Tf::Bgra8Unorm,
        F::B8G8R8A8_UINT => Tf::Bgra8Unorm,
        F::R8G8B8A8_UINT => Tf::Rgba8Uint,
        F::R8G8B8A8_SINT => Tf::Rgba8Sint,
        F::A2B10G10R10_UINT_PACK32 => Tf::Rgb10a2Uint,
        F::A2B10G10R10_UNORM_PACK32 => Tf::Rgb10a2Unorm,
        F::B10G11R11_UFLOAT_PACK32 => Tf::Rg11b10Ufloat,
        F::R32G32_UINT => Tf::Rg32Uint,
        F::R32G32_SINT => Tf::Rg32Sint,
        F::R32G32_SFLOAT => Tf::Rg32Float,
        F::R16G16B16A16_UINT => Tf::Rgba16Uint,
        F::R16G16B16A16_SINT => Tf::Rgba16Sint,
        F::R16G16B16A16_UNORM => Tf::Rgba16Unorm,
        F::R16G16B16A16_SNORM => Tf::Rgba16Snorm,
        F::R16G16B16A16_SFLOAT => Tf::Rgba16Float,
        F::R32G32B32A32_UINT => Tf::Rgba32Uint,
        F::R32G32B32A32_SINT => Tf::Rgba32Sint,
        F::R32G32B32A32_SFLOAT => Tf::Rgba32Float,
        F::D32_SFLOAT => Tf::Depth32Float,
        F::D32_SFLOAT_S8_UINT => Tf::Depth32FloatStencil8,
        F::D16_UNORM => Tf::Depth16Unorm,
        F::G8_B8R8_2PLANE_420_UNORM => Tf::NV12,
        F::E5B9G9R9_UFLOAT_PACK32 => Tf::Rgb9e5Ufloat,
        F::BC1_RGBA_UNORM_BLOCK => Tf::Bc1RgbaUnorm,
        F::BC1_RGBA_SRGB_BLOCK => Tf::Bc1RgbaUnormSrgb,
        F::BC2_UNORM_BLOCK => Tf::Bc2RgbaUnorm,
        F::BC2_SRGB_BLOCK => Tf::Bc2RgbaUnormSrgb,
        F::BC3_UNORM_BLOCK => Tf::Bc3RgbaUnorm,
        F::BC3_SRGB_BLOCK => Tf::Bc3RgbaUnormSrgb,
        F::BC4_UNORM_BLOCK => Tf::Bc4RUnorm,
        F::BC4_SNORM_BLOCK => Tf::Bc4RSnorm,
        F::BC5_UNORM_BLOCK => Tf::Bc5RgUnorm,
        F::BC5_SNORM_BLOCK => Tf::Bc5RgSnorm,
        F::BC6H_UFLOAT_BLOCK => Tf::Bc6hRgbUfloat,
        F::BC6H_SFLOAT_BLOCK => Tf::Bc6hRgbFloat,
        F::BC7_UNORM_BLOCK => Tf::Bc7RgbaUnorm,
        F::BC7_SRGB_BLOCK => Tf::Bc7RgbaUnormSrgb,
        F::ETC2_R8G8B8_UNORM_BLOCK => Tf::Etc2Rgb8Unorm,
        F::ETC2_R8G8B8_SRGB_BLOCK => Tf::Etc2Rgb8UnormSrgb,
        F::ETC2_R8G8B8A1_UNORM_BLOCK => Tf::Etc2Rgb8A1Unorm,
        F::ETC2_R8G8B8A1_SRGB_BLOCK => Tf::Etc2Rgb8A1UnormSrgb,
        F::ETC2_R8G8B8A8_UNORM_BLOCK => Tf::Etc2Rgba8Unorm,
        F::ETC2_R8G8B8A8_SRGB_BLOCK => Tf::Etc2Rgba8UnormSrgb,
        F::EAC_R11_UNORM_BLOCK => Tf::EacR11Unorm,
        F::EAC_R11_SNORM_BLOCK => Tf::EacR11Snorm,
        F::EAC_R11G11_UNORM_BLOCK => Tf::EacRg11Unorm,
        F::EAC_R11G11_SNORM_BLOCK => Tf::EacRg11Snorm,
        F::ASTC_4X4_UNORM_BLOCK => Tf::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::Unorm,
        },
        F::ASTC_5X4_UNORM_BLOCK => Tf::Astc {
            block: AstcBlock::B5x4,
            channel: AstcChannel::Unorm,
        },
        F::ASTC_5X5_UNORM_BLOCK => Tf::Astc {
            block: AstcBlock::B5x5,
            channel: AstcChannel::Unorm,
        },
        F::ASTC_6X5_UNORM_BLOCK => Tf::Astc {
            block: AstcBlock::B6x5,
            channel: AstcChannel::Unorm,
        },
        F::ASTC_6X6_UNORM_BLOCK => Tf::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::Unorm,
        },
        F::ASTC_8X5_UNORM_BLOCK => Tf::Astc {
            block: AstcBlock::B8x5,
            channel: AstcChannel::Unorm,
        },
        F::ASTC_8X6_UNORM_BLOCK => Tf::Astc {
            block: AstcBlock::B8x6,
            channel: AstcChannel::Unorm,
        },
        F::ASTC_8X8_UNORM_BLOCK => Tf::Astc {
            block: AstcBlock::B8x8,
            channel: AstcChannel::Unorm,
        },
        F::ASTC_10X5_UNORM_BLOCK => Tf::Astc {
            block: AstcBlock::B10x5,
            channel: AstcChannel::Unorm,
        },
        F::ASTC_10X6_UNORM_BLOCK => Tf::Astc {
            block: AstcBlock::B10x6,
            channel: AstcChannel::Unorm,
        },
        F::ASTC_10X8_UNORM_BLOCK => Tf::Astc {
            block: AstcBlock::B10x8,
            channel: AstcChannel::Unorm,
        },
        F::ASTC_10X10_UNORM_BLOCK => Tf::Astc {
            block: AstcBlock::B10x10,
            channel: AstcChannel::Unorm,
        },
        F::ASTC_12X10_UNORM_BLOCK => Tf::Astc {
            block: AstcBlock::B12x10,
            channel: AstcChannel::Unorm,
        },
        F::ASTC_12X12_UNORM_BLOCK => Tf::Astc {
            block: AstcBlock::B12x12,
            channel: AstcChannel::Unorm,
        },
        F::ASTC_4X4_SRGB_BLOCK => Tf::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::UnormSrgb,
        },
        F::ASTC_5X4_SRGB_BLOCK => Tf::Astc {
            block: AstcBlock::B5x4,
            channel: AstcChannel::UnormSrgb,
        },
        F::ASTC_5X5_SRGB_BLOCK => Tf::Astc {
            block: AstcBlock::B5x5,
            channel: AstcChannel::UnormSrgb,
        },
        F::ASTC_6X5_SRGB_BLOCK => Tf::Astc {
            block: AstcBlock::B6x5,
            channel: AstcChannel::UnormSrgb,
        },
        F::ASTC_6X6_SRGB_BLOCK => Tf::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::UnormSrgb,
        },
        F::ASTC_8X5_SRGB_BLOCK => Tf::Astc {
            block: AstcBlock::B8x5,
            channel: AstcChannel::UnormSrgb,
        },
        F::ASTC_8X6_SRGB_BLOCK => Tf::Astc {
            block: AstcBlock::B8x6,
            channel: AstcChannel::UnormSrgb,
        },
        F::ASTC_8X8_SRGB_BLOCK => Tf::Astc {
            block: AstcBlock::B8x8,
            channel: AstcChannel::UnormSrgb,
        },
        F::ASTC_10X5_SRGB_BLOCK => Tf::Astc {
            block: AstcBlock::B10x5,
            channel: AstcChannel::UnormSrgb,
        },
        F::ASTC_10X6_SRGB_BLOCK => Tf::Astc {
            block: AstcBlock::B10x6,
            channel: AstcChannel::UnormSrgb,
        },
        F::ASTC_10X8_SRGB_BLOCK => Tf::Astc {
            block: AstcBlock::B10x8,
            channel: AstcChannel::UnormSrgb,
        },
        F::ASTC_10X10_SRGB_BLOCK => Tf::Astc {
            block: AstcBlock::B10x10,
            channel: AstcChannel::UnormSrgb,
        },
        F::ASTC_12X10_SRGB_BLOCK => Tf::Astc {
            block: AstcBlock::B12x10,
            channel: AstcChannel::UnormSrgb,
        },
        F::ASTC_12X12_SRGB_BLOCK => Tf::Astc {
            block: AstcBlock::B12x12,
            channel: AstcChannel::UnormSrgb,
        },
        F::ASTC_4X4_SFLOAT_BLOCK_EXT => Tf::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::Hdr,
        },
        F::ASTC_5X4_SFLOAT_BLOCK_EXT => Tf::Astc {
            block: AstcBlock::B5x4,
            channel: AstcChannel::Hdr,
        },
        F::ASTC_5X5_SFLOAT_BLOCK_EXT => Tf::Astc {
            block: AstcBlock::B5x5,
            channel: AstcChannel::Hdr,
        },
        F::ASTC_6X5_SFLOAT_BLOCK_EXT => Tf::Astc {
            block: AstcBlock::B6x5,
            channel: AstcChannel::Hdr,
        },
        F::ASTC_6X6_SFLOAT_BLOCK_EXT => Tf::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::Hdr,
        },
        F::ASTC_8X5_SFLOAT_BLOCK_EXT => Tf::Astc {
            block: AstcBlock::B8x5,
            channel: AstcChannel::Hdr,
        },
        F::ASTC_8X6_SFLOAT_BLOCK_EXT => Tf::Astc {
            block: AstcBlock::B8x6,
            channel: AstcChannel::Hdr,
        },
        F::ASTC_8X8_SFLOAT_BLOCK_EXT => Tf::Astc {
            block: AstcBlock::B8x8,
            channel: AstcChannel::Hdr,
        },
        F::ASTC_10X5_SFLOAT_BLOCK_EXT => Tf::Astc {
            block: AstcBlock::B10x5,
            channel: AstcChannel::Hdr,
        },
        F::ASTC_10X6_SFLOAT_BLOCK_EXT => Tf::Astc {
            block: AstcBlock::B10x6,
            channel: AstcChannel::Hdr,
        },
        F::ASTC_10X8_SFLOAT_BLOCK_EXT => Tf::Astc {
            block: AstcBlock::B10x8,
            channel: AstcChannel::Hdr,
        },
        F::ASTC_10X10_SFLOAT_BLOCK_EXT => Tf::Astc {
            block: AstcBlock::B10x10,
            channel: AstcChannel::Hdr,
        },
        F::ASTC_12X10_SFLOAT_BLOCK_EXT => Tf::Astc {
            block: AstcBlock::B12x10,
            channel: AstcChannel::Hdr,
        },
        F::ASTC_12X12_SFLOAT_BLOCK_EXT => Tf::Astc {
            block: AstcBlock::B12x12,
            channel: AstcChannel::Hdr,
        },
        _ => return None,
    })
}