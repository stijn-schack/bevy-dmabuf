use ash::vk;

pub fn to_srgb(vk_format: vk::Format) -> Option<vk::Format> {
    use vk::Format as F;
    Some(match vk_format {
        F::B8G8R8A8_UNORM => F::B8G8R8A8_SRGB,
        F::R8G8B8A8_UNORM => F::R8G8B8A8_SRGB,
        F::A8B8G8R8_UNORM_PACK32 => F::A8B8G8R8_SRGB_PACK32,
        _ => return None,
    })
}
