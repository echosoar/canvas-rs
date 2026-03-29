//! Pure-Rust 2-D drawing library with a web-canvas-like API.
//!
//! No external dependencies are required.
//!
//! # Quick start
//!
//! ```
//! use canvas_rs::Canvas;
//!
//! // Create a 200×100 canvas.
//! let canvas = Canvas::new(200, 100);
//! let mut ctx = canvas.get_context("2d").unwrap();
//!
//! // White background.
//! ctx.set_fill_style("white");
//! ctx.fill_rect(0.0, 0.0, 200.0, 100.0);
//!
//! // Red circle.
//! ctx.set_fill_style("red");
//! ctx.begin_path();
//! ctx.arc(100.0, 50.0, 40.0, 0.0, std::f64::consts::PI * 2.0, false);
//! ctx.fill();
//!
//! // Export as data URL.
//! let url = canvas.to_data_url();
//! assert!(url.starts_with("data:image/png;base64,"));
//! ```

pub mod canvas;
pub mod color;
pub mod image;
pub mod path;
pub mod png;
pub mod render;

pub use canvas::{Canvas, Context2D};
pub use color::Color;
pub use image::ImageData;
pub use render::LineCap;
