use bevy::{
    app::{App, Plugin},
    log::warn,
    render::{
        renderer::{raw_vulkan_init::RawVulkanInitSettings, RenderAdapterInfo}, RenderApp,
        RenderPlugin,
    },
};
use std::{any::type_name, ffi::CStr};
use wgpu::Backend::Vulkan;

const REQUIRED_EXTENSIONS: &[&CStr] = &[
    ash::ext::image_drm_format_modifier::NAME,
    ash::ext::external_memory_dma_buf::NAME,
    ash::khr::external_memory_fd::NAME,
    ash::khr::external_memory::NAME,
    ash::khr::swapchain::NAME,
];

pub const fn required_device_extensions() -> &'static [&'static CStr] {
    REQUIRED_EXTENSIONS
}

/// Plugin to init the vulkan session with the required extensions,
/// probably not needed when using bevy_mod_openxr
/// Adding this plugin means that wgpu will be forced to use the Vulkan backend.
/// Must be added before [RenderPlugin], if using [bevy::DefaultPlugins],
/// make sure to add this using [bevy::app::PluginGroupBuilder::add_before]
///
/// **Usage**
///
/// ```
/// use bevy::prelude::*;
/// use bevy::render::RenderPlugin;
/// use bevy_dmabuf::wgpu_init::DmabufWgpuInitPlugin;
///
/// App::new()
///  .add_plugins(DefaultPlugins.build().add_before::<RenderPlugin>(DmabufWgpuInitPlugin));
/// ```
pub struct DmabufWgpuInitPlugin;

impl Plugin for DmabufWgpuInitPlugin {
    fn build(&self, app: &mut App) {
        if app.is_plugin_added::<RenderPlugin>() {
            panic!(
                "{} must be added before {}",
                type_name::<Self>(),
                type_name::<RenderPlugin>()
            );
        }

        let mut vulkan_settings = app
            .world_mut()
            .get_resource_or_init::<RawVulkanInitSettings>();
        unsafe {
            vulkan_settings.add_create_device_callback(|args, _adapter, _additional_features| {
                args.extensions.extend(REQUIRED_EXTENSIONS);
            })
        }
    }

    fn ready(&self, app: &App) -> bool {
        app.get_added_plugins::<RenderPlugin>()
            .first()
            .map(|render_plugin| render_plugin.ready(app))
            .unwrap_or(true)
    }

    fn cleanup(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app(RenderApp) {
            let backend = render_app.world().resource::<RenderAdapterInfo>().backend;
            if backend != Vulkan {
                warn!(
                    "This plugin only supports the Vulkan backend. Currently running with {backend}."
                );
            }
        } else {
            warn!("Render app not present. This plugin has no effect.");
        }
    }
}
