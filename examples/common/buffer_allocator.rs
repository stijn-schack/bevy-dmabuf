use bevy::{platform::collections::HashMap, prelude::*};
use bevy_dmabuf::{
    dmatex::{Dmatex, DmatexPlane, Resolution},
    ExternalBufferCreationData,
};
use drm_fourcc::{DrmFourcc, DrmModifier};
use smithay::backend::allocator::{
    dmabuf::{AsDmabuf, Dmabuf}, gbm::{GbmAllocator, GbmBuffer, GbmBufferFlags, GbmDevice},
    Allocator,
    Buffer,
};
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

pub(super) struct ExternalImageSourcePlugin {
    pub capture_dir: &'static Path,
}

impl Plugin for ExternalImageSourcePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ExternalBufferSource::new(self.capture_dir));
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct BufferId(u64);

#[derive(Debug, Resource)]
pub struct ExternalBufferSource {
    buffer_allocator: GbmAllocator<File>,
    buffers: HashMap<BufferId, GbmBuffer>,
    next_id: u64,
    capture_dir: &'static Path,
}

impl ExternalBufferSource {
    fn new(capture_dir: &'static Path) -> Self {
        if !capture_dir.is_dir() {
            panic!(
                "{:?} is not a directory, or the user does not have permissions to access this path",
                capture_dir
            );
        }

        let drm = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/dri/renderD128")
            .expect("Failed to open DRI file");
        let gbm = GbmDevice::new(drm).expect("Failed to open GBM Device");
        let allocator = GbmAllocator::new(gbm, GbmBufferFlags::RENDERING);
        Self {
            buffer_allocator: allocator,
            buffers: HashMap::new(),
            next_id: 0,
            capture_dir,
        }
    }

    pub fn create_buffer_from_image(
        &mut self,
        image: &Image,
    ) -> (BufferId, ExternalBufferCreationData) {
        let size = image.size();
        let mut buffer = self
            .buffer_allocator
            .create_buffer(size.x, size.y, DrmFourcc::Abgr8888, &[DrmModifier::Invalid])
            .expect("Failed to allocate buffer");

        let src = image.data.clone().unwrap();

        buffer
            .map_mut(0, 0, size.x, size.y, |mapped_obj| {
                let src = src.as_slice();
                let stride = mapped_obj.stride() as usize;
                let dst = mapped_obj.buffer_mut();

                for y in 0..size.y as usize {
                    let dst_row = &mut dst[y * stride..y * stride + (size.x as usize * 4)];
                    let src_row = &src[y * size.x as usize * 4..(y + 1) * size.x as usize * 4];

                    for x in 0..size.x as usize {
                        let r = src_row[x * 4];
                        let g = src_row[x * 4 + 1];
                        let b = src_row[x * 4 + 2];
                        let a = src_row[x * 4 + 3];

                        #[cfg(target_endian = "little")]
                        {
                            dst_row[x * 4] = r;
                            dst_row[x * 4 + 1] = g;
                            dst_row[x * 4 + 2] = b;
                            dst_row[x * 4 + 3] = a;
                        }
                        #[cfg(target_endian = "big")]
                        {
                            dst_row[x * 4] = a;
                            dst_row[x * 4 + 1] = b;
                            dst_row[x * 4 + 2] = g;
                            dst_row[x * 4 + 3] = r;
                        }
                    }
                }
            })
            .expect("Failed to map GbmBuffer.");

        let dma = buffer.export().expect("Failed to export buffer as DmaBuf");
        let id = self.save(buffer);
        (
            id,
            to_creation_data(dma, image.texture_descriptor.format.is_srgb()),
        )
    }

    pub fn create_empty_buffer(
        &mut self,
        width: u32,
        height: u32,
    ) -> (BufferId, ExternalBufferCreationData) {
        let buffer = self
            .buffer_allocator
            .create_buffer(width, height, DrmFourcc::Abgr8888, &[DrmModifier::Linear])
            .expect("Failed to allocate buffer");
        let dma = buffer.export().expect("Failed to export as dma buffer");
        let id = self.save(buffer);
        (id, to_creation_data(dma, true))
    }

    fn save(&mut self, buffer: GbmBuffer) -> BufferId {
        let id = BufferId(self.next_id);
        self.next_id += 1;
        self.buffers.insert(id, buffer);
        id
    }

    pub fn remove(&mut self, buffer_id: BufferId) {
        self.buffers.remove(&buffer_id);
    }

    pub fn write_to_disk(&self, buffer_id: BufferId) {
        match self.buffers.get(&buffer_id) {
            Some(buffer) => {
                let mut file_path = PathBuf::from(self.capture_dir);
                file_path.push(format!("example-buffer-{}.png", buffer_id.0));
                self.write_buffer_to_disk(buffer, &file_path);
            }
            None => warn!(
                "Can not write buffer {} to disk as it does not exist (anymore)",
                buffer_id.0
            ),
        };
    }

    fn write_buffer_to_disk(&self, buffer: &GbmBuffer, file_path: &Path) {
        use ::image::{ImageBuffer, ImageFormat, Rgba};
        let image = buffer
            .map(0, 0, buffer.width(), buffer.height(), |mapped_buffer| {
                let stride = buffer.stride() as usize;
                let data = mapped_buffer.buffer();
                ImageBuffer::from_fn(mapped_buffer.width(), mapped_buffer.height(), |x, y| {
                    let offset = (y as usize * stride) + (x as usize * 4);

                    #[cfg(target_endian = "little")]
                    {
                        let r = data[offset];
                        let g = data[offset + 1];
                        let b = data[offset + 2];
                        let a = data[offset + 3];
                        Rgba([r, g, b, a])
                    }
                    #[cfg(target_endian = "big")]
                    {
                        let a = data[offset];
                        let b = data[offset + 1];
                        let g = data[offset + 2];
                        let r = data[offset + 3];
                        Rgba([r, g, b, a])
                    }
                })
            })
            .expect("Failed to map GbmBuffer");

        if let Err(err) = image.save_with_format(file_path, ImageFormat::Png) {
            error!("Failed to save buffer as png file: {}", err);
        } else {
            info!("Saved buffer to {:?}", file_path);
        }
    }
}

fn to_creation_data(dma: Dmabuf, srgb: bool) -> ExternalBufferCreationData {
    let planes = {
        let mut planes = Vec::new();

        let mut offsets = dma.offsets();
        let mut strides = dma.strides();
        for plane_fd in dma.handles() {
            let plane = DmatexPlane {
                dmabuf_fd: plane_fd.try_clone_to_owned().unwrap(),
                offset: offsets.next().unwrap(),
                stride: strides.next().unwrap() as i32,
            };
            planes.push(plane);
        }
        planes
    };

    let res = Resolution {
        x: dma.size().w as u32,
        y: dma.size().h as u32,
    };

    ExternalBufferCreationData::Dmabuf {
        dma: Dmatex {
            planes,
            res,
            format: dma.format(),
            srgb,
        },
    }
}
