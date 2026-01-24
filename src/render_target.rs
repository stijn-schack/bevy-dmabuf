use crate::asset::render_world::ExternalBufferImported;
use crate::asset::*;
use bevy::ecs::system::lifetimeless::SResMut;
use bevy::ecs::system::SystemParamItem;
use bevy::render::render_resource::TextureUsages;
use bevy::render::texture::ManualTextureView;
use bevy::{
    camera::{ManualTextureViewHandle, RenderTarget},
    prelude::*,
    render::RenderApp,
};
use render_world::*;
use std::ops::Deref;

pub(crate) struct ExternalRenderTargetPlugin;

impl Plugin for ExternalRenderTargetPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExternalBufferAssetPlugin::<CameraRenderTarget>::default())
            .init_resource::<ManualTextureViewHandles>();

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(ExtractSchedule, sync_render_targets)
                .init_resource::<PendingExternalRenderTargets>();
        }
    }
}

#[derive(Component, Debug)]
pub struct ExternalRenderTarget {
    _buffer_handle: UntypedHandle,
    view_handle: ManualTextureViewHandle,
}

#[derive(TypePath, Debug)]
pub struct CameraRenderTarget {
    view_handle: ManualTextureViewHandle,
}

impl ExternalBufferUsage for CameraRenderTarget {
    type PublicType = ExternalRenderTargetBundle;
    type MainParams = SResMut<ManualTextureViewHandles>;
    type RenderParams = SResMut<PendingExternalRenderTargets>;

    fn texture_usages() -> TextureUsages {
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_DST
    }

    fn init(
        buffer_handle: UntypedHandle,
        params: &mut SystemParamItem<Self::MainParams>,
    ) -> (Self, Self::PublicType) {
        let texture_view_handles = params;
        let view_handle = texture_view_handles.reserve_handle();
        (
            Self { view_handle },
            ExternalRenderTargetBundle::new(buffer_handle, view_handle),
        )
    }

    fn on_buffer_imported(
        event: On<ExternalBufferImported<Self>>,
        params: &mut SystemParamItem<Self::RenderParams>,
    ) {
        let pending_targets = params;
        let ExternalBufferImported {
            asset_id: _,
            texture,
            view,
            usage_data,
        } = event.deref();

        let extent_3d = texture.size();
        let manual_view = ManualTextureView {
            texture_view: view.clone(),
            size: uvec2(extent_3d.width, extent_3d.height),
            view_format: texture.format(),
        };

        pending_targets.insert(usage_data.view_handle, manual_view);
    }
}

#[derive(Bundle, Debug)]
pub struct ExternalRenderTargetBundle {
    external_target: ExternalRenderTarget,
    render_target: RenderTarget,
}

impl ExternalRenderTargetBundle {
    fn new(buffer_handle: UntypedHandle, view_handle: ManualTextureViewHandle) -> Self {
        Self {
            external_target: ExternalRenderTarget {
                _buffer_handle: buffer_handle,
                view_handle,
            },
            render_target: RenderTarget::None { size: UVec2::ZERO },
        }
    }
}

#[derive(Resource, Default)]
pub struct ManualTextureViewHandles {
    next_id: u32,
}

impl ManualTextureViewHandles {
    fn reserve_handle(&mut self) -> ManualTextureViewHandle {
        let handle = ManualTextureViewHandle(self.next_id);
        self.next_id += 1;
        handle
    }
}

mod render_world {
    use crate::render_target::ExternalRenderTarget;
    use bevy::{
        camera::{ManualTextureViewHandle, RenderTarget},
        platform::collections::HashMap,
        prelude::*,
        render::{texture::ManualTextureView, MainWorld},
    };

    #[derive(Resource, Default, Debug, Deref, DerefMut)]
    pub struct PendingExternalRenderTargets(HashMap<ManualTextureViewHandle, ManualTextureView>);

    pub fn sync_render_targets(
        mut main_world: ResMut<MainWorld>,
        mut pending: ResMut<PendingExternalRenderTargets>,
        mut cache: Local<Vec<(Entity, ManualTextureViewHandle, ManualTextureView)>>,
    ) {
        let mut empty_render_targets = main_world.query::<(Entity, &ExternalRenderTarget)>();
        for (entity, target) in empty_render_targets.iter(&main_world) {
            if let Some(texture_view) = pending.remove(&target.view_handle) {
                trace!(
                    "Preparing RenderTarget::ManualTextureView({:?}) insertion for {}",
                    target.view_handle, entity
                );
                cache.push((entity, target.view_handle, texture_view));
            }
        }

        main_world.resource_scope::<ManualTextureViews, ()>(|main_world, mut texture_views| {
            for (entity, handle, texture_view) in cache.drain(..) {
                texture_views.insert(handle, texture_view);
                let render_target = RenderTarget::TextureView(handle);
                debug!("Inserting {:?} for entity {}", &render_target, entity);
                main_world.entity_mut(entity).insert(render_target);
            }
        });

        cache.clear();
    }
}
