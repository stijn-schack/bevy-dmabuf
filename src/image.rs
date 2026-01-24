use crate::{
    asset::{ExternalBufferAssetPlugin, ExternalBufferUsage}, ExternalBufferImportFailed,
    ExternalBufferImported,
};
use bevy::{
    asset::AssetEventSystems,
    ecs::{
        system::lifetimeless::{SRes, SResMut},
        system::SystemParamItem,
    },
    platform::collections::HashMap,
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_resource::TextureUsages,
        texture::{DefaultImageSampler, GpuImage},
    },
};
use std::ops::Deref;

pub(crate) struct ExternalImagePlugin;

impl Plugin for ExternalImagePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExternalBufferAssetPlugin::<TextureSampling>::default())
            .init_resource::<ExternalBufferImageTable>()
            .add_systems(
                PostUpdate,
                sync_external_buffer_to_image_handle.after(AssetEventSystems),
            );
    }
}

#[derive(TypePath, Debug)]
pub struct TextureSampling {
    image_id: AssetId<Image>,
}

impl ExternalBufferUsage for TextureSampling {
    type PublicType = Handle<Image>;
    type MainParams = (SResMut<Assets<Image>>, SResMut<ExternalBufferImageTable>);
    type RenderParams = (SResMut<RenderAssets<GpuImage>>, SRes<DefaultImageSampler>);

    fn texture_usages() -> TextureUsages {
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC
    }

    fn init(
        buffer_handle: UntypedHandle,
        params: &mut SystemParamItem<Self::MainParams>,
    ) -> (Self, Self::PublicType) {
        let (images, external_buffer_image_table) = params;
        let image_handle = images.reserve_handle();
        external_buffer_image_table.insert(image_handle.id(), buffer_handle);
        (
            Self {
                image_id: image_handle.id(),
            },
            image_handle,
        )
    }

    fn on_buffer_imported(
        event: On<ExternalBufferImported<Self>>,
        params: &mut SystemParamItem<Self::RenderParams>,
    ) {
        let (gpu_images, default_sampler) = params;
        let ExternalBufferImported {
            asset_id,
            texture,
            view,
            usage_data,
        } = event.deref();

        let texture_format = texture.format();
        let size = texture.size();
        let mips = texture.mip_level_count();
        let sampler = (***default_sampler).clone();

        let gpu_image = GpuImage {
            texture: texture.clone(),
            texture_view: view.clone(),
            texture_format,
            texture_view_format: None,
            sampler,
            size,
            mip_level_count: mips,
            had_data: false,
        };

        gpu_images.insert(usage_data.image_id, gpu_image);

        debug!(
            "Set GpuImage for {}, backed by external buffer {}",
            usage_data.image_id, asset_id
        );
    }

    fn on_buffer_import_failed(
        event: On<ExternalBufferImportFailed<Self>>,
        params: &mut SystemParamItem<Self::MainParams>,
    ) {
        debug!("Received {event:?}, attempting to insert fallback image.",);
        let image_id = event.usage_data.image_id;
        let (images, image_table) = params;
        if let Err(err) = images.insert(image_id, Image::default()) {
            warn!(
                "Failed to insert fallback image with id {image_id} for failed buffer import {}. {err}",
                event.buffer_id
            );
        }
        image_table.remove(&event.usage_data.image_id);
    }
}

#[derive(Resource, Default, Deref, DerefMut)]
pub struct ExternalBufferImageTable(HashMap<AssetId<Image>, UntypedHandle>);

fn sync_external_buffer_to_image_handle(
    mut image_events: MessageReader<AssetEvent<Image>>,
    mut image_table: ResMut<ExternalBufferImageTable>,
) {
    for event in image_events.read() {
        match event {
            AssetEvent::Unused { id } | AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                if let Some(handle) = image_table.remove(id) {
                    debug!(
                        "Removed entry from ExternalBufferImageTable: ({}, {})",
                        id,
                        handle.id()
                    );
                }
            }
            _ => {}
        }
    }
}
