use crate::color::Color;

/// A rectangular buffer of RGBA pixels.
///
/// This is the type accepted by `Context2D::draw_image*` and can also be used
/// to construct image data from raw bytes or from a `Canvas`.
#[derive(Clone, Debug)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA bytes (4 bytes per pixel).
    pub data: Vec<u8>,
}

impl ImageData {
    /// Create an all-transparent image.
    pub fn new(width: u32, height: u32) -> Self {
        ImageData {
            width,
            height,
            data: vec![0u8; (width * height * 4) as usize],
        }
    }

    /// Create an image from existing RGBA bytes.  Panics if `data.len() !=
    /// width * height * 4`.
    pub fn from_rgba(width: u32, height: u32, data: Vec<u8>) -> Self {
        assert_eq!(
            data.len(),
            (width * height * 4) as usize,
            "ImageData::from_rgba: data length mismatch"
        );
        ImageData {
            width,
            height,
            data,
        }
    }

    /// Sample a pixel using nearest-neighbour interpolation.
    /// Returns `Color::transparent()` for out-of-bounds coordinates.
    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> Color {
        if x >= self.width || y >= self.height {
            return Color::transparent();
        }
        let idx = ((y * self.width + x) * 4) as usize;
        Color::rgba(
            self.data[idx],
            self.data[idx + 1],
            self.data[idx + 2],
            self.data[idx + 3],
        )
    }

    /// Sample a pixel using nearest-neighbour with `f64` coordinates
    /// (for scaled drawing).  Clamps to image bounds.
    #[inline]
    pub fn sample(&self, fx: f64, fy: f64) -> Color {
        let x = (fx.floor() as i64).clamp(0, self.width as i64 - 1) as u32;
        let y = (fy.floor() as i64).clamp(0, self.height as i64 - 1) as u32;
        self.get_pixel(x, y)
    }
}
