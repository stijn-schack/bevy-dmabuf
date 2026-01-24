mod asset;
pub use asset::{
    render_world::ExternalBufferImportFailed, render_world::ExternalBufferImported, ExternalBufferCreationData,
    ExternalBufferLoader, ExternalBufferUsage,
};

#[cfg(target_os = "linux")]
pub mod dmatex;
#[cfg(feature = "sampling")]
mod image;
#[cfg(feature = "sampling")]
pub use image::TextureSampling;

#[cfg(feature = "render_target")]
mod render_target;
#[cfg(feature = "render_target")]
pub use render_target::CameraRenderTarget;

pub mod wgpu_init;
pub use wgpu_init::required_device_extensions;

use bevy::app::App;
pub struct ExternalBufferPlugin;

impl bevy::app::Plugin for ExternalBufferPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "sampling")]
        app.add_plugins(image::ExternalImagePlugin);
        #[cfg(feature = "render_target")]
        app.add_plugins(render_target::ExternalRenderTargetPlugin);
    }
}
