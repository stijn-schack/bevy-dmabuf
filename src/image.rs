use crate::asset::{
    ExternalBuffer, ExternalBufferAssetLoader, ExternalBufferCreationData, ExternalBufferUsage,
};
use bevy::{
    asset::AssetEventSystems, ecs::system::SystemParam, platform::collections::HashMap, prelude::*,
    render::RenderApp,
};

pub(crate) struct ExternalImagePlugin;

impl Plugin for ExternalImagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExternalBufferImageTable>().add_systems(
            PostUpdate,
            remove_external_buffer_when_image_handle_dropped.after(AssetEventSystems),
        );

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .world_mut()
                .add_observer(render_world::handle_buffer_imported_event);
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct ExternalImageLoaderParams<'w> {
    images: ResMut<'w, Assets<Image>>,
    image_table: ResMut<'w, ExternalBufferImageTable>,
}

impl ExternalImageLoaderParams<'_> {
    fn reserve_handle(&mut self) -> Handle<Image> {
        self.images.reserve_handle()
    }

    fn register_handles(
        &mut self,
        image_id: AssetId<Image>,
        buffer_handle: Handle<ExternalBuffer>,
    ) {
        self.image_table.insert(image_id, buffer_handle);
    }
}

impl<'w> ExternalBufferAssetLoader<'w> {
    pub fn load_as_image(&mut self, creation_data: ExternalBufferCreationData) -> Handle<Image> {
        let image_handle = self.image_loader.reserve_handle();

        let buffer_handle = self.add(ExternalBuffer {
            creation_data: Some(creation_data),
            usage: ExternalBufferUsage::Sampling(image_handle.id()),
        });

        self.image_loader
            .register_handles(image_handle.id(), buffer_handle);
        image_handle
    }
}

#[derive(Resource, Default, Deref, DerefMut)]
struct ExternalBufferImageTable(HashMap<AssetId<Image>, Handle<ExternalBuffer>>);

fn remove_external_buffer_when_image_handle_dropped(
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

pub(crate) mod render_world {
    use crate::asset::{render_world::ExternalBufferImported, ExternalBufferUsage};
    use bevy::{
        prelude::*,
        render::render_asset::RenderAssets,
        render::texture::{DefaultImageSampler, GpuImage},
    };
    use std::ops::Deref;

    pub fn handle_buffer_imported_event(
        event: On<ExternalBufferImported>,
        mut gpu_images: ResMut<RenderAssets<GpuImage>>,
        default_sampler: Res<DefaultImageSampler>,
    ) {
        let ExternalBufferImported {
            asset_id,
            texture,
            view,
            usage,
        } = event.deref();

        let image_id = match usage {
            ExternalBufferUsage::Sampling(image_id) => *image_id,
            _ => return,
        };

        let texture_format = texture.format();
        let size = texture.size();
        let mips = texture.mip_level_count();
        let sampler = (**default_sampler).clone();

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

        gpu_images.insert(image_id, gpu_image);

        debug!(
            "Set GpuImage for {}, backed by external buffer {}",
            image_id, asset_id
        );
    }
}
