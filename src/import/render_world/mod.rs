use super::{ExternalImage, ExternalImageCreationData};
use ash::vk;
use bevy::platform::collections::HashMap;
use bevy::{
    asset::{AssetId, RenderAssetUsages},
    ecs::system::lifetimeless::SResMut,
    ecs::system::{lifetimeless::SRes, SystemParamItem},
    log::debug,
    prelude::*,
    render::{
        render_asset::{AssetExtractionError, PrepareAssetError, RenderAsset, RenderAssets},
        render_resource::{Texture, TextureView},
        renderer::RenderDevice,
        texture::{DefaultImageSampler, GpuImage},
    },
};
use thiserror::Error;

pub mod hal;
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum GpuExternalImage {
    Imported(GpuImage),
    Invalid(ImportError),
}

impl RenderAsset for GpuExternalImage {
    type SourceAsset = ExternalImage;
    type Param = (
        SRes<RenderDevice>,
        SRes<DefaultImageSampler>,
        SResMut<RenderAssets<GpuImage>>,
        SResMut<AssetIdCache>,
    );

    fn asset_usage(_: &Self::SourceAsset) -> RenderAssetUsages {
        RenderAssetUsages::RENDER_WORLD
    }

    fn byte_len(source_asset: &Self::SourceAsset) -> Option<usize> {
        debug_assert!(source_asset.creation_data.is_some());
        source_asset.creation_data.as_ref().map(|data| match data {
            ExternalImageCreationData::Dmabuf { dma, .. } => {
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

        debug!(
            "Trying to prepare render asset for {} {}",
            asset_id, source_asset.image_id
        );

        let (render_device, default_sampler, gpu_images, asset_ids) = params;

        let ExternalImageCreationData::Dmabuf { dma, usage } = source_asset.creation_data.unwrap();
        debug!("Importing external texture into render context");
        let import_result = hal::import_dmabuf_as_texture(render_device.wgpu_device(), dma, usage);
        let gpu_ext_img = match import_result {
            Ok(img) => {
                let texture_format = img.texture.format();
                let size = img.texture.size();
                let mips = img.texture.mip_level_count();
                Ok(Self::Imported(GpuImage {
                    texture: img.texture,
                    texture_view: img.texture_view,
                    texture_format,
                    texture_view_format: None,
                    sampler: (***default_sampler).clone(),
                    size,
                    mip_level_count: mips,
                    had_data: false,
                }))
            }
            Err(err) => Ok(Self::Invalid(err)),
        }?;

        match &gpu_ext_img {
            GpuExternalImage::Imported(imported_gpu_img) => {
                debug!("GpuImage successfully imported for {}", asset_id);
                gpu_images.insert(source_asset.image_id, imported_gpu_img.clone());
                asset_ids.insert(asset_id, source_asset.image_id);
            }
            GpuExternalImage::Invalid(err) => {
                error!("Failed to import external image {}: {}", asset_id, err)
            }
        }

        Ok(gpu_ext_img)
    }

    fn unload_asset(
        source_asset: AssetId<Self::SourceAsset>,
        param: &mut SystemParamItem<Self::Param>,
    ) {
        let (.., gpu_images, asset_ids) = param;
        if let Some(img_id) = asset_ids.get(&source_asset).copied() {
            gpu_images.remove(img_id);
            debug!("Removed GpuImage for {}", source_asset)
        }
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

            Ok(ExternalImage {
                creation_data: Some(creation_data),
                image_id: source.image_id,
            })
        }
    }
}

#[derive(Debug)]
pub struct ImportedTexture {
    texture: Texture,
    texture_view: TextureView,
}

#[derive(Resource, Debug, Default, Deref, DerefMut)]
pub(crate) struct AssetIdCache(HashMap<AssetId<ExternalImage>, AssetId<Image>>);

impl Clone for ExternalImage {
    /// ExternalImages should not be cloned.
    /// The render world needs exclusive access to underlying file descriptors.
    /// However, to implement [RenderAsset], [RenderAsset::SourceAsset] needs to implement [Clone].
    /// This is a workaround to ensure no data is actually cloned, while still adhering to the trait bounds.
    fn clone(&self) -> Self {
        #[cfg(debug_assertions)]
        unimplemented!(
            "Clone implementation needed to satisfy RenderAsset trait bounds. However, ExternalImage should never be cloned."
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
