use std::cell::RefCell;
use std::rc::Rc;

use crate::color::{parse_color, Color};
use crate::font::Font;
use crate::image::ImageData;
use crate::path::{Path, PathCommand};
use crate::render::{self, LineCap};

// ── Canvas ───────────────────────────────────────────────────────────────────

/// A 2-D drawing surface, analogous to the HTML `<canvas>` element.
///
/// ```
/// use canvas::Canvas;
///
/// let canvas = Canvas::new(100, 100);
/// let mut ctx = canvas.get_context("2d").unwrap();
/// ctx.set_fill_style("red");
/// ctx.fill_rect(0.0, 0.0, 100.0, 100.0);
/// ```
pub struct Canvas {
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// RGBA pixel buffer, row-major.
    pub(crate) buffer: Rc<RefCell<Vec<u8>>>,
}

impl Canvas {
    /// Create a new transparent canvas of the given size.
    pub fn new(width: u32, height: u32) -> Self {
        Canvas {
            width,
            height,
            buffer: Rc::new(RefCell::new(vec![0u8; (width * height * 4) as usize])),
        }
    }

    /// Return a 2-D rendering context.  Any context type other than `"2d"`
    /// returns `None`.
    pub fn get_context(&self, context_type: &str) -> Option<Context2D> {
        if context_type != "2d" {
            return None;
        }
        Some(Context2D {
            buffer: Rc::clone(&self.buffer),
            width: self.width,
            height: self.height,
            fill_style: Color::black(),
            stroke_style: Color::black(),
            line_width: 1.0,
            line_cap: LineCap::Butt,
            path: Path::new(),
            clip: None,
            font: None,
            font_size: 32,
            font_family: "common".to_string(),
            font_style: String::new(),
            font_string: "32px common".to_string(),
        })
    }

    /// Return the canvas width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Return the canvas height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Read a copy of the raw RGBA pixel buffer.
    pub fn get_image_data(&self) -> ImageData {
        ImageData::from_rgba(self.width, self.height, self.buffer.borrow().clone())
    }

    /// Borrow the raw RGBA pixel buffer.
    pub fn pixels(&self) -> std::cell::Ref<'_, Vec<u8>> {
        self.buffer.borrow()
    }
}

// ── Context2D ────────────────────────────────────────────────────────────────

/// A 2-D rendering context, analogous to the browser `CanvasRenderingContext2D`.
pub struct Context2D {
    buffer: Rc<RefCell<Vec<u8>>>,
    width: u32,
    height: u32,

    // ── State ──────────────────────────────────────────────────
    fill_style: Color,
    stroke_style: Color,
    line_width: f64,
    line_cap: LineCap,
    path: Path,
    clip: Option<Vec<bool>>,

    // ── Font ───────────────────────────────────────────────────
    font: Option<Font>,
    font_size: u32,
    font_family: String,
    font_style: String,  // bold, italic, etc.
    font_string: String, // original font string like "bold 48px serif"
}

// ─── Property accessors ───────────────────────────────────────────────────────

impl Context2D {
    // fill_style

    /// Set the fill colour from any CSS colour string.
    /// Invalid strings are silently ignored.
    pub fn set_fill_style(&mut self, style: &str) {
        if let Some(c) = parse_color(style) {
            self.fill_style = c;
        }
    }

    /// Return the current fill colour as an `"rgba(r,g,b,a)"` string.
    pub fn fill_style(&self) -> String {
        color_to_css(self.fill_style)
    }

    // stroke_style

    /// Set the stroke colour from any CSS colour string.
    /// Invalid strings are silently ignored.
    pub fn set_stroke_style(&mut self, style: &str) {
        if let Some(c) = parse_color(style) {
            self.stroke_style = c;
        }
    }

    /// Return the current stroke colour as an `"rgba(r,g,b,a)"` string.
    pub fn stroke_style(&self) -> String {
        color_to_css(self.stroke_style)
    }

    // line_width

    pub fn set_line_width(&mut self, width: f64) {
        if width > 0.0 {
            self.line_width = width;
        }
    }

    pub fn line_width(&self) -> f64 {
        self.line_width
    }

    // line_cap

    /// Set the line-cap style: `"butt"` (default), `"round"`, or `"square"`.
    pub fn set_line_cap(&mut self, cap: &str) {
        self.line_cap = LineCap::parse_cap(cap);
    }

    pub fn line_cap(&self) -> &'static str {
        self.line_cap.as_str()
    }

    // font

    /// Set the font using CSS font string format: `"bold 48px serif"`.
    ///
    /// Parses the font string to extract font size and family name.
    /// The font file should be in the lib directory as `{family}.txt`.
    /// Invalid strings are silently ignored.
    pub fn set_font(&mut self, font_str: &str) {
        if let Some((size, family, style)) = parse_font_string(font_str) {
            self.font_size = size;
            self.font_family = family;
            self.font_style = style;
            self.font_string = font_str.to_string();
            self.font = None; // Clear cached font, will load on first use
        }
    }

    /// Return the current font string like `"bold 48px serif"`.
    pub fn font(&self) -> &str {
        &self.font_string
    }

    /// Load the font if not already loaded.
    fn ensure_font_loaded(&mut self) {
        if self.font.is_none() {
            self.font = Font::load(&self.font_family).ok();
        }
    }
}

