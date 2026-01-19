use crate::dmatex::Dmatex;
use bevy::{
    asset::AssetEventSystems,
    camera::{ManualTextureViewHandle, RenderTarget},
    ecs::system::SystemParam,
    platform::collections::HashMap,
    prelude::*,
    render::{render_asset::RenderAssetPlugin, texture::GpuImage, RenderApp},
};
use render_world::{
    sync_render_targets, AssetIdCache, GpuExternalBuffer, PendingExternalRenderTargets,
};
use std::fmt::Debug;

mod render_world;

pub struct ExternalBufferPlugin;

impl Plugin for ExternalBufferPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<ExternalBuffer>()
            .init_resource::<ExternalBufferImageTable>()
            .init_resource::<ManualTextureViewHandles>()
            .add_plugins(RenderAssetPlugin::<GpuExternalBuffer, GpuImage>::default())
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
pub struct ExternalBufferAssetLoader<'w> {
    external_images: ResMut<'w, Assets<ExternalBuffer>>,
    images: ResMut<'w, Assets<Image>>,
    image_table: ResMut<'w, ExternalBufferImageTable>,
    manual_texture_view_handles: ResMut<'w, ManualTextureViewHandles>,
}

impl ExternalBufferAssetLoader<'_> {
    pub fn load_texture(&mut self, creation_data: ExternalBufferCreationData) -> Handle<Image> {
        let img_handle = self.images.reserve_handle();
        let ext_handle = self.external_images.add(ExternalBuffer {
            creation_data: Some(creation_data),
            usage: ExternalBufferUsage::Sampling(img_handle.id()),
        });

        self.image_table
            .insert(img_handle.id(), ext_handle);
        img_handle
    }

    pub fn load_render_target(
        &mut self,
        creation_data: ExternalBufferCreationData,
    ) -> ExternalRenderTargetBundle {
        let view_handle = self.manual_texture_view_handles.reserve_handle();
        let buffer_handle = self.external_images.add(ExternalBuffer {
            creation_data: Some(creation_data),
            usage: ExternalBufferUsage::RenderTarget(view_handle),
        });
        ExternalRenderTargetBundle::new(ExternalRenderTarget {
            _buffer_handle: buffer_handle,
            view_handle,
        })
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
    Dmabuf { dma: Dmatex },
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum ExternalBufferUsage {
    Sampling(AssetId<Image>),
    RenderTarget(ManualTextureViewHandle),
}

#[derive(Resource, Default, Deref, DerefMut)]
struct ExternalBufferImageTable(HashMap<AssetId<Image>, Handle<ExternalBuffer>>);

fn remove_ext_image_on_handle_dropped(
    mut image_events: MessageReader<AssetEvent<Image>>,
    mut image_table: ResMut<ExternalBufferImageTable>,
) {
    for event in image_events.read() {
        if let AssetEvent::Unused { id } = event
            && let Some(handle) = image_table.remove(id)
        {
            debug!(
                "Removed entry from ExternalBufferImageTable: ({}, {})",
                id,
                handle.id()
            );
        }
    }
}

#[derive(Component, Debug)]
pub struct ExternalRenderTarget {
    _buffer_handle: Handle<ExternalBuffer>,
    view_handle: ManualTextureViewHandle,
}

#[derive(Bundle, Debug)]
pub struct ExternalRenderTargetBundle {
    external_target: ExternalRenderTarget,
    render_target: RenderTarget,
}

impl ExternalRenderTargetBundle {
    fn new(external_target: ExternalRenderTarget) -> Self {
        Self {
            external_target,
            render_target: RenderTarget::None { size: UVec2::ZERO },
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
