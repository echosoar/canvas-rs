use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::color::Color;
use crate::font::Font;
use crate::font::CharBitmap;
use crate::gradient::{LinearGradient, RadialGradient, Style};
use crate::image::ImageData;
use crate::path::{Path, PathCommand, RoundRectPath};
use crate::render::{self, LineCap, TextAlign};

thread_local! {
    static FONT_CACHE: RefCell<HashMap<String, Rc<Font>>> = RefCell::new(HashMap::new());
    static TEXT_GLYPH_CACHE: RefCell<HashMap<GlyphCacheKey, Rc<CachedGlyph>>> = RefCell::new(HashMap::new());
    static TEXT_RUN_CACHE: RefCell<HashMap<TextRunCacheKey, Rc<CachedTextRun>>> = RefCell::new(HashMap::new());
}

fn load_cached_font(font_name: &str) -> Option<Font> {
    let cache_key = if font_name.is_empty() {
        "common".to_string()
    } else {
        font_name.to_ascii_lowercase()
    };

    FONT_CACHE.with(|cache| {
        if let Some(font) = cache.borrow().get(&cache_key).cloned() {
            return Some((*font).clone());
        }

        let font = Font::load(font_name).ok()?;
        cache.borrow_mut().insert(cache_key, Rc::new(font.clone()));
        Some(font)
    })
}

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
        let common_font = load_cached_font("common");
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
            text_aa_grid: 4,
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
    text_aa_grid: usize,
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
    text_aa_grid: usize,

    // ── State Stack (for save/restore) ────────────────────────
    state_stack: Vec<ContextState>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct GlyphCacheKey {
    font_name: String,
    source_size: u32,
    font_size: u32,
    aa_grid: usize,
    ch: char,
}

#[derive(Debug)]
struct CachedGlyph {
    width: usize,
    height: usize,
    coverage: Vec<u8>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TextRunCacheKey {
    primary_font_name: String,
    primary_source_size: u32,
    fallback_font_name: Option<String>,
    fallback_source_size: Option<u32>,
    font_size: u32,
    aa_grid: usize,
    text: String,
}

#[derive(Debug)]
struct CachedTextRun {
    width: usize,
    height: usize,
    coverage: Vec<u8>,
}

enum TextRunSegment {
    Glyph(Rc<CachedGlyph>),
    Space(usize),
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
            self.font = load_cached_font(&self.font_family);
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

    /// Add a rounded rectangle to the current path.
    ///
    /// `radii` accepts 1 to 4 corner radii in CSS order:
    /// - 1 value: all corners
    /// - 2 values: top-left/bottom-right, top-right/bottom-left
    /// - 3 values: top-left, top-right/bottom-left, bottom-right
    /// - 4 values: top-left, top-right, bottom-right, bottom-left
    pub fn round_rect(&mut self, x: f64, y: f64, width: f64, height: f64, radii: &[f64]) {
        if width == 0.0 || height == 0.0 {
            return;
        }

        let left = x.min(x + width);
        let right = x.max(x + width);
        let top = y.min(y + height);
        let bottom = y.max(y + height);
        let rect_width = right - left;
        let rect_height = bottom - top;
        let radii = normalize_round_rect_radii(rect_width, rect_height, radii);

        self.path.commands.push(PathCommand::RoundRect(RoundRectPath {
            left,
            top,
            width: rect_width,
            height: rect_height,
            radii: [
                radii.top_left,
                radii.top_right,
                radii.bottom_right,
                radii.bottom_left,
            ],
        }));
    }

    /// Close the current sub-path by adding a straight line back to its start.
    pub fn close_path(&mut self) {
        self.path.commands.push(PathCommand::ClosePath);
    }

