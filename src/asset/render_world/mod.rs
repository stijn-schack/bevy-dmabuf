use super::{ExternalBuffer, ExternalBufferCreationData, ExternalBufferUsage};
use ash::vk;
use bevy::ecs::system::lifetimeless::SCommands;
use bevy::{
    asset::{AssetId, RenderAssetUsages},
    ecs::system::{lifetimeless::SRes, SystemParamItem},
    prelude::*,
    render::{
        render_asset::{AssetExtractionError, PrepareAssetError, RenderAsset},
        render_resource::{Texture, TextureView},
        renderer::RenderDevice,
    },
};
use thiserror::Error;

mod hal;

#[derive(Debug)]
#[allow(unused)]
pub(super) enum GpuExternalBuffer {
    Imported {
        texture: Texture,
        view: TextureView,
        usage: ExternalBufferUsage,
    },
    Invalid(ImportError),
}

#[derive(Debug)]
struct ImportedTexture {
    texture: Texture,
    texture_view: TextureView,
}

#[derive(Event, Debug)]
pub(crate) struct ExternalBufferImported {
    pub asset_id: AssetId<ExternalBuffer>,
    pub texture: Texture,
    pub view: TextureView,
    pub usage: ExternalBufferUsage,
}

#[derive(Event, Debug)]
pub(crate) struct ExternalBufferImportFailed {
    pub asset_id: AssetId<ExternalBuffer>,
}

impl RenderAsset for GpuExternalBuffer {
    type SourceAsset = ExternalBuffer;
    type Param = (SRes<RenderDevice>, SCommands);

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
        _previous_asset: Option<&Self>,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        debug_assert!(
            _previous_asset.is_none(),
            "External images have immutable descriptions. Do not attempt to change the internals representations directly. If a new import is wanted, simply add another ExternalImage asset."
        );

        trace!("Trying to prepare render asset {asset_id}");

        let (render_device, commands) = params;

        let ExternalBufferCreationData::Dmabuf { dma } = source_asset.creation_data.unwrap();

        let import_result =
            hal::import_dmabuf_as_texture(render_device.wgpu_device(), dma, source_asset.usage);

        let gpu_buffer = match import_result {
            Ok(ImportedTexture {
                   texture,
                   texture_view: view,
               }) => {
                commands.trigger(ExternalBufferImported {
                    asset_id,
                    texture: texture.clone(),
                    view: view.clone(),
                    usage: source_asset.usage,
                });
                GpuExternalBuffer::Imported {
                    texture,
                    view,
                    usage: source_asset.usage,
                }
            }
            Err(err) => {
                error!("Failed to import external image {}: {}", asset_id, err);
                commands.trigger(ExternalBufferImportFailed { asset_id });
                GpuExternalBuffer::Invalid(err)
            }
        };

        Ok(gpu_buffer)
    }

    fn unload_asset(
        asset_id: AssetId<Self::SourceAsset>,
        _param: &mut SystemParamItem<Self::Param>,
    ) {
        trace!("unloaded GpuExternalBuffer {}", asset_id);
    }

    fn take_gpu_data(
        source: &mut Self::SourceAsset,
        previous_gpu_asset: Option<&Self>,
    ) -> Result<Self::SourceAsset, AssetExtractionError> {
        if previous_gpu_asset.is_some() {
            Err(AssetExtractionError::AlreadyExtracted)
        } else {
            let creation_data = source
                .creation_data
                .take()
                .ok_or(AssetExtractionError::AlreadyExtracted)?;

            Ok(ExternalBuffer {
                creation_data: Some(creation_data),
                usage: source.usage,
            })
        }
    }
}

impl Clone for ExternalBuffer {
    /// ExternalBuffers should not be cloned.
    /// The render world needs exclusive access to underlying file descriptors.
    /// However, to implement [RenderAsset], [RenderAsset::SourceAsset] needs to implement [Clone].
    /// This is a workaround to ensure no data is actually cloned, while still adhering to the trait bounds.
    fn clone(&self) -> Self {
        #[cfg(debug_assertions)]
        unreachable!(
            "Clone implementation needed to satisfy RenderAsset trait bounds. However, ExternalBuffer should never be cloned."
        );
        #[cfg(not(debug_assertions))]
        Self {
            creation_data: None,
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
