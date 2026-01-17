use crate::dmatex::Dmatex;
use bevy::asset::AssetEventSystems;
use bevy::{
    ecs::system::SystemParam,
    platform::collections::HashMap,
    prelude::*,
    render::{render_asset::RenderAssetPlugin, texture::GpuImage, RenderApp},
};
use render_world::{AssetIdCache, GpuExternalImage};
use std::fmt::Debug;

mod render_world;

pub struct DmabufImportPlugin;

impl Plugin for DmabufImportPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<ExternalImage>()
            .init_resource::<ExternalImageCache>()
            .add_plugins(RenderAssetPlugin::<GpuExternalImage, GpuImage>::default())
            .add_systems(PostUpdate, remove_ext_image_on_handle_dropped.after(AssetEventSystems));

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<AssetIdCache>();
        }
    }
}

#[derive(SystemParam)]
pub struct ExternalImageAssetLoader<'w> {
    external_images: ResMut<'w, Assets<ExternalImage>>,
    images: ResMut<'w, Assets<Image>>,
    external_image_cache: ResMut<'w, ExternalImageCache>,
}

impl ExternalImageAssetLoader<'_> {
    pub fn load(&mut self, creation_data: ExternalImageCreationData) -> Result<Handle<Image>> {
        let img_handle = self.images.reserve_handle();
        let ext_handle = self.external_images.add(ExternalImage {
            creation_data: Some(creation_data),
            image_id: img_handle.id(),
        });

        self.external_image_cache
            .insert(img_handle.id(), ext_handle);
        Ok(img_handle)
    }
}

#[derive(Asset, TypePath, Debug)]
pub(crate) struct ExternalImage {
    pub creation_data: Option<ExternalImageCreationData>,
    pub image_id: AssetId<Image>,
}

#[derive(Debug)]
pub enum ExternalImageCreationData {
    #[cfg(target_os = "linux")]
    Dmabuf { dma: Dmatex, usage: DmatexUsage },
}

#[derive(Debug, Copy, Clone)]
pub enum DmatexUsage {
    Sampling,
    RenderTarget,
}

#[derive(Resource, Default, Deref, DerefMut)]
struct ExternalImageCache(HashMap<AssetId<Image>, Handle<ExternalImage>>);

fn remove_ext_image_on_handle_dropped(
    mut image_events: MessageReader<AssetEvent<Image>>,
    mut image_cache: ResMut<ExternalImageCache>,
) {
    for event in image_events.read() {
        if let AssetEvent::Unused { id } = event
            && let Some(handle) = image_cache.remove(id)
        {
            debug!(
                "Removed entry from ExternalImageCache: ({}, {})",
                id,
                handle.id()
            );
        }
    }
}
