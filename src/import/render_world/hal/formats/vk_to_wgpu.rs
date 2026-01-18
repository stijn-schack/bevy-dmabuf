use ash::vk;

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