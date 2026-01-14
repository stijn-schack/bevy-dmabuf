use crate::dmatex::Dmatex;
use crate::format_mapping::{drm_fourcc_to_vk_format, vk_format_to_srgb, vulkan_to_wgpu};
use crate::import::hal::{import_texture, memory_barrier};
use ash::vk;
use bevy::{
    app::Plugin,
    asset::{AssetId, Assets, Handle, RenderAssetUsages},
    ecs::{
        resource::Resource,
        schedule::{IntoScheduleConfigs as _, SystemSet},
        system::{Res, ResMut},
        world::World,
    },
    image::Image,
    log::{debug, debug_span, error, trace, warn},
    platform::collections::HashMap,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin}, render_asset::{prepare_assets, RenderAssets}, render_resource::{Texture, TextureView},
        renderer::RenderDevice,
        texture::GpuImage,
        Render,
        RenderApp,
        RenderSystems,
    },
    utils::default,
};
use drm_fourcc::DrmFourcc;
use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
};
use thiserror::Error;
use wgpu::TextureUsages;

mod hal;

pub struct DmabufImportPlugin;

impl Plugin for DmabufImportPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        let handles = ImportedDmatexs(default());
        app.insert_resource(handles.clone());
        app.add_plugins(ExtractResourcePlugin::<ImportedDmatexs>::default());
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.configure_sets(
                Render,
                (
                    DmatexRenderSystemSet::InsertIntoGpuImages
                        .in_set(RenderSystems::PrepareAssets)
                        .after(prepare_assets::<GpuImage>),
                    DmatexRenderSystemSet::AcquireDmatexs
                        .in_set(RenderSystems::PrepareAssets)
                        .after(DmatexRenderSystemSet::InsertIntoGpuImages),
                    DmatexRenderSystemSet::ReleaseDmatexs.in_set(RenderSystems::Cleanup),
                ),
            );
            render_app.add_systems(
                Render,
                insert_dmatex_into_gpu_images.in_set(DmatexRenderSystemSet::InsertIntoGpuImages),
            );
            render_app.add_systems(
                Render,
                (
                    acquire_dmatex_images.in_set(DmatexRenderSystemSet::AcquireDmatexs),
                    release_dmatex_images.in_set(DmatexRenderSystemSet::ReleaseDmatexs),
                ),
            );
        } else {
            warn!("unable to init dmabuf importing!");
        }
    }
}

#[derive(SystemSet, Hash, Debug, Clone, PartialEq, Eq, Copy)]
pub enum DmatexRenderSystemSet {
    InsertIntoGpuImages,
    AcquireDmatexs,
    ReleaseDmatexs,
}

#[derive(Resource, Clone, ExtractResource)]
pub struct ImportedDmatexs(Arc<Mutex<HashMap<AssetId<Image>, DmaImage>>>);

#[derive(Debug)]
enum DmaImage {
    UnImported(Dmatex, DropCallback, DmatexUsage),
    Imported(ImportedTexture),
}

#[derive(Clone, Copy, Debug)]
pub enum DmatexUsage {
    Sampling,
}

pub struct DropCallback(pub Option<Box<dyn FnOnce() + 'static + Send + Sync>>);
impl Debug for DropCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DropCallback").finish()
    }
}
impl Drop for DropCallback {
    fn drop(&mut self) {
        if let Some(callback) = self.0.take() {
            callback();
        }
    }
}

impl ImportedDmatexs {
    pub fn set(
        &self,
        images: &mut Assets<Image>,
        buf: Dmatex,
        usage: DmatexUsage,
        on_drop: Option<Box<dyn FnOnce() + 'static + Send + Sync>>,
    ) -> Result<Handle<Image>, ImportError> {
        let handle = get_handle(images, &buf)?;
        #[expect(clippy::unwrap_used)]
        self.0.lock().unwrap().insert(
            handle.id(),
            DmaImage::UnImported(buf, DropCallback(on_drop), usage),
        );
        Ok(handle)
    }
    pub fn insert_imported_dmatex(
        &self,
        images: &mut Assets<Image>,
        tex: ImportedTexture,
    ) -> Handle<Image> {
        let handle = debug_span!("creating dummy image").in_scope(|| {
            images.add(Image::new_uninit(
                tex.texture.size(),
                tex.texture.dimension(),
                tex.texture.format(),
                RenderAssetUsages::RENDER_WORLD,
            ))
        });

        let _span = debug_span!("inserting image handle").entered();
        #[expect(clippy::unwrap_used)]
        self.0
            .lock()
            .unwrap()
            .insert(handle.id(), DmaImage::Imported(tex));
        handle
    }
}

