use crate::dmatex::Dmatex;
use bevy::asset::AssetEventSystems;
use bevy::camera::{ManualTextureViewHandle, RenderTarget};
use bevy::ecs::component::{Components, ComponentsRegistrator};
use bevy::{
    ecs::system::SystemParam,
    platform::collections::HashMap,
    prelude::*,
    render::{render_asset::RenderAssetPlugin, texture::GpuImage, RenderApp},
};
use render_world::{
    sync_render_targets, AssetIdCache, GpuExternalImage, PendingExternalRenderTargets,
};
use std::fmt::Debug;

mod render_world;

pub struct DmabufImportPlugin;

impl Plugin for DmabufImportPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<ExternalImage>()
            .init_resource::<ExternalImageCache>()
            .init_resource::<ManualTextureViewHandles>()
            .add_plugins(RenderAssetPlugin::<GpuExternalImage, GpuImage>::default())
            .add_systems(
                PostUpdate,
                remove_ext_image_on_handle_dropped.after(AssetEventSystems),
            );

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(ExtractSchedule, sync_render_targets)
                .init_resource::<AssetIdCache>()
                .init_resource::<PendingExternalRenderTargets>();
        }
    }
}

#[derive(SystemParam)]
pub struct ExternalImageAssetLoader<'w> {
    external_images: ResMut<'w, Assets<ExternalImage>>,
    images: ResMut<'w, Assets<Image>>,
    external_image_cache: ResMut<'w, ExternalImageCache>,
    manual_texture_view_handles: ResMut<'w, ManualTextureViewHandles>,
}

impl ExternalImageAssetLoader<'_> {
    pub fn load_texture(&mut self, creation_data: ExternalImageCreationData) -> Handle<Image> {
        let img_handle = self.images.reserve_handle();
        let ext_handle = self.external_images.add(ExternalImage {
            creation_data: Some(creation_data),
            usage: ExternalImageUsage::Sampling(img_handle.id()),
        });

        self.external_image_cache
            .insert(img_handle.id(), ext_handle);
        img_handle
    }

    pub fn load_render_target(
        &mut self,
        creation_data: ExternalImageCreationData,
    ) -> ExternalRenderTargetBundle {
        let view_handle = self.manual_texture_view_handles.reserve_handle();
        let image_handle = self.external_images.add(ExternalImage {
            creation_data: Some(creation_data),
            usage: ExternalImageUsage::RenderTarget(view_handle),
        });
        ExternalRenderTargetBundle::new(ExternalRenderTarget {
            image_handle,
            view_handle,
        })
    }
}

#[derive(Asset, TypePath, Debug)]
pub(crate) struct ExternalImage {
    pub creation_data: Option<ExternalImageCreationData>,
    pub usage: ExternalImageUsage,
}

#[derive(Debug)]
pub enum ExternalImageCreationData {
    #[cfg(target_os = "linux")]
    Dmabuf { dma: Dmatex },
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum ExternalImageUsage {
    Sampling(AssetId<Image>),
    RenderTarget(ManualTextureViewHandle),
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

#[derive(Component, Debug)]
pub struct ExternalRenderTarget {
    image_handle: Handle<ExternalImage>,
    view_handle: ManualTextureViewHandle,
}

#[derive(Bundle, Debug)]
pub struct ExternalRenderTargetBundle {
    external_target: ExternalRenderTarget,
    render_target: RenderTarget,
    pub camera: Camera,
}

impl ExternalRenderTargetBundle {
    fn new(external_target: ExternalRenderTarget) -> Self {
        Self {
            external_target,
            render_target: RenderTarget::None { size: UVec2::ZERO },
            camera: Camera::default(),
        }
    }
}

#[derive(Resource, Default)]
struct ManualTextureViewHandles {
    next_id: u32,
}

impl ManualTextureViewHandles {
    fn reserve_handle(&mut self) -> ManualTextureViewHandle {
        let handle = ManualTextureViewHandle(self.next_id);
        self.next_id += 1;
        handle
    }
}
