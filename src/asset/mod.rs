use crate::asset::render_world::{ExternalBufferImportFailed, ExternalBufferImported};
use bevy::{
    ecs::system::{StaticSystemParam, SystemParam, SystemParamItem},
    prelude::*,
    render::erased_render_asset::{ErasedRenderAssetDependency, ErasedRenderAssetPlugin},
    render::render_resource::TextureUsages,
    render::RenderApp,
};
use std::fmt::Debug;
use std::marker::PhantomData;

pub(crate) mod render_world;

pub(super) struct ExternalBufferAssetPlugin<
    U: ExternalBufferUsage,
    AFTER: ErasedRenderAssetDependency + 'static = (),
> {
    phantom: PhantomData<fn() -> (U, AFTER)>,
}

impl<U: ExternalBufferUsage, AFTER: ErasedRenderAssetDependency + 'static> Default
for ExternalBufferAssetPlugin<U, AFTER>
{
    fn default() -> Self {
        Self { phantom: default() }
    }
}

impl<U: ExternalBufferUsage, AFTER: ErasedRenderAssetDependency + 'static> Plugin
for ExternalBufferAssetPlugin<U, AFTER>
{
    fn build(&self, app: &mut App) {
        app.init_asset::<ExternalBuffer<U>>()
            .add_plugins(ErasedRenderAssetPlugin::<ExternalBuffer<U>, AFTER>::default())
            .add_observer(on_buffer_import_failed::<U>);

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<render_world::FailedImports<U>>()
                .add_systems(
                    ExtractSchedule,
                    render_world::trigger_failed_import_events::<U>,
                );
            render_app.world_mut().add_observer(on_buffer_imported::<U>);
        }
    }
}

#[derive(SystemParam)]
pub struct ExternalBufferLoader<'w, 's, U: ExternalBufferUsage> {
    external_buffers: ResMut<'w, Assets<ExternalBuffer<U>>>,
    params: StaticSystemParam<'w, 's, <U as ExternalBufferUsage>::MainParams>,
}

impl<'w, 's, U: ExternalBufferUsage> ExternalBufferLoader<'w, 's, U> {
    pub fn load(&mut self, creation_data: ExternalBufferCreationData) -> U::PublicType {
        let buffer_handle = self.external_buffers.reserve_handle();
        let (usage_data, public_representation) =
            U::init(buffer_handle.clone().untyped(), &mut self.params);
        self.external_buffers
            .insert(
                buffer_handle.id(),
                ExternalBuffer::new(creation_data, usage_data),
            )
            .unwrap();
        public_representation
    }
}

#[derive(Asset, TypePath, Debug)]
struct ExternalBuffer<U: ExternalBufferUsage> {
    pub creation_data: Option<ExternalBufferCreationData>,
    usage: U,
}

impl<U: ExternalBufferUsage> ExternalBuffer<U> {
    pub fn new(creation_data: ExternalBufferCreationData, usage: U) -> Self {
        Self {
            creation_data: Some(creation_data),
            usage,
        }
    }
}

#[derive(Debug, TypePath)]
pub enum ExternalBufferCreationData {
    #[cfg(target_os = "linux")]
    Dmabuf { dma: crate::dmatex::Dmatex },
}

pub trait ExternalBufferUsage: Sized + Send + Sync + TypePath {
    type PublicType;
    type MainParams: SystemParam;
    type RenderParams: SystemParam;
    fn texture_usages() -> TextureUsages;

    fn init(
        buffer_handle: UntypedHandle,
        params: &mut SystemParamItem<Self::MainParams>,
    ) -> (Self, Self::PublicType);

    /// [Observer] system that is runs in the render world, right after the buffer is imported successfully.
    fn on_buffer_imported(
        event: On<ExternalBufferImported<Self>>,
        params: &mut SystemParamItem<Self::RenderParams>,
    );

    /// [Observer] system that runs in the main world, triggered after a failed buffer import.
    /// Intended to perform usage specific cleanup and/or provide fallbacks after a failed buffer import.
    fn on_buffer_import_failed(
        event: On<ExternalBufferImportFailed<Self>>,
        params: &mut SystemParamItem<Self::MainParams>,
    ) {
        let _ = (event, params);
    }
}

fn on_buffer_imported<U: ExternalBufferUsage>(
    event: On<ExternalBufferImported<U>>,
    params: StaticSystemParam<U::RenderParams>,
) {
    let mut params = params.into_inner();
    U::on_buffer_imported(event, &mut params)
}

fn on_buffer_import_failed<U: ExternalBufferUsage>(
    event: On<ExternalBufferImportFailed<U>>,
    params: StaticSystemParam<U::MainParams>,
) {
    let mut params = params.into_inner();
    U::on_buffer_import_failed(event, &mut params);
}