fn acquire_dmatex_images(world: &mut World) {
    let device = world.resource::<RenderDevice>();
    let dmatexs = world.resource::<ImportedDmatexs>();
    memory_barrier(device, dmatexs, ImageQueueTransfer::Acquire);
}
fn release_dmatex_images(world: &mut World) {
    let device = world.resource::<RenderDevice>();
    let dmatexs = world.resource::<ImportedDmatexs>();
    memory_barrier(device, dmatexs, ImageQueueTransfer::Release);
}

enum ImageQueueTransfer {
    Acquire,
    Release,
}

fn insert_dmatex_into_gpu_images(
    mut gpu_images: ResMut<RenderAssets<GpuImage>>,
    imported: Res<ImportedDmatexs>,
    device: Res<RenderDevice>,
) {
    #[expect(clippy::unwrap_used)]
    let mut imported = imported.0.lock().unwrap();
    let handles = imported.keys().copied().collect::<Vec<_>>();
    for asset_id in handles {
        // filter out outdated dmatexs
        if gpu_images.get(asset_id).is_none() {
            imported.remove(&asset_id);
            continue;
        }
        if matches!(imported.get(&asset_id), Some(DmaImage::UnImported(_, _, _)))
            && let Some(DmaImage::UnImported(dmabuf, on_drop, usage)) = imported.remove(&asset_id)
        {
            match import_texture(&device, dmabuf, on_drop, usage) {
                Ok(tex) => {
                    debug!("imported dmatex");
                    imported.insert(asset_id, DmaImage::Imported(tex));
                }
                Err(err) => {
                    error!("failed to import dmatex: {err}");
                    continue;
                }
            }
        }
        let Some(render_tex) = gpu_images.get_mut(asset_id) else {
            warn!("invalid texture handle (unreachable)");
            #[cfg(debug_assertions)]
            unreachable!();
            #[cfg(not(debug_assertions))]
            continue;
        };

        if let Some(DmaImage::Imported(tex)) = imported.get(&asset_id) {
            trace!("setting texture view!");
            render_tex.texture_view = tex.texture_view.clone();
            render_tex.size = tex.texture.size();
            render_tex.mip_level_count = tex.texture.mip_level_count();
            render_tex.texture = tex.texture.clone();
        } else {
            error!("unreachable");
        }
    }
}

fn get_handle(images: &mut Assets<Image>, buf: &Dmatex) -> Result<Handle<Image>, ImportError> {
    let desc = get_imported_descriptor(buf)?;
    Ok(images.add(Image::new_uninit(
        desc.size,
        desc.dimension,
        desc.format,
        RenderAssetUsages::RENDER_WORLD,
    )))
}

#[derive(Error, Debug, Clone, Copy)]
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

fn get_imported_descriptor(buf: &Dmatex) -> Result<wgpu::TextureDescriptor<'static>, ImportError> {
    let vulkan_format = drm_fourcc_to_vk_format(
        DrmFourcc::try_from(buf.format).map_err(ImportError::UnrecognizedFourcc)?,
    )
    .ok_or(ImportError::VulkanIncompatibleFormat)?;
    let vulkan_format = buf
        .srgb
        .then(|| vk_format_to_srgb(vulkan_format))
        .flatten()
        .unwrap_or(vulkan_format);
    Ok(wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: buf.res.x,
            height: buf.res.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: vulkan_to_wgpu(vulkan_format).ok_or(ImportError::WgpuIncompatibleFormat)?,
        usage: TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC
            | TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

#[derive(Clone, Debug)]
pub struct ImportedTexture {
    texture: Texture,
    texture_view: TextureView,
    _usage: DmatexUsage,
}

impl ImportedTexture {
    pub fn new(texture: Texture, texture_view: TextureView) -> ImportedTexture {
        ImportedTexture {
            texture,
            texture_view,
            _usage: DmatexUsage::Sampling,
        }
    }
}