// ─── Path methods ─────────────────────────────────────────────────────────────

impl Context2D {
    /// Reset the current path.
    pub fn begin_path(&mut self) {
        self.path = Path::new();
    }

    /// Begin a new sub-path at `(x, y)`.
    pub fn move_to(&mut self, x: f64, y: f64) {
        self.path.commands.push(PathCommand::MoveTo(x, y));
    }

    /// Add a line to `(x, y)`.
    pub fn line_to(&mut self, x: f64, y: f64) {
        self.path.commands.push(PathCommand::LineTo(x, y));
    }

    /// Append a circular arc to the path.
    ///
    /// - `(x, y)`: centre of the arc.
    /// - `radius`: radius in pixels.
    /// - `start_angle` / `end_angle`: in radians (0 = right, clockwise).
    /// - `counterclockwise`: if `true`, sweep counter-clockwise.
    pub fn arc(
        &mut self,
        x: f64,
        y: f64,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        counterclockwise: bool,
    ) {
        self.path.commands.push(PathCommand::Arc(
            x,
            y,
            radius,
            start_angle,
            end_angle,
            counterclockwise,
        ));
    }

    /// Close the current sub-path by adding a straight line back to its start.
    pub fn close_path(&mut self) {
        self.path.commands.push(PathCommand::ClosePath);
    }

    /// Fill the interior of the current path with `fillStyle`.
    pub fn fill(&mut self) {
        let sub_paths = self.path.flatten();
        let color = self.fill_style;
        let mut buf = self.buffer.borrow_mut();
        for pts in &sub_paths {
            render::fill_subpath(&mut buf, self.width, self.height, pts, color, &self.clip);
        }
    }

    /// Stroke the outline of the current path with `strokeStyle`.
    pub fn stroke(&mut self) {
        let sub_paths = self.path.flatten();
        let color = self.stroke_style;
        let lw = self.line_width;
        let cap = self.line_cap;
        let mut buf = self.buffer.borrow_mut();
        for pts in &sub_paths {
            render::stroke_polyline(&mut buf, self.width, self.height, pts, color, lw, cap, &self.clip);
        }
    }

    /// Use the current path as the new clipping region.
    ///
    /// Subsequent drawing operations are restricted to the interior of the
    /// path.  The clip is intersected with any existing clip region.
    pub fn clip(&mut self) {
        let sub_paths = self.path.flatten();
        self.clip = Some(render::build_clip_mask(
            self.width,
            self.height,
            &sub_paths,
            &self.clip,
        ));
    }
}

// ─── Rectangle / clear methods ────────────────────────────────────────────────

impl Context2D {
    /// Fill a rectangle with `fillStyle`.
    pub fn fill_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let color = self.fill_style;
        render::fill_rect(
            &mut self.buffer.borrow_mut(),
            self.width,
            self.height,
            x, y, width, height,
            color,
            &self.clip,
        );
    }

    /// Stroke the outline of a rectangle with `strokeStyle`.
    pub fn stroke_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let color = self.stroke_style;
        let lw = self.line_width;
        let cap = self.line_cap;
        render::stroke_rect(
            &mut self.buffer.borrow_mut(),
            self.width,
            self.height,
            x, y, width, height,
            color, lw, cap,
            &self.clip,
        );
    }

    /// Erase a rectangle to fully transparent black.
    pub fn clear_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        render::clear_rect(
            &mut self.buffer.borrow_mut(),
            self.width,
            self.height,
            x, y, width, height,
        );
    }
}

// ─── Image methods ────────────────────────────────────────────────────────────

impl Context2D {
    /// Draw `image` at `(dx, dy)` at its natural size.
    pub fn draw_image(&mut self, image: &ImageData, dx: f64, dy: f64) {
        render::draw_image(
            &mut self.buffer.borrow_mut(),
            self.width,
            self.height,
            image, dx, dy,
            &self.clip,
        );
    }

    /// Draw `image` at `(dx, dy)` scaled to `(dw, dh)`.
    pub fn draw_image_with_size(&mut self, image: &ImageData, dx: f64, dy: f64, dw: f64, dh: f64) {
        render::draw_image_region(
            &mut self.buffer.borrow_mut(),
            self.width,
            self.height,
            image,
            0.0, 0.0, image.width as f64, image.height as f64,
            dx, dy, dw, dh,
            &self.clip,
        );
    }

