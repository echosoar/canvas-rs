//! Pure-Rust 2-D drawing library with a web-canvas-like API.
//!
//! No external dependencies are required.  All drawing operations work on an
//! in-memory RGBA pixel buffer.  To encode or decode PNG images, use the
//! companion `images` crate.
//!
//! # Quick start
//!
//! ```
//! use canvas::Canvas;
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
//! ```

pub mod canvas;
pub mod color;
pub mod font;
pub mod gradient;
pub mod image;
pub mod path;
pub mod render;
pub mod svg;

pub use crate::canvas::{Canvas, Context2D};
pub use crate::color::Color;
pub use crate::font::{Font, FontConfig, FontWidth};
pub use crate::gradient::{LinearGradient, RadialGradient, Style};
pub use crate::image::ImageData;
pub use crate::render::{LineCap, TextAlign};
pub use crate::svg::{draw_svg, render_svg, render_svg_str};
