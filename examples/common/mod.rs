use bevy::prelude::*;
use bevy_dmabuf::{
    dmatex::{Dmatex, DmatexPlane, Resolution},
    import::{DmatexUsage, ExternalImageCreationData},
};
use drm_fourcc::{DrmFourcc, DrmModifier};
use smithay::backend::allocator::{
    dmabuf::AsDmabuf, gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
    Allocator,
    Buffer,
};
use std::fs::{File, OpenOptions};

#[derive(Debug, Resource)]
pub struct ExternalImageSource {
    buffer_allocator: GbmAllocator<File>,
}

impl ExternalImageSource {
    pub fn new() -> Self {
        let drm = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/dri/renderD128")
            .expect("Failed to open DRI file");
        let gbm = GbmDevice::new(drm).expect("Failed to open GBM Device");
        let allocator = GbmAllocator::new(gbm, GbmBufferFlags::RENDERING);
        Self {
            buffer_allocator: allocator,
        }
    }

    pub fn create_buffer(&mut self, image: &Image) -> ExternalImageCreationData {
        let size = image.size();
        let mut buffer = self
            .buffer_allocator
            .create_buffer(size.x, size.y, DrmFourcc::Abgr8888, &[DrmModifier::Linear])
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
                            dst_row[x * 4 + 1] = r;
                            dst_row[x * 4 + 2] = g;
                            dst_row[x * 4 + 3] = b;
                        }
                    }
                }
            })
            .expect("Failed to copy image to GPU buffer.");

        let dma = buffer.export().expect("Failed to export buffer as DmaBuf");

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

        ExternalImageCreationData::Dmabuf {
            dma: Dmatex {
                planes,
                res,
                format: dma.format(),
                srgb: image.texture_descriptor.format.is_srgb(),
            },
            usage: DmatexUsage::Sampling,
        }
    }
}
