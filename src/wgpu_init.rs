use bevy::{
    app::{
        App,
        Plugin,
        PluginGroup,
        PluginGroupBuilder,
    },
    log::warn,
    render::{
        renderer::{
            raw_vulkan_init::RawVulkanInitSettings,
            RenderAdapterInfo,
        },
        settings::{Backends, RenderCreation, WgpuSettings},
        RenderApp,
        RenderPlugin,
    },
    utils::default,
};
use std::any::type_name;
use std::ffi::CStr;
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
pub struct DmabufWgpuInitPlugin;

pub fn add_dmabuf_init_plugin<G: PluginGroup>(plugins: G) -> PluginGroupBuilder {
    plugins
        .build()
        .disable::<RenderPlugin>()
        .add_before::<RenderPlugin>(DmabufWgpuInitPlugin)
}

impl Plugin for DmabufWgpuInitPlugin {
    fn build(&self, app: &mut App) {
        if app.is_plugin_added::<RenderPlugin>() {
            panic!(
                "{} must be added before {}",
                type_name::<Self>(),
                type_name::<RenderPlugin>()
            );
        }

        let mut vulkan_settings = RawVulkanInitSettings::default();
        unsafe {
            vulkan_settings.add_create_device_callback(|args, _adapter, _additional_features| {
                args.extensions.extend(REQUIRED_EXTENSIONS);
            })
        }
        app.insert_resource(vulkan_settings)
            .add_plugins(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    backends: Some(Backends::VULKAN),
                    ..default()
                }),
                ..default()
            });
    }

    fn ready(&self, app: &App) -> bool {
        app.get_added_plugins::<RenderPlugin>().first()
            .map(|render_plugin| render_plugin.ready(app))
            .unwrap_or(true)
    }

    fn cleanup(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app(RenderApp) {
            let backend = render_app.world().resource::<RenderAdapterInfo>().backend;
            if backend != Vulkan {
                warn!(
                    "{} only supports the wgpu Vulkan backend. Currently running with {backend}",
                    type_name::<Self>()
                );
            }
        } else {
            warn!(
                "Added {0} but the RenderApp is not present. Either remove {0} or add {1}",
                type_name::<Self>(),
                type_name::<RenderPlugin>()
            )
        }
    }
}