    /// Fill the interior of the current path with `fillStyle`.
    pub fn fill(&mut self) {
        if let Some(round_rect) = self.path.as_round_rect() {
            let style = self.fill_style.clone();
            render::fill_round_rect_style(
                &mut self.buffer.borrow_mut(),
                self.width,
                self.height,
                round_rect,
                &style,
                &self.clip,
            );
            return;
        }

        let sub_paths = self.path.flatten();
        let style = self.fill_style.clone();
        let mut buf = self.buffer.borrow_mut();
        for pts in &sub_paths {
            render::fill_subpath_style(&mut buf, self.width, self.height, pts, &style, &self.clip);
        }
    }

    /// Stroke the outline of the current path with `strokeStyle`.
    pub fn stroke(&mut self) {
        if let Some(round_rect) = self.path.as_round_rect() {
            let style = self.stroke_style.clone();
            render::stroke_round_rect_style(
                &mut self.buffer.borrow_mut(),
                self.width,
                self.height,
                round_rect,
                &style,
                self.line_width,
                &self.clip,
            );
            return;
        }

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
    #[inline]
    fn bitmap_value_at(bitmap: &[Vec<bool>], x: isize, y: isize) -> f64 {
        if x < 0 || y < 0 {
            return 0.0;
        }
        let yu = y as usize;
        let xu = x as usize;
        if yu >= bitmap.len() || xu >= bitmap[yu].len() {
            return 0.0;
        }
        if bitmap[yu][xu] { 1.0 } else { 0.0 }
    }

    #[inline]
    fn sample_bitmap_bilinear(bitmap: &[Vec<bool>], x: f64, y: f64) -> f64 {
        let x0 = x.floor() as isize;
        let y0 = y.floor() as isize;
        let x1 = x0 + 1;
        let y1 = y0 + 1;

        let tx = x - x0 as f64;
        let ty = y - y0 as f64;

        let v00 = Self::bitmap_value_at(bitmap, x0, y0);
        let v10 = Self::bitmap_value_at(bitmap, x1, y0);
        let v01 = Self::bitmap_value_at(bitmap, x0, y1);
        let v11 = Self::bitmap_value_at(bitmap, x1, y1);

        let v0 = v00 * (1.0 - tx) + v10 * tx;
        let v1 = v01 * (1.0 - tx) + v11 * tx;
        (v0 * (1.0 - ty) + v1 * ty).clamp(0.0, 1.0)
    }

    #[inline]
    fn sample_bitmap_nearest(bitmap: &[Vec<bool>], x: f64, y: f64) -> f64 {
        let xi = x.round() as isize;
        let yi = y.round() as isize;
        Self::bitmap_value_at(bitmap, xi, yi)
    }

    #[inline]
    fn coverage_value_at(coverage: &[u8], width: usize, height: usize, x: isize, y: isize) -> f64 {
        if x < 0 || y < 0 {
            return 0.0;
        }
        let xu = x as usize;
        let yu = y as usize;
        if xu >= width || yu >= height {
            return 0.0;
        }
        coverage[yu * width + xu] as f64 / 255.0
    }

    #[inline]
    fn sample_coverage_bilinear(coverage: &[u8], width: usize, height: usize, x: f64, y: f64) -> f64 {
        let x0 = x.floor() as isize;
        let y0 = y.floor() as isize;
        let x1 = x0 + 1;
        let y1 = y0 + 1;

        let tx = x - x0 as f64;
        let ty = y - y0 as f64;

        let v00 = Self::coverage_value_at(coverage, width, height, x0, y0);
        let v10 = Self::coverage_value_at(coverage, width, height, x1, y0);
        let v01 = Self::coverage_value_at(coverage, width, height, x0, y1);
        let v11 = Self::coverage_value_at(coverage, width, height, x1, y1);

        let v0 = v00 * (1.0 - tx) + v10 * tx;
        let v1 = v01 * (1.0 - tx) + v11 * tx;
        (v0 * (1.0 - ty) + v1 * ty).clamp(0.0, 1.0)
    }

    #[inline]
    fn sample_coverage_nearest(coverage: &[u8], width: usize, height: usize, x: f64, y: f64) -> f64 {
        let xi = x.round() as isize;
        let yi = y.round() as isize;
        Self::coverage_value_at(coverage, width, height, xi, yi)
    }

    #[inline]
    fn line_height(&self) -> f64 {
        self.font_size.max(1) as f64
    }

    fn text_lines<'a>(text: &'a str) -> Vec<&'a str> {
        text.split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
            .collect()
    }

