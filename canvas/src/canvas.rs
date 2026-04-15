use std::cell::RefCell;
use std::rc::Rc;

use crate::color::Color;
use crate::font::Font;
use crate::gradient::{LinearGradient, RadialGradient, Style};
use crate::image::ImageData;
use crate::path::{Path, PathCommand};
use crate::render::{self, LineCap, TextAlign};

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
        // Load common font as fallback
        let common_font = Font::load("common").ok();
        Some(Context2D {
            buffer: Rc::clone(&self.buffer),
            width: self.width,
            height: self.height,
            fill_style: Style::from_color(Color::black()),
            stroke_style: Style::from_color(Color::black()),
            line_width: 1.0,
            line_cap: LineCap::Butt,
            path: Path::new(),
            clip: None,
            font: None,
            common_font,
            font_size: 32,
            font_family: "common".to_string(),
            font_style: String::new(),
            font_string: "32px common".to_string(),
            text_align: TextAlign::Start,
            state_stack: Vec::new(),
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

// ── Canvas State (for save/restore) ───────────────────────────────────────────

/// Saved state for Canvas save/restore operations.
/// Mirrors the Web Canvas API behavior where the drawing state is saved,
/// but not the current path.
#[derive(Clone)]
struct ContextState {
    fill_style: Style,
    stroke_style: Style,
    line_width: f64,
    line_cap: LineCap,
    clip: Option<Vec<bool>>,
    font: Option<Font>,
    common_font: Option<Font>,
    font_size: u32,
    font_family: String,
    font_style: String,
    font_string: String,
    text_align: TextAlign,
}

// ── Context2D ────────────────────────────────────────────────────────────────

/// A 2-D rendering context, analogous to the browser `CanvasRenderingContext2D`.
pub struct Context2D {
    buffer: Rc<RefCell<Vec<u8>>>,
    width: u32,
    height: u32,

    // ── State ──────────────────────────────────────────────────
    fill_style: Style,
    stroke_style: Style,
    line_width: f64,
    line_cap: LineCap,
    path: Path,
    clip: Option<Vec<bool>>,

    // ── Font ───────────────────────────────────────────────────
    font: Option<Font>,
    common_font: Option<Font>,  // Fallback font for characters not in main font
    font_size: u32,
    font_family: String,
    font_style: String,  // bold, italic, etc.
    font_string: String, // original font string like "bold 48px serif"
    text_align: TextAlign,

    // ── State Stack (for save/restore) ────────────────────────
    state_stack: Vec<ContextState>,
}

// ─── Property accessors ───────────────────────────────────────────────────────

impl Context2D {
    // fill_style

    /// Set the fill style from any CSS colour string.
    /// Invalid strings are silently ignored.
    pub fn set_fill_style(&mut self, style: &str) {
        if let Some(s) = Style::from_color_str(style) {
            self.fill_style = s;
        }
    }

    /// Set the fill style from a gradient.
    pub fn set_fill_style_gradient(&mut self, gradient: &LinearGradient) {
        self.fill_style = Style::LinearGradient(gradient.clone());
    }

    /// Set the fill style from a radial gradient.
    pub fn set_fill_style_radial_gradient(&mut self, gradient: &RadialGradient) {
        self.fill_style = Style::RadialGradient(gradient.clone());
    }

    /// Return the current fill style as a string representation.
    /// For colors, returns `"rgba(r,g,b,a)"`.
    /// For gradients, returns a description string.
    pub fn fill_style(&self) -> String {
        style_to_string(&self.fill_style)
    }

    // stroke_style

    /// Set the stroke style from any CSS colour string.
    /// Invalid strings are silently ignored.
    pub fn set_stroke_style(&mut self, style: &str) {
        if let Some(s) = Style::from_color_str(style) {
            self.stroke_style = s;
        }
    }

    /// Set the stroke style from a gradient.
    pub fn set_stroke_style_gradient(&mut self, gradient: &LinearGradient) {
        self.stroke_style = Style::LinearGradient(gradient.clone());
    }

    /// Set the stroke style from a radial gradient.
    pub fn set_stroke_style_radial_gradient(&mut self, gradient: &RadialGradient) {
        self.stroke_style = Style::RadialGradient(gradient.clone());
    }

    /// Return the current stroke style as a string representation.
    pub fn stroke_style(&self) -> String {
        style_to_string(&self.stroke_style)
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

    // text_align

    /// Set the text alignment: `"start"` (default), `"end"`, `"left"`, `"right"`, or `"center"`.
    ///
    /// - `start` and `left`: Text starts at the given x position (left-aligned).
    /// - `end` and `right`: Text ends at the given x position (right-aligned).
    /// - `center`: Text is centered at the given x position.
    pub fn set_text_align(&mut self, align: &str) {
        self.text_align = TextAlign::parse_align(align);
    }

    /// Return the current text alignment as a string.
    pub fn text_align(&self) -> &'static str {
        self.text_align.as_str()
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
        let style = self.fill_style.clone();
        let mut buf = self.buffer.borrow_mut();
        for pts in &sub_paths {
            render::fill_subpath_style(&mut buf, self.width, self.height, pts, &style, &self.clip);
        }
    }

    /// Stroke the outline of the current path with `strokeStyle`.
    pub fn stroke(&mut self) {
        let sub_paths = self.path.flatten();
        let style = self.stroke_style.clone();
        let lw = self.line_width;
        let cap = self.line_cap;
        let mut buf = self.buffer.borrow_mut();
        for pts in &sub_paths {
            render::stroke_polyline_style(&mut buf, self.width, self.height, pts, &style, lw, cap, &self.clip);
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
        let style = self.fill_style.clone();
        render::fill_rect_style(
            &mut self.buffer.borrow_mut(),
            self.width,
            self.height,
            x, y, width, height,
            &style,
            &self.clip,
        );
    }

    /// Stroke the outline of a rectangle with `strokeStyle`.
    pub fn stroke_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let style = self.stroke_style.clone();
        let lw = self.line_width;
        let cap = self.line_cap;
        render::stroke_rect_style(
            &mut self.buffer.borrow_mut(),
            self.width,
            self.height,
            x, y, width, height,
            &style, lw, cap,
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
    /// `font_size`. Characters not found in the font will fallback to common font.
    ///
    /// The x position is interpreted based on `textAlign`:
    /// - `start` / `left`: x is the left edge of the text.
    /// - `end` / `right`: x is the right edge of the text.
    /// - `center`: x is the center of the text.
    pub fn fill_text(&mut self, text: &str, x: f64, y: f64) {
        // println!("fill_text: '{}' at ({}, {}) with font '{}'", text, x, y, self.font_string);
        self.ensure_font_loaded();

        // Determine which font to use as primary, and common_font as fallback
        let primary_font = self.font.as_ref().or(self.common_font.as_ref());

        // println!("Primary font: {:?}, Common font: {:?}", primary_font.as_ref().map(|f| &f.config), self.common_font.as_ref().map(|f| &f.config));
        if primary_font.is_none() {
            return;
        }

        let primary_font = primary_font.unwrap();
        let style = self.fill_style.clone();
        let font_size = self.font_size;

        // Calculate scale
        let scale = font_size as f64 / primary_font.config.size as f64;
        let scaled_height = (primary_font.config.size as f64 * scale).ceil() as u32;

        // First, calculate total text width to apply textAlign
        let mut total_scaled_width = 0.0;
        for ch in text.chars() {
            let char_bitmap = primary_font.get_char(ch)
                .or_else(|| self.common_font.as_ref().and_then(|f| f.get_char(ch)));

            if let Some(char_bm) = char_bitmap {
                let scaled_width = (char_bm.width as f64 * scale).ceil() as f64;
                total_scaled_width += scaled_width;
            } else if ch == ' ' {
                // Space character not in font: default to half-width (size/2)
                let half_width = primary_font.config.size / 2;
                let scaled_width = (half_width as f64 * scale).ceil() as f64;
                total_scaled_width += scaled_width;
            }
        }

        // Apply textAlign offset
        let x_offset = x - self.text_align.calculate_x_offset(total_scaled_width);

        let mut current_x = x_offset;

        // Render each character with fallback
        for ch in text.chars() {
            // Try primary font first, then fallback to common_font
            let char_bitmap = primary_font.get_char(ch)
                .or_else(|| self.common_font.as_ref().and_then(|f| f.get_char(ch)));

            if let Some(char_bm) = char_bitmap {
                let scaled_width = (char_bm.width as f64 * scale).ceil() as usize;

                // Draw the character
                let mut buf = self.buffer.borrow_mut();
                for dst_y in 0..scaled_height as usize {
                    for dst_x in 0..scaled_width {
                        let src_x = (dst_x as f64 / scale).round() as usize;
                        let src_y = (dst_y as f64 / scale).round() as usize;

                        if src_x < char_bm.width as usize && src_y < char_bm.height as usize {
                            if char_bm.bitmap[src_y][src_x] {
                                let px = current_x as i64 + dst_x as i64;
                                let py = y as i64 + dst_y as i64;
                                render::put_pixel_style(
                                    &mut buf,
                                    self.width,
                                    self.height,
                                    px,
                                    py,
                                    &style,
                                    &self.clip,
                                );
                            }
                        }
                    }
                }

                current_x += scaled_width as f64;
            } else if ch == ' ' {
                // Space character not in font: default to half-width (size/2)
                let half_width = primary_font.config.size / 2;
                let scaled_width = (half_width as f64 * scale).ceil() as f64;
                current_x += scaled_width;
            }
        }
    }

    /// Fill text with maximum width constraint.
    /// If the text is wider than max_width, it will be scaled down to fit.
    /// Characters not found in the font will fallback to common font.
    ///
    /// The x position is interpreted based on `textAlign`:
    /// - `start` / `left`: x is the left edge of the text.
    /// - `end` / `right`: x is the right edge of the text.
    /// - `center`: x is the center of the text.
    pub fn fill_text_with_max_width(&mut self, text: &str, x: f64, y: f64, max_width: f64) {
        self.ensure_font_loaded();

        // Determine which font to use as primary, and common_font as fallback
        let primary_font = self.font.as_ref().or(self.common_font.as_ref());
        if primary_font.is_none() {
            return;
        }

        let primary_font = primary_font.unwrap();
        let (bitmap, original_width, _) = primary_font.render_text_with_fallback(
            text,
            self.font_size,
            self.common_font.as_ref(),
        );

        if bitmap.is_empty() || original_width == 0 {
            return;
        }

        // Calculate scale factor if text exceeds max_width
        let scale = if original_width as f64 > max_width {
            max_width / original_width as f64
        } else {
            1.0
        };

        let style = self.fill_style.clone();
        let scaled_height = (bitmap.len() as f64 * scale).ceil() as usize;
        let scaled_width = (bitmap[0].len() as f64 * scale).ceil() as usize;
        let scaled_width_f64 = scaled_width as f64;

        // Apply textAlign offset
        let x_offset = x - self.text_align.calculate_x_offset(scaled_width_f64);

        // Draw scaled text
        let mut buf = self.buffer.borrow_mut();
        for dst_y in 0..scaled_height {
            for dst_x in 0..scaled_width {
                // Nearest neighbor sampling
                let src_x = (dst_x as f64 / scale).round() as usize;
                let src_y = (dst_y as f64 / scale).round() as usize;

                if src_y < bitmap.len() && src_x < bitmap[src_y].len() && bitmap[src_y][src_x] {
                    let px = x_offset as i64 + dst_x as i64;
                    let py = y as i64 + dst_y as i64;
                    render::put_pixel_style(
                        &mut buf,
                        self.width,
                        self.height,
                        px,
                        py,
                        &style,
                        &self.clip,
                    );
                }
            }
        }
    }

    /// Measure text width with current font settings.
    /// Returns the width in pixels.
    /// Characters not found in the font will fallback to common font.
    pub fn measure_text(&self, text: &str) -> f64 {
        // Need to temporarily load font for measurement
        let font = Font::load(&self.font_family).ok();
        let common_font = Font::load("common").ok();

        let primary_font = font.as_ref().or(common_font.as_ref());
        if let Some(font) = primary_font {
            let (_, width, _) = font.render_text_with_fallback(
                text,
                self.font_size,
                common_font.as_ref(),
            );
            width as f64
        } else {
            0.0
        }
    }
}

// ─── State management methods (save/restore) ───────────────────────────────────

impl Context2D {
    /// Save the current drawing state onto the state stack.
    ///
    /// The saved state includes:
    /// - fill style, stroke style
    /// - line width, line cap
    /// - clipping region
    /// - font settings (font, font size, font family, font style, font string)
    ///
    /// Note: The current path is NOT saved, consistent with Web Canvas API behavior.
    pub fn save(&mut self) {
        let state = ContextState {
            fill_style: self.fill_style.clone(),
            stroke_style: self.stroke_style.clone(),
            line_width: self.line_width,
            line_cap: self.line_cap,
            clip: self.clip.clone(),
            font: self.font.clone(),
            common_font: self.common_font.clone(),
            font_size: self.font_size,
            font_family: self.font_family.clone(),
            font_style: self.font_style.clone(),
            font_string: self.font_string.clone(),
            text_align: self.text_align,
        };
        self.state_stack.push(state);
    }

    /// Restore the drawing state from the state stack.
    ///
    /// Pops the top state from the stack and applies it to the context.
    /// If the state stack is empty, this method does nothing.
    ///
    /// Note: The current path is NOT restored, consistent with Web Canvas API behavior.
    pub fn restore(&mut self) {
        if let Some(state) = self.state_stack.pop() {
            self.fill_style = state.fill_style;
            self.stroke_style = state.stroke_style;
            self.line_width = state.line_width;
            self.line_cap = state.line_cap;
            self.clip = state.clip;
            self.font = state.font;
            self.common_font = state.common_font;
            self.font_size = state.font_size;
            self.font_family = state.font_family;
            self.font_style = state.font_style;
            self.font_string = state.font_string;
            self.text_align = state.text_align;
        }
    }
}

// ─── Gradient creation methods ───────────────────────────────────────────────────

impl Context2D {
    /// Create a linear gradient along the line from (x0, y0) to (x1, y1).
    ///
    /// Returns a `LinearGradient` that can be configured with color stops
    /// and then assigned to `fillStyle` or `strokeStyle`.
    ///
    /// # Example
    /// ```
    /// use canvas::Canvas;
    ///
    /// let canvas = Canvas::new(200, 100);
    /// let mut ctx = canvas.get_context("2d").unwrap();
    ///
    /// let mut gradient = ctx.create_linear_gradient(0.0, 0.0, 200.0, 0.0);
    /// gradient.add_color_stop(0.0, "red");
    /// gradient.add_color_stop(1.0, "blue");
    /// ctx.set_fill_style_gradient(&gradient);
    /// ctx.fill_rect(0.0, 0.0, 200.0, 100.0);
    /// ```
    pub fn create_linear_gradient(&self, x0: f64, y0: f64, x1: f64, y1: f64) -> LinearGradient {
        LinearGradient::new(x0, y0, x1, y1)
    }

    /// Create a radial gradient from inner circle (x0, y0, r0) to outer circle (x1, y1, r1).
    ///
    /// Returns a `RadialGradient` that can be configured with color stops
    /// and then assigned to `fillStyle` or `strokeStyle`.
    ///
    /// # Example
    /// ```
    /// use canvas::Canvas;
    ///
    /// let canvas = Canvas::new(100, 100);
    /// let mut ctx = canvas.get_context("2d").unwrap();
    ///
    /// let mut gradient = ctx.create_radial_gradient(50.0, 50.0, 10.0, 50.0, 50.0, 50.0);
    /// gradient.add_color_stop(0.0, "red");
    /// gradient.add_color_stop(1.0, "blue");
    /// ctx.set_fill_style_radial_gradient(&gradient);
    /// ctx.fill_rect(0.0, 0.0, 100.0, 100.0);
    /// ```
    pub fn create_radial_gradient(
        &self,
        x0: f64,
        y0: f64,
        r0: f64,
        x1: f64,
        y1: f64,
        r1: f64,
    ) -> RadialGradient {
        RadialGradient::new(x0, y0, r0, x1, y1, r1)
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn color_to_css(c: Color) -> String {
    format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a as f64 / 255.0)
}

fn style_to_string(style: &Style) -> String {
    match style {
        Style::Color(c) => color_to_css(*c),
        Style::LinearGradient(g) => {
            format!("LinearGradient({},{}) -> ({},{}) with {} stops",
                g.x0, g.y0, g.x1, g.y1, g.stops.len())
        }
        Style::RadialGradient(g) => {
            format!("RadialGradient({},{},{}) -> ({},{},{}) with {} stops",
                g.x0, g.y0, g.r0, g.x1, g.y1, g.r1, g.stops.len())
        }
    }
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
