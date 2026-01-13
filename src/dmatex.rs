use std::os::fd::OwnedFd;

/// Dmabuf Backed Texture
#[derive(Debug)]
pub struct Dmatex {
    pub planes: Vec<DmatexPlane>,
    pub res: Resolution,
    pub format: u32,
    pub modifier: u64,
    /// if the format has an srgb version, use that
    pub srgb: bool,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Resolution {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug)]
pub struct DmatexPlane {
    pub dmabuf_fd: OwnedFd,
    pub offset: u32,
    pub stride: i32,
}