    fn draw_cached_text_run(
        &self,
        buf: &mut Vec<u8>,
        text_run: &CachedTextRun,
        x: f64,
        y: f64,
        style: &Style,
    ) {
        match style {
            Style::Color(color) => {
                for dst_y in 0..text_run.height {
                    for dst_x in 0..text_run.width {
                        let alpha = text_run.coverage[dst_y * text_run.width + dst_x];
                        if alpha == 0 {
                            continue;
                        }

                        render::put_pixel_color_coverage_u8(
                            buf,
                            self.width,
                            self.height,
                            x as i64 + dst_x as i64,
                            y as i64 + dst_y as i64,
                            *color,
                            alpha,
                            &self.clip,
                        );
                    }
                }
            }
            _ => {
                for dst_y in 0..text_run.height {
                    for dst_x in 0..text_run.width {
                        let alpha = text_run.coverage[dst_y * text_run.width + dst_x];
                        if alpha == 0 {
                            continue;
                        }

                        render::put_pixel_style_coverage(
                            buf,
                            self.width,
                            self.height,
                            x as i64 + dst_x as i64,
                            y as i64 + dst_y as i64,
                            style,
                            alpha as f64 / 255.0,
                            &self.clip,
                        );
                    }
                }
            }
        }
    }

    fn draw_scaled_cached_text_run(
        &self,
        buf: &mut Vec<u8>,
        text_run: &CachedTextRun,
        x: f64,
        y: f64,
        scale: f64,
        style: &Style,
    ) {
        if text_run.width == 0 || text_run.height == 0 || scale <= 0.0 {
            return;
        }

        let scaled_height = (text_run.height as f64 * scale).ceil() as usize;
        let scaled_width = (text_run.width as f64 * scale).ceil() as usize;
        for dst_y in 0..scaled_height {
            for dst_x in 0..scaled_width {
                let grid = self.text_aa_grid;
                let coverage = if grid <= 1 {
                    let sample_x = dst_x as f64 / scale;
                    let sample_y = dst_y as f64 / scale;
                    Self::sample_coverage_nearest(&text_run.coverage, text_run.width, text_run.height, sample_x, sample_y)
                } else {
                    let mut accum = 0.0;
                    let total_samples = grid * grid;
                    for sy in 0..grid {
                        for sx in 0..grid {
                            let sample_x = ((dst_x as f64 + (sx as f64 + 0.5) / grid as f64) + 0.5) / scale - 0.5;
                            let sample_y = ((dst_y as f64 + (sy as f64 + 0.5) / grid as f64) + 0.5) / scale - 0.5;
                            accum += Self::sample_coverage_bilinear(
                                &text_run.coverage,
                                text_run.width,
                                text_run.height,
                                sample_x,
                                sample_y,
                            );
                        }
                    }
                    (accum / total_samples as f64).clamp(0.0, 1.0)
                };

                if coverage > 0.0 {
                    render::put_pixel_style_coverage(
                        buf,
                        self.width,
                        self.height,
                        x as i64 + dst_x as i64,
                        y as i64 + dst_y as i64,
                        style,
                        coverage,
                        &self.clip,
                    );
                }
            }
        }
    }

