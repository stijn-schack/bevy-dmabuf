use super::{ExternalBuffer, ExternalBufferCreationData, ExternalBufferUsage, ExternalRenderTarget};
use ash::vk;
use bevy::camera::RenderTarget;
use bevy::render::MainWorld;
use bevy::{
    asset::{AssetId, RenderAssetUsages},
    camera::ManualTextureViewHandle,
    ecs::{
        system::lifetimeless::SResMut,
        system::{lifetimeless::SRes, SystemParamItem},
    },
    platform::collections::HashMap,
    prelude::*,
    render::{
        render_asset::{AssetExtractionError, PrepareAssetError, RenderAsset, RenderAssets},
        render_resource::{Texture, TextureView},
        renderer::RenderDevice,
        texture::{DefaultImageSampler, GpuImage, ManualTextureView},
    },
};
use thiserror::Error;

pub mod hal;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum GpuExternalBuffer {
    Sampling {
        image_id: AssetId<Image>,
        gpu_image: GpuImage,
    },
    RenderTarget {
        handle: ManualTextureViewHandle,
        texture_view: ManualTextureView,
    },
    Invalid(ImportError),
}

impl RenderAsset for GpuExternalBuffer {
    type SourceAsset = ExternalBuffer;
    type Param = (
        SRes<RenderDevice>,
        SRes<DefaultImageSampler>,
        SResMut<RenderAssets<GpuImage>>,
        SResMut<AssetIdCache>,
        SResMut<PendingExternalRenderTargets>,
    );

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

        debug!("Trying to prepare render asset {asset_id}");

        let (render_device, default_sampler, gpu_images, asset_ids, pending_render_targets) = params;

        let ExternalBufferCreationData::Dmabuf { dma } = source_asset.creation_data.unwrap();
        debug!("Importing external texture into render context");
        let import_result =
            hal::import_dmabuf_as_texture(render_device.wgpu_device(), dma, source_asset.usage);
        let gpu_ext_img = match import_result {
            Ok(imported) => match source_asset.usage {
                ExternalBufferUsage::Sampling(image_id) => {
                    let texture_format = imported.texture.format();
                    let size = imported.texture.size();
                    let mips = imported.texture.mip_level_count();
                    Ok(Self::Sampling {
                        image_id,
                        gpu_image: GpuImage {
                            texture: imported.texture,
                            texture_view: imported.texture_view,
                            texture_format,
                            texture_view_format: None,
                            sampler: (***default_sampler).clone(),
                            size,
                            mip_level_count: mips,
                            had_data: false,
                        },
                    })
                }
                ExternalBufferUsage::RenderTarget(handle) => {
                    let extent_3d = imported.texture.size();
                    Ok(Self::RenderTarget {
                        handle,
                        texture_view: ManualTextureView {
                            texture_view: imported.texture_view,
                            size: uvec2(extent_3d.width, extent_3d.height),
                            view_format: imported.texture.format(),
                        },
                    })
                }
            },
            Err(err) => Ok(Self::Invalid(err)),
        }?;

        match &gpu_ext_img {
            GpuExternalBuffer::Sampling {
                image_id,
                gpu_image,
            } => {
                debug!(
                    "GpuImage successfully imported for {} backed by image {}",
                    asset_id, image_id
                );
                gpu_images.insert(*image_id, gpu_image.clone());
                asset_ids.insert(asset_id, *image_id);
            }
            GpuExternalBuffer::RenderTarget {
                handle,
                texture_view,
            } => {
                debug!(
                    "RenderTarget successfully imported for {} backed by texture view handle {:?}",
                    asset_id, handle
                );
                pending_render_targets.insert(*handle, texture_view.clone());
            }
            GpuExternalBuffer::Invalid(err) => {
                error!("Failed to import external image {}: {}", asset_id, err)
            }
        }

        Ok(gpu_ext_img)
    }

    fn unload_asset(
        source_asset: AssetId<Self::SourceAsset>,
        param: &mut SystemParamItem<Self::Param>,
    ) {
        let (_render_device, _default_sampler, gpu_images, asset_ids, _texture_views) = param;

        if let Some(img_id) = asset_ids.get(&source_asset).copied() {
            gpu_images.remove(img_id);
            asset_ids.remove(&source_asset);
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

            Ok(ExternalBuffer {
                creation_data: Some(creation_data),
                usage: source.usage,
            })
        }
    }
}

pub(super) fn sync_render_targets(
    mut main_world: ResMut<MainWorld>,
    mut pending: ResMut<PendingExternalRenderTargets>,
    mut cache: Local<Vec<(Entity, ManualTextureViewHandle, ManualTextureView)>>,
) {
    let mut empty_render_targets = main_world.query::<(Entity, &ExternalRenderTarget)>();
    for (entity, target) in empty_render_targets.iter(&main_world) {
        if let Some(texture_view) = pending.remove(&target.view_handle) {
            trace!("Preparing RenderTarget::ManualTextureView({:?}) insertion for {}", target.view_handle, entity);
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

#[derive(Debug)]
pub struct ImportedTexture {
    texture: Texture,
    texture_view: TextureView,
}

#[derive(Resource, Debug, Default, Deref, DerefMut)]
pub(crate) struct AssetIdCache(HashMap<AssetId<ExternalBuffer>, AssetId<Image>>);

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

#[derive(Resource, Default, Debug, Deref, DerefMut)]
pub(crate) struct PendingExternalRenderTargets(HashMap<ManualTextureViewHandle, ManualTextureView>);

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