    /// Draw a sub-rectangle of `image` (source `sx,sy,sw,sh`) into the
    /// destination area `(dx, dy, dw, dh)`, scaling as needed.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_image_source(
        &mut self,
        image: &ImageData,
        sx: f64, sy: f64, sw: f64, sh: f64,
        dx: f64, dy: f64, dw: f64, dh: f64,
    ) {
        render::draw_image_region(
            &mut self.buffer.borrow_mut(),
            self.width,
            self.height,
            image,
            sx, sy, sw, sh,
            dx, dy, dw, dh,
            &self.clip,
        );
    }

    /// Draw a `Canvas` at `(dx, dy)` at its natural size.
    pub fn draw_canvas(&mut self, src: &Canvas, dx: f64, dy: f64) {
        let img = src.get_image_data();
        self.draw_image(&img, dx, dy);
    }

    /// Draw a `Canvas` scaled to `(dw, dh)`.
    pub fn draw_canvas_with_size(&mut self, src: &Canvas, dx: f64, dy: f64, dw: f64, dh: f64) {
        let img = src.get_image_data();
        self.draw_image_with_size(&img, dx, dy, dw, dh);
    }
}

// ─── Text methods ─────────────────────────────────────────────────────────────

impl Context2D {
    /// Fill text at position `(x, y)` using the current `fillStyle` and font settings.
    ///
    /// The text is rendered using the loaded font bitmap, scaled to the current
    /// `font_size`. Characters not found in the font are skipped.
    pub fn fill_text(&mut self, text: &str, x: f64, y: f64) {
        self.ensure_font_loaded();

        if let Some(font) = &self.font {
            let color = self.fill_style;
            let (bitmap, _text_width, _text_height) = font.render_text(text, self.font_size);

            if bitmap.is_empty() {
                return;
            }

            // Draw each pixel of the text bitmap
            let mut buf = self.buffer.borrow_mut();
            for (row_idx, row) in bitmap.iter().enumerate() {
                for (col_idx, pixel) in row.iter().enumerate() {
                    if *pixel {
                        let px = x as i64 + col_idx as i64;
                        let py = y as i64 + row_idx as i64;
                        render::put_pixel(
                            &mut buf,
                            self.width,
                            self.height,
                            px,
                            py,
                            color,
                            &self.clip,
                        );
                    }
                }
            }
        }
    }

    /// Fill text with maximum width constraint.
    /// If the text is wider than max_width, it will be scaled down to fit.
    pub fn fill_text_with_max_width(&mut self, text: &str, x: f64, y: f64, max_width: f64) {
        self.ensure_font_loaded();

        if let Some(font) = &self.font {
            let (bitmap, original_width, _) = font.render_text(text, self.font_size);

            if bitmap.is_empty() || original_width == 0 {
                return;
            }

            // Calculate scale factor if text exceeds max_width
            let scale = if original_width as f64 > max_width {
                max_width / original_width as f64
            } else {
                1.0
            };

            let color = self.fill_style;
            let scaled_height = (bitmap.len() as f64 * scale).ceil() as usize;
            let scaled_width = (bitmap[0].len() as f64 * scale).ceil() as usize;

            // Draw scaled text
            let mut buf = self.buffer.borrow_mut();
            for dst_y in 0..scaled_height {
                for dst_x in 0..scaled_width {
                    // Nearest neighbor sampling
                    let src_x = (dst_x as f64 / scale).round() as usize;
                    let src_y = (dst_y as f64 / scale).round() as usize;

                    if src_y < bitmap.len() && src_x < bitmap[src_y].len() && bitmap[src_y][src_x] {
                        let px = x as i64 + dst_x as i64;
                        let py = y as i64 + dst_y as i64;
                        render::put_pixel(
                            &mut buf,
                            self.width,
                            self.height,
                            px,
                            py,
                            color,
                            &self.clip,
                        );
                    }
                }
            }
        }
    }

    /// Measure text width with current font settings.
    /// Returns the width in pixels.
    pub fn measure_text(&self, text: &str) -> f64 {
        // Need to temporarily load font for measurement
        let font = Font::load(&self.font_family).ok();
        if let Some(font) = font {
            let (_, width, _) = font.render_text(text, self.font_size);
            width as f64
        } else {
            0.0
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn color_to_css(c: Color) -> String {
    format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a as f64 / 255.0)
}

/// Parse CSS font string like "bold 48px serif".
/// Returns (size, family, style) where style is "bold", "italic", etc.
fn parse_font_string(font_str: &str) -> Option<(u32, String, String)> {
    let parts: Vec<&str> = font_str.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let mut size = 32u32;  // default size
    let mut family = "common".to_string();  // default family
    let mut style = String::new();

    for part in parts {
        // Check for size (e.g., "48px")
        if part.ends_with("px") {
            let size_str = part.trim_end_matches("px");
            if let Ok(s) = size_str.parse::<u32>() {
                size = s;
            }
        }
        // Check for style keywords
        else if part == "bold" || part == "italic" || part == "normal" {
            if !style.is_empty() {
                style.push(' ');
            }
            style.push_str(part);
        }
        // Last non-size, non-style part is the family
        else {
            family = part.to_string();
        }
    }

    Some((size, family, style))
}
