mod asset;
#[cfg(target_os = "linux")]
pub mod dmatex;
#[cfg(feature = "sampling")]
mod image;
#[cfg(feature = "render_target")]
mod render_target;

pub use asset::{ExternalBufferAssetLoader, ExternalBufferCreationData};
pub mod wgpu_init;
pub use wgpu_init::required_device_extensions;

use bevy::app::App;
pub struct ExternalBufferPlugin;

impl bevy::app::Plugin for ExternalBufferPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(asset::ExternalBufferAssetPlugin);
        #[cfg(feature = "sampling")]
        app.add_plugins(image::ExternalImagePlugin);
        #[cfg(feature = "render_target")]
        app.add_plugins(render_target::ExternalRenderTargetPlugin);
    }
}
