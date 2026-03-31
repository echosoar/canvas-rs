//! PNG encoding and decoding for the `canvas` crate.
//!
//! This crate wraps the core `canvas` RGBA buffer with PNG import/export
//! capabilities.
//!
//! # Quick start
//!
//! ```no_run
//! use canvas::Canvas;
//!
//! let mut canvas = Canvas::new(200, 100);
//! let mut ctx = canvas.get_context("2d").unwrap();
//! ctx.set_fill_style("red");
//! ctx.fill_rect(0.0, 0.0, 200.0, 100.0);
//!
//! // Export to a data URL.
//! let url = images::to_data_url(&canvas);
//! assert!(url.starts_with("data:image/png;base64,"));
//!
//! // Or get raw PNG bytes.
//! let png_bytes: Vec<u8> = images::to_blob(&canvas);
//! ```

pub mod encoder;

pub use encoder::{base64_encode, encode_png};

use canvas::{Canvas, ImageData};

/// Encode a `Canvas` as raw PNG bytes.
pub fn to_blob(canvas: &Canvas) -> Vec<u8> {
    let buf = canvas.pixels();
    encode_png(canvas.width(), canvas.height(), &buf)
}

/// Encode a `Canvas` as a `data:image/png;base64,...` URL.
pub fn to_data_url(canvas: &Canvas) -> String {
    format!("data:image/png;base64,{}", base64_encode(&to_blob(canvas)))
}

/// Decode a PNG byte slice into an [`ImageData`] with RGBA pixels.
///
/// Returns an error string if the bytes cannot be decoded as a valid PNG.
pub fn from_png(bytes: &[u8]) -> Result<ImageData, String> {
    use png::ColorType;
    use std::io::Cursor;

    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("PNG read_info error: {e}"))?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let frame = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("PNG decode error: {e}"))?;
    let raw = buf[..frame.buffer_size()].to_vec();
    let (w, h) = (frame.width, frame.height);
    let rgba = match frame.color_type {
        ColorType::Rgba => raw,
        ColorType::Rgb => raw
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255u8])
            .collect(),
        ColorType::Grayscale => raw
            .iter()
            .flat_map(|&v| [v, v, v, 255u8])
            .collect(),
        ColorType::GrayscaleAlpha => raw
            .chunks(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        other => return Err(format!("unsupported PNG color type: {other:?}")),
    };
    Ok(ImageData::from_rgba(w, h, rgba))
}