    fn get_cached_glyph(
        &self,
        font: &Font,
        ch: char,
        char_bm: &CharBitmap,
    ) -> Rc<CachedGlyph> {
        let key = GlyphCacheKey {
            font_name: font.name.clone(),
            source_size: font.config.size,
            font_size: self.font_size,
            aa_grid: self.text_aa_grid,
            ch,
        };

        TEXT_GLYPH_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if let Some(glyph) = cache.get(&key) {
                return Rc::clone(glyph);
            }

            let glyph = Rc::new(Self::build_cached_glyph(
                char_bm,
                font.config.size,
                self.font_size,
                self.text_aa_grid,
            ));
            cache.insert(key, Rc::clone(&glyph));
            glyph
        })
    }

    fn build_cached_glyph(
        char_bm: &CharBitmap,
        source_font_size: u32,
        target_font_size: u32,
        aa_grid: usize,
    ) -> CachedGlyph {
        let scale = target_font_size as f64 / source_font_size as f64;
        let height = (char_bm.height as f64 * scale).ceil() as usize;
        let width = (char_bm.width as f64 * scale).ceil() as usize;
        let mut coverage = vec![0u8; width * height];

        for dst_y in 0..height {
            for dst_x in 0..width {
                let value = if aa_grid <= 1 {
                    let sample_x = dst_x as f64 / scale;
                    let sample_y = dst_y as f64 / scale;
                    Self::sample_bitmap_nearest(&char_bm.bitmap, sample_x, sample_y)
                } else {
                    let mut accum = 0.0;
                    let total_samples = aa_grid * aa_grid;
                    for sy in 0..aa_grid {
                        for sx in 0..aa_grid {
                            let sample_x =
                                ((dst_x as f64 + (sx as f64 + 0.5) / aa_grid as f64) + 0.5) / scale - 0.5;
                            let sample_y =
                                ((dst_y as f64 + (sy as f64 + 0.5) / aa_grid as f64) + 0.5) / scale - 0.5;
                            accum += Self::sample_bitmap_bilinear(&char_bm.bitmap, sample_x, sample_y);
                        }
                    }
                    (accum / total_samples as f64).clamp(0.0, 1.0)
                };

                coverage[dst_y * width + dst_x] = (value * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }

        CachedGlyph { width, height, coverage }
    }

    #[inline]
    fn scaled_space_width(source_font_size: u32, target_font_size: u32) -> usize {
        let half_width = source_font_size / 2;
        (half_width as f64 * target_font_size as f64 / source_font_size as f64).ceil() as usize
    }

    fn build_cached_text_run(&self, primary_font: &Font, text: &str) -> CachedTextRun {
        let mut segments: Vec<TextRunSegment> = Vec::new();
        let mut total_width = 0usize;
        let mut max_height = 0usize;

        for ch in text.chars() {
            if let Some(char_bm) = primary_font.get_char(ch) {
                let glyph = self.get_cached_glyph(primary_font, ch, char_bm);
                total_width += glyph.width;
                max_height = max_height.max(glyph.height);
                segments.push(TextRunSegment::Glyph(glyph));
            } else if let Some(fallback_font) = self.common_font.as_ref() {
                if let Some(char_bm) = fallback_font.get_char(ch) {
                    let glyph = self.get_cached_glyph(fallback_font, ch, char_bm);
                    total_width += glyph.width;
                    max_height = max_height.max(glyph.height);
                    segments.push(TextRunSegment::Glyph(glyph));
                    continue;
                }
                if ch == ' ' {
                    let width = Self::scaled_space_width(primary_font.config.size, self.font_size);
                    total_width += width;
                    segments.push(TextRunSegment::Space(width));
                }
            } else if ch == ' ' {
                let width = Self::scaled_space_width(primary_font.config.size, self.font_size);
                total_width += width;
                segments.push(TextRunSegment::Space(width));
            }
        }

        if total_width == 0 || max_height == 0 {
            return CachedTextRun {
                width: total_width,
                height: max_height,
                coverage: Vec::new(),
            };
        }

        let mut coverage = vec![0u8; total_width * max_height];
        let mut current_x = 0usize;

        for segment in segments {
            match segment {
                TextRunSegment::Glyph(glyph) => {
                    for dst_y in 0..glyph.height {
                        let dst_row_start = dst_y * total_width + current_x;
                        let src_row_start = dst_y * glyph.width;
                        coverage[dst_row_start..dst_row_start + glyph.width]
                            .copy_from_slice(&glyph.coverage[src_row_start..src_row_start + glyph.width]);
                    }
                    current_x += glyph.width;
                }
                TextRunSegment::Space(width) => {
                    current_x += width;
                }
            }
        }

        CachedTextRun {
            width: total_width,
            height: max_height,
            coverage,
        }
    }

    fn get_cached_text_run(&self, primary_font: &Font, text: &str) -> Rc<CachedTextRun> {
        let fallback_font = self.common_font.as_ref();
        let key = TextRunCacheKey {
            primary_font_name: primary_font.name.clone(),
            primary_source_size: primary_font.config.size,
            fallback_font_name: fallback_font.map(|font| font.name.clone()),
            fallback_source_size: fallback_font.map(|font| font.config.size),
            font_size: self.font_size,
            aa_grid: self.text_aa_grid,
            text: text.to_string(),
        };

        TEXT_RUN_CACHE.with(|cache| {
            if let Some(text_run) = cache.borrow().get(&key) {
                return Rc::clone(text_run);
            }

            let text_run = Rc::new(self.build_cached_text_run(primary_font, text));
            let mut cache = cache.borrow_mut();
            if let Some(existing) = cache.get(&key) {
                return Rc::clone(existing);
            }
            cache.insert(key, Rc::clone(&text_run));
            text_run
        })
    }

    /// Set text antialias sample grid size. Values are clamped to [1, 8].
    pub fn set_text_antialias_grid(&mut self, grid: u32) {
        self.text_aa_grid = grid.clamp(1, 8) as usize;
    }

    /// Return current text antialias sample grid size.
    pub fn text_antialias_grid(&self) -> usize {
        self.text_aa_grid
    }

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
        self.ensure_font_loaded();

        let primary_font = self.font.as_ref().or(self.common_font.as_ref());
        if primary_font.is_none() || text.is_empty() {
            return;
        }

        let primary_font = primary_font.unwrap();
        let lines = Self::text_lines(text);
        let line_height = self.line_height();

        let mut buf = self.buffer.borrow_mut();
        let style = self.fill_style.clone();
        for (index, line) in lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }

            let text_run = self.get_cached_text_run(primary_font, line);
            if text_run.width == 0 || text_run.height == 0 {
                continue;
            }

            let x_offset = x - self.text_align.calculate_x_offset(text_run.width as f64);
            let y_offset = y + index as f64 * line_height;
            self.draw_cached_text_run(&mut buf, &text_run, x_offset, y_offset, &style);
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

        if text.is_empty() || max_width <= 0.0 {
            return;
        }

        // Determine which font to use as primary, and common_font as fallback
        let primary_font = self.font.as_ref().or(self.common_font.as_ref());
        if primary_font.is_none() {
            return;
        }

        let primary_font = primary_font.unwrap();
        let lines = Self::text_lines(text);
        let mut line_runs = Vec::with_capacity(lines.len());
        let mut widest_width = 0usize;
        for line in &lines {
            if line.is_empty() {
                line_runs.push(None);
                continue;
            }

            let text_run = self.get_cached_text_run(primary_font, line);
            widest_width = widest_width.max(text_run.width);
            line_runs.push(Some(text_run));
        }

        if widest_width == 0 {
            return;
        }
        if widest_width as f64 <= max_width {
            self.fill_text(text, x, y);
            return;
        }

        let scale = max_width / widest_width as f64;
        let style = self.fill_style.clone();
        let line_height = self.line_height() * scale;

        let mut buf = self.buffer.borrow_mut();
        for (index, text_run) in line_runs.iter().enumerate() {
            let Some(text_run) = text_run else {
                continue;
            };

            let scaled_width = (text_run.width as f64 * scale).ceil() as usize;
            let x_offset = x - self.text_align.calculate_x_offset(scaled_width as f64);
            let y_offset = y + index as f64 * line_height;
            self.draw_scaled_cached_text_run(&mut buf, text_run, x_offset, y_offset, scale, &style);
        }
    }

    /// Measure text width with current font settings.
    /// Returns the width in pixels.
    /// Characters not found in the font will fallback to common font.
    pub fn measure_text(&self, text: &str) -> f64 {
        let font = load_cached_font(&self.font_family);
        let common_font = load_cached_font("common");

        let primary_font = font.as_ref().or(common_font.as_ref());
        if let Some(font) = primary_font {
            Self::text_lines(text)
                .into_iter()
                .map(|line| {
                    let (_, width, _) = font.render_text_with_fallback(
                        line,
                        self.font_size,
                        common_font.as_ref(),
                    );
                    width as f64
                })
                .fold(0.0, f64::max)
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
            text_aa_grid: self.text_aa_grid,
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
            self.text_aa_grid = state.text_aa_grid;
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

#[derive(Clone, Copy, Debug)]
struct RoundRectRadii {
    top_left: f64,
    top_right: f64,
    bottom_right: f64,
    bottom_left: f64,
}

fn normalize_round_rect_radii(width: f64, height: f64, radii: &[f64]) -> RoundRectRadii {
    let mut corners = match radii {
        [] => [0.0, 0.0, 0.0, 0.0],
        [a] => [*a, *a, *a, *a],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d, ..] => [*a, *b, *c, *d],
    };

    for radius in &mut corners {
        *radius = if radius.is_finite() { radius.max(0.0) } else { 0.0 };
    }

    let scale = [
        if corners[0] + corners[1] > 0.0 { width / (corners[0] + corners[1]) } else { 1.0 },
        if corners[3] + corners[2] > 0.0 { width / (corners[3] + corners[2]) } else { 1.0 },
        if corners[0] + corners[3] > 0.0 { height / (corners[0] + corners[3]) } else { 1.0 },
        if corners[1] + corners[2] > 0.0 { height / (corners[1] + corners[2]) } else { 1.0 },
    ]
    .into_iter()
    .fold(1.0, f64::min)
    .min(1.0);

    if scale < 1.0 {
        for radius in &mut corners {
            *radius *= scale;
        }
    }

    RoundRectRadii {
        top_left: corners[0],
        top_right: corners[1],
        bottom_right: corners[2],
        bottom_left: corners[3],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_text_run_reuses_same_entry() {
        let canvas = Canvas::new(32, 32);
        let mut ctx = canvas.get_context("2d").unwrap();
        ctx.set_font("16px common");
        ctx.ensure_font_loaded();

        let primary_font = ctx.font.as_ref().or(ctx.common_font.as_ref()).unwrap();
        let run1 = ctx.get_cached_text_run(primary_font, "AB 12");
        let run2 = ctx.get_cached_text_run(primary_font, "AB 12");

        assert!(Rc::ptr_eq(&run1, &run2));
        assert!(run1.width > 0);
        assert!(run1.height > 0);
    }

    #[test]
    fn cached_text_run_matches_measured_width() {
        let canvas = Canvas::new(64, 64);
        let mut ctx = canvas.get_context("2d").unwrap();
        ctx.set_font("20px common");
        ctx.ensure_font_loaded();

        let primary_font = ctx.font.as_ref().or(ctx.common_font.as_ref()).unwrap();
        let run = ctx.get_cached_text_run(primary_font, "A A");

        assert_eq!(run.width as f64, ctx.measure_text("A A"));
    }

    #[test]
    fn measure_text_uses_widest_line_for_multiline_text() {
        let canvas = Canvas::new(64, 64);
        let mut ctx = canvas.get_context("2d").unwrap();
        ctx.set_font("20px common");

        assert_eq!(ctx.measure_text("AB\nABCD"), ctx.measure_text("ABCD"));
    }
}
