use crate::asset::*;
use bevy::{
    camera::{ManualTextureViewHandle, RenderTarget},
    ecs::system::SystemParam,
    prelude::*,
    render::RenderApp,
};

use render_world::*;

pub(crate) struct ExternalRenderTargetPlugin;

impl Plugin for ExternalRenderTargetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ManualTextureViewHandles>();

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(ExtractSchedule, sync_render_targets)
                .init_resource::<PendingExternalRenderTargets>();
            let render_world = render_app.world_mut();
            render_world.add_observer(add_pending_target_on_buffer_imported);
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct ExternalRenderTargetLoaderParams<'w> {
    manual_texture_view_handles: ResMut<'w, ManualTextureViewHandles>,
}

impl<'w> ExternalBufferAssetLoader<'w> {
    pub fn load_render_target(
        &mut self,
        creation_data: ExternalBufferCreationData,
    ) -> ExternalRenderTargetBundle {
        let params = &mut self.render_target_loader_params;

        let view_handle = params.manual_texture_view_handles.reserve_handle();
        let buffer_handle = self.add(ExternalBuffer {
            creation_data: Some(creation_data),
            usage: ExternalBufferUsage::RenderTarget(view_handle),
        });
        ExternalRenderTargetBundle::new(ExternalRenderTarget {
            _buffer_handle: buffer_handle,
            view_handle,
        })
    }
}

#[derive(Component, Debug)]
struct ExternalRenderTarget {
    _buffer_handle: Handle<ExternalBuffer>,
    pub(crate) view_handle: ManualTextureViewHandle,
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

mod render_world {
    use super::ExternalRenderTarget;
    use crate::asset::render_world::ExternalBufferImported;
    use crate::asset::ExternalBufferUsage;
    use bevy::{
        camera::{ManualTextureViewHandle, RenderTarget},
        platform::collections::HashMap,
        prelude::*,
        render::{texture::ManualTextureView, MainWorld},
    };
    use std::ops::Deref;

    #[derive(Resource, Default, Debug, Deref, DerefMut)]
    pub struct PendingExternalRenderTargets(HashMap<ManualTextureViewHandle, ManualTextureView>);

    pub fn add_pending_target_on_buffer_imported(
        event: On<ExternalBufferImported>,
        mut pending_targets: ResMut<PendingExternalRenderTargets>,
    ) {
        let ExternalBufferImported {
            asset_id: _,
            texture,
            view,
            usage,
        } = event.deref();

        let handle = match usage {
            ExternalBufferUsage::RenderTarget(handle) => *handle,
            _ => return,
        };

        let extent_3d = texture.size();
        let manual_view = ManualTextureView {
            texture_view: view.clone(),
            size: uvec2(extent_3d.width, extent_3d.height),
            view_format: texture.format(),
        };

        pending_targets.insert(handle, manual_view);
    }

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
