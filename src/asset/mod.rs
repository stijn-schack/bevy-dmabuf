use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    render::{render_asset::RenderAssetPlugin, texture::GpuImage},
};
use render_world::GpuExternalBuffer;
use std::fmt::Debug;

pub(crate) mod render_world;

pub(super) struct ExternalBufferAssetPlugin;

impl Plugin for ExternalBufferAssetPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<ExternalBuffer>()
            .add_plugins(RenderAssetPlugin::<GpuExternalBuffer, GpuImage>::default());
    }
}

#[derive(SystemParam)]
pub struct ExternalBufferAssetLoader<'w> {
    external_buffers: ResMut<'w, Assets<ExternalBuffer>>,
    #[cfg(feature = "sampling")]
    pub(crate) image_loader: crate::image::ExternalImageLoaderParams<'w>,
    #[cfg(feature = "render_target")]
    pub(crate) render_target_loader_params:
        crate::render_target::ExternalRenderTargetLoaderParams<'w>,
}

impl<'w> ExternalBufferAssetLoader<'w> {
    pub(crate) fn add(&mut self, buffer: ExternalBuffer) -> Handle<ExternalBuffer> {
        self.external_buffers.add(buffer)
    }
}

#[derive(Asset, TypePath, Debug)]
pub(crate) struct ExternalBuffer {
    pub creation_data: Option<ExternalBufferCreationData>,
    pub usage: ExternalBufferUsage,
}

#[derive(Debug)]
pub enum ExternalBufferCreationData {
    #[cfg(target_os = "linux")]
    Dmabuf { dma: crate::dmatex::Dmatex },
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum ExternalBufferUsage {
    #[cfg(feature = "sampling")]
    Sampling(AssetId<Image>),
    #[cfg(feature = "render_target")]
    RenderTarget(bevy::camera::ManualTextureViewHandle),
}
