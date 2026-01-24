use super::{ExternalBuffer, ExternalBufferCreationData, ExternalBufferUsage};
use ash::vk;
use bevy::{
    asset::{AssetId, RenderAssetUsages, UntypedAssetId},
    ecs::system::{
        lifetimeless::{SCommands, SRes, SResMut},
        SystemParamItem,
    },
    platform::collections::HashMap,
    prelude::*,
    render::{
        erased_render_asset::{ErasedRenderAsset, PrepareAssetError},
        render_resource::{Texture, TextureView},
        renderer::RenderDevice,
        MainWorld,
    },
};
use thiserror::Error;

mod hal;

#[derive(Debug)]
#[allow(unused)]
pub(super) enum GpuExternalBuffer {
    Imported(ImportedTexture),
    Invalid(ImportError),
}

#[derive(Debug)]
pub(super) struct ImportedTexture {
    texture: Texture,
    view: TextureView,
}

#[derive(Event, Debug)]
pub struct ExternalBufferImported<U: ExternalBufferUsage> {
    pub asset_id: UntypedAssetId,
    pub texture: Texture,
    pub view: TextureView,
    pub usage_data: U,
}

#[derive(Event, Message, Debug)]
pub struct ExternalBufferImportFailed<U> {
    pub buffer_id: UntypedAssetId,
    pub usage_data: U,
}

impl<U: ExternalBufferUsage> ErasedRenderAsset for ExternalBuffer<U> {
    type SourceAsset = Self;
    type ErasedAsset = GpuExternalBuffer;
    type Param = (SRes<RenderDevice>, SCommands, SResMut<FailedImports<U>>);

    fn asset_usage(_: &Self::SourceAsset) -> RenderAssetUsages {
        RenderAssetUsages::RENDER_WORLD
    }

    fn byte_len(source_asset: &Self::SourceAsset) -> Option<usize> {
        debug_assert!(source_asset.creation_data.is_some());
        source_asset.creation_data.as_ref().map(|data| match data {
            ExternalBufferCreationData::Dmabuf { dma, .. } => {
                dma.res.x as usize * dma.res.y as usize
            }
        })
    }

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        asset_id: AssetId<Self::SourceAsset>,
        params: &mut SystemParamItem<Self::Param>,
    ) -> std::result::Result<Self::ErasedAsset, PrepareAssetError<Self::SourceAsset>> {
        trace!("Trying to prepare render asset {asset_id}");

        let (render_device, commands, failed_imports) = params;

        let ExternalBufferCreationData::Dmabuf { dma } = source_asset.creation_data.unwrap();

        let import_result =
            hal::import_dmabuf_as_texture(render_device.wgpu_device(), dma, U::texture_usages());

        let gpu_buffer = match import_result {
            Ok(imported) => {
                commands.trigger(ExternalBufferImported {
                    asset_id: asset_id.untyped(),
                    texture: imported.texture.clone(),
                    view: imported.view.clone(),
                    usage_data: source_asset.usage,
                });
                GpuExternalBuffer::Imported(imported)
            }
            Err(err) => {
                error!("Failed to import external buffer {}: {}", asset_id, err);
                failed_imports.insert(asset_id, source_asset.usage);
                GpuExternalBuffer::Invalid(err)
            }
        };

        Ok(gpu_buffer)
    }

    fn unload_asset(
        asset_id: AssetId<Self::SourceAsset>,
        _param: &mut SystemParamItem<Self::Param>,
    ) {
        trace!("Unloaded external buffer {}", asset_id);
    }
}

#[derive(Resource, Debug, Deref, DerefMut)]
pub(super) struct FailedImports<U: ExternalBufferUsage>(HashMap<AssetId<ExternalBuffer<U>>, U>);

impl<U: ExternalBufferUsage> Default for FailedImports<U> {
    fn default() -> Self {
        Self(default())
    }
}
pub(super) fn trigger_failed_import_events<U: ExternalBufferUsage>(
    mut main_world: ResMut<MainWorld>,
    mut failed_imports: ResMut<FailedImports<U>>,
) {
    failed_imports
        .drain()
        .map(|(asset_id, usage_data)| ExternalBufferImportFailed {
            buffer_id: asset_id.untyped(),
            usage_data,
        })
        .for_each(|event| main_world.trigger(event));
}

impl<U: ExternalBufferUsage> Clone for ExternalBuffer<U> {
    /// ExternalBuffers should not be cloned.
    /// The render world needs exclusive access to underlying file descriptors.
    /// However, to implement [RenderAsset], [RenderAsset::SourceAsset] needs to implement [Clone].
    /// This is a workaround to ensure no data is actually cloned, while still adhering to the trait bounds.
    fn clone(&self) -> Self {
        #[cfg(debug_assertions)]
        unreachable!(
            "Clone implementation is needed to satisfy RenderAsset trait bounds. However, ExternalBuffer should never be cloned."
        );
        #[cfg(not(debug_assertions))]
        Self {
            creation_data: None,
            usage: self.usage,
        }
    }
}

#[derive(Error, Debug)]
pub enum ImportError {
    #[error("Format is not compatible with Vulkan")]
    VulkanIncompatibleFormat,
    #[error("Format is not compatible with Wgpu")]
    WgpuIncompatibleFormat,
    #[error("Unsupported Modifier for Format")]
    ModifierInvalid,
    #[error("Unable to create Vulkan Image: {0}")]
    VulkanImageCreationFailed(vk::Result),
    #[error("Unrecognized Fourcc/Format")]
    UnrecognizedFourcc(#[from] drm_fourcc::UnrecognizedFourcc),
    #[error("RenderDevice is not a Vulkan Device")]
    NotVulkan,
    #[error("Unable to find valid Gpu Memory type index")]
    NoValidMemoryTypes,
    #[error("Unable to allocate Vulkan Gpu Memory: {0}")]
    VulkanMemoryAllocFailed(vk::Result),
    #[error("Unable to bind Vulkan Gpu Memory to Vulkan Image: {0}")]
    VulkanImageMemoryBindFailed(vk::Result),
    #[error(
        "The number of DmaTex planes does not equal the number of planes defined by the drm modifier"
    )]
    IncorrectNumberOfPlanes,
    #[error("No Planes to Import")]
    NoPlanes,
}
