//! This file is a modified version of Smithay's DRM/Vulkan format conversions
//! Original file: https://github.com/Smithay/smithay/blob/2928e4f34541d957b7b3c3b3e13b2539cd44990f/src/backend/allocator/vulkan/format.rs
//!
//! MIT License
//!
//! Copyright (c) 2017 Victor Berger and Victoria Brekenfeld
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

#![allow(dead_code)]

mod colorspace;
pub use colorspace::*;

/// Macro to generate format conversions between Vulkan and FourCC format codes.
///
/// Any entry in this table may have attributes associated with a conversion. This is needed for `PACK` Vulkan
/// formats which may only have an alternative given a specific host endian.
///
/// See the module documentation for usage details.
macro_rules! vk_format_table {
    (
        $(
            // This meta specifier is used for format conversions for PACK formats.
            $(#[$conv_meta:meta])*
            $fourcc: ident => $vk: ident
        ),* $(,)?
    ) => {
        /// Converts a FourCC format code to a Vulkan format code.
        ///
        /// This will return [`None`] if the format is not known.
        ///
        /// These format conversions will return all known FourCC and Vulkan format conversions. However a
        /// Vulkan implementation may not support some Vulkan format. One notable example of this are the
        /// formats introduced in `VK_EXT_4444_formats`. The corresponding FourCC codes will return the
        /// formats from `VK_EXT_4444_formats`, but the caller is responsible for testing that a Vulkan device
        /// supports these formats.
        pub const fn get_vk_format(fourcc: drm_fourcc::DrmFourcc) -> Option<ash::vk::Format> {
            match fourcc {
                $(
                    $(#[$conv_meta])*
                    drm_fourcc::DrmFourcc::$fourcc => Some(ash::vk::Format::$vk),
                )*

                _ => None,
            }
        }

        /// Returns all the known format conversions.
        ///
        /// The list contains FourCC format codes that may be converted using [`get_vk_format`].
        pub const fn known_formats() -> &'static [drm_fourcc::DrmFourcc] {
            &[
                $(
                    drm_fourcc::DrmFourcc::$fourcc
                ),*
            ]
        }
    };
}

//
// Vulkan classifies formats by both channel sizes and colorspace. FourCC format codes do not classify formats
// based on colorspace.
//
// To implement this correctly, it is likely that parsing vulkan.xml and classifying families of colorspaces
// would be needed since there are a lot of formats.
//
// Many of these conversions come from wsi_common_wayland.c in Mesa
vk_format_table! {
    Argb8888 => B8G8R8A8_UNORM,
    Xrgb8888 => B8G8R8A8_UNORM,

    Abgr8888 => R8G8B8A8_UNORM,
    Xbgr8888 => R8G8B8A8_UNORM,

    // PACK32 formats are equivalent to u32 instead of [u8; 4] and thus their layout depends on the host
    // endian.
    #[cfg(target_endian = "little")]
    Rgba8888 => A8B8G8R8_UNORM_PACK32,
    #[cfg(target_endian = "little")]
    Rgbx8888 => A8B8G8R8_UNORM_PACK32,

    #[cfg(target_endian = "little")]
    Argb2101010 => A2R10G10B10_UNORM_PACK32,
    #[cfg(target_endian = "little")]
    Xrgb2101010 => A2R10G10B10_UNORM_PACK32,

    #[cfg(target_endian = "little")]
    Abgr2101010 => A2B10G10R10_UNORM_PACK32,
    #[cfg(target_endian = "little")]
    Xbgr2101010 => A2B10G10R10_UNORM_PACK32,
}
