// Rasterizer helper functions often need many parameters (canvas buffer,
// dimensions, coordinates, color, clip).  Allow the lint globally for this
// module rather than annotating every function individually.
#![allow(clippy::too_many_arguments, clippy::ptr_arg)]

use crate::color::Color;
use crate::gradient::Style;
use crate::image::ImageData;

const SHAPE_AA_GRID: usize = 4;
const GEOMETRY_EPSILON: f64 = 1e-9;

// ── Pixel helpers ────────────────────────────────────────────────────────────

/// Set a pixel, applying source-over alpha blending and honouring the clip
/// mask.  `clip` is `None` (no clip) or a slice of booleans indexed by
/// `y * width + x`; a `true` value means "draw".
#[inline]
pub fn put_pixel(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    x: i64,
    y: i64,
    color: Color,
    clip: &Option<Vec<bool>>,
) {
    if x < 0 || y < 0 || x >= width as i64 || y >= height as i64 {
        return;
    }
    let idx = (y as u32 * width + x as u32) as usize;
    if let Some(mask) = clip {
        if !mask[idx] {
            return;
        }
    }
    let base = idx * 4;
    let dst = Color::rgba(buf[base], buf[base + 1], buf[base + 2], buf[base + 3]);
    let result = color.blend_onto(dst);
    buf[base] = result.r;
    buf[base + 1] = result.g;
    buf[base + 2] = result.b;
    buf[base + 3] = result.a;
}

// ── Rectangle primitives ─────────────────────────────────────────────────────

/// Fill an axis-aligned rectangle.
pub fn fill_rect(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: Color,
    clip: &Option<Vec<bool>>,
) {
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let x1 = (x + w).ceil() as i64;
    let y1 = (y + h).ceil() as i64;
    for py in y0..y1 {
        for px in x0..x1 {
            put_pixel(buf, width, height, px, py, color, clip);
        }
    }
}

/// Stroke an axis-aligned rectangle outline.
pub fn stroke_rect(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: Color,
    line_width: f64,
    line_cap: LineCap,
    clip: &Option<Vec<bool>>,
) {
    // Draw the four edges as thick line segments.
    let (x0, y0, x1, y1) = (x, y, x + w, y + h);
    draw_thick_line(buf, width, height, x0, y0, x1, y0, color, line_width, line_cap, clip);
    draw_thick_line(buf, width, height, x1, y0, x1, y1, color, line_width, line_cap, clip);
    draw_thick_line(buf, width, height, x1, y1, x0, y1, color, line_width, line_cap, clip);
    draw_thick_line(buf, width, height, x0, y1, x0, y0, color, line_width, line_cap, clip);
}

/// Clear a rectangle to transparent black.
pub fn clear_rect(buf: &mut Vec<u8>, width: u32, height: u32, x: f64, y: f64, w: f64, h: f64) {
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let x1 = (x + w).ceil() as i64;
    let y1 = (y + h).ceil() as i64;
    for py in y0..y1 {
        for px in x0..x1 {
            if px < 0 || py < 0 || px >= width as i64 || py >= height as i64 {
                continue;
            }
            let base = (py as u32 * width + px as u32) as usize * 4;
            buf[base] = 0;
            buf[base + 1] = 0;
            buf[base + 2] = 0;
            buf[base + 3] = 0;
        }
    }
}

// ── Line drawing ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

impl LineCap {
    pub fn parse_cap(s: &str) -> LineCap {
        match s {
            "round" => LineCap::Round,
            "square" => LineCap::Square,
            _ => LineCap::Butt,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            LineCap::Butt => "butt",
            LineCap::Round => "round",
            LineCap::Square => "square",
        }
    }
}

/// Text alignment for fill_text operations.
/// `start` and `left` are equivalent (left-aligned).
/// `end` and `right` are equivalent (right-aligned).
/// `center` centers the text at the given x position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlign {
    Start,
    End,
    Left,
    Right,
    Center,
}

impl TextAlign {
    /// Parse a text alignment string.
    /// Defaults to `Start` for invalid values.
    pub fn parse_align(s: &str) -> TextAlign {
        match s {
            "start" => TextAlign::Start,
            "end" => TextAlign::End,
            "left" => TextAlign::Left,
            "right" => TextAlign::Right,
            "center" => TextAlign::Center,
            _ => TextAlign::Start,
        }
    }

    /// Convert to string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            TextAlign::Start => "start",
            TextAlign::End => "end",
            TextAlign::Left => "left",
            TextAlign::Right => "right",
            TextAlign::Center => "center",
        }
    }

    /// Calculate the x offset for text rendering based on alignment.
    /// Returns the offset to subtract from the given x position.
    /// - Start/Left: no offset (x is left edge)
    /// - End/Right: full text width offset (x is right edge)
    /// - Center: half text width offset (x is center)
    pub fn calculate_x_offset(self, text_width: f64) -> f64 {
        match self {
            TextAlign::Start | TextAlign::Left => 0.0,
            TextAlign::End | TextAlign::Right => text_width,
            TextAlign::Center => text_width / 2.0,
        }
    }
}

/// Fill a circle (disc) centred at `(cx, cy)` with the given `radius`.
fn fill_disc(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    cx: f64,
    cy: f64,
    radius: f64,
    color: Color,
    clip: &Option<Vec<bool>>,
) {
    let r = radius.ceil() as i64;
    let cx_i = cx.round() as i64;
    let cy_i = cy.round() as i64;
    let r2 = radius * radius;
    for dy in -r..=r {
        for dx in -r..=r {
            let d2 = (dx * dx + dy * dy) as f64;
            if d2 <= r2 + 0.5 {
                put_pixel(buf, width, height, cx_i + dx, cy_i + dy, color, clip);
            }
        }
    }
}

/// Draw a thick line from `(x0,y0)` to `(x1,y1)` with the given `line_width`
/// and `line_cap` style.
///
/// Algorithm: build a filled convex quadrilateral for the body of the line,
/// plus cap geometry at each end, and rasterise it all via scanline fill.
pub fn draw_thick_line(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    color: Color,
    line_width: f64,
    cap: LineCap,
    clip: &Option<Vec<bool>>,
) {
    let hw = line_width / 2.0; // half-width

    let dx = x1 - x0;
    let dy = y1 - y0;
    let len = (dx * dx + dy * dy).sqrt();

    if len < 1e-9 {
        // Degenerate (point): just draw a disc/square at the point.
        match cap {
            LineCap::Round => fill_disc(buf, width, height, x0, y0, hw, color, clip),
            _ => fill_rect(
                buf, width, height,
                x0 - hw, y0 - hw, line_width, line_width,
                color, clip,
            ),
        }
        return;
    }

    // Unit perpendicular (normal) vector.
    let nx = -dy / len;
    let ny = dx / len;
    // Unit direction vector (for square caps).
    let ux = dx / len;
    let uy = dy / len;

    // Extend endpoints for square cap.
    let (p0x, p0y, p1x, p1y) = match cap {
        LineCap::Square => (
            x0 - ux * hw,
            y0 - uy * hw,
            x1 + ux * hw,
            y1 + uy * hw,
        ),
        _ => (x0, y0, x1, y1),
    };

    // Four corners of the parallelogram.
    let corners = [
        (p0x + nx * hw, p0y + ny * hw),
        (p0x - nx * hw, p0y - ny * hw),
        (p1x - nx * hw, p1y - ny * hw),
        (p1x + nx * hw, p1y + ny * hw),
    ];

    fill_polygon(buf, width, height, &corners, color, clip);

    // Round caps: add semicircles at each endpoint.
    if cap == LineCap::Round {
        fill_disc(buf, width, height, x0, y0, hw, color, clip);
        fill_disc(buf, width, height, x1, y1, hw, color, clip);
    }
}

// ── Scanline polygon fill ────────────────────────────────────────────────────

/// Fill an arbitrary simple polygon using the scanline + even-odd rule.
pub fn fill_polygon(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    pts: &[(f64, f64)],
    color: Color,
    clip: &Option<Vec<bool>>,
) {
    if pts.len() < 3 {
        return;
    }
    let min_y = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max_y = pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);

    let y_start = min_y.floor() as i64;
    let y_end = max_y.ceil() as i64;

    let n = pts.len();

    for py in y_start..=y_end {
        let fy = py as f64 + 0.5; // sample at centre of pixel row
        let mut crossings: Vec<f64> = Vec::new();
        for i in 0..n {
            let (x0, y0) = pts[i];
            let (x1, y1) = pts[(i + 1) % n];
            if (y0 <= fy && y1 > fy) || (y1 <= fy && y0 > fy) {
                let t = (fy - y0) / (y1 - y0);
                crossings.push(x0 + t * (x1 - x0));
            }
        }
        crossings.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut i = 0;
        while i + 1 < crossings.len() {
            let xa = crossings[i].floor() as i64;
            let xb = crossings[i + 1].ceil() as i64;
            for px in xa..xb {
                put_pixel(buf, width, height, px, py, color, clip);
            }
            i += 2;
        }
    }
}

// ── Path stroke & fill ───────────────────────────────────────────────────────

/// Stroke a list of (connected) polyline points.
pub fn stroke_polyline(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    pts: &[(f64, f64)],
    color: Color,
    line_width: f64,
    cap: LineCap,
    clip: &Option<Vec<bool>>,
) {
    if pts.len() < 2 {
        if pts.len() == 1 {
            // Single point – draw a disc/square.
            match cap {
                LineCap::Round => {
                    fill_disc(buf, width, height, pts[0].0, pts[0].1, line_width / 2.0, color, clip)
                }
                _ => fill_rect(
                    buf, width, height,
                    pts[0].0 - line_width / 2.0,
                    pts[0].1 - line_width / 2.0,
                    line_width,
                    line_width,
                    color, clip,
                ),
            }
        }
        return;
    }

    // Draw all segments using Butt caps for the segment body so that
    // interior joints never get incorrect cap geometry.  Endpoint caps are
    // added separately below.
    for i in 0..pts.len() - 1 {
        let (x0, y0) = pts[i];
        let (x1, y1) = pts[i + 1];
        draw_thick_line(buf, width, height, x0, y0, x1, y1, color, line_width, LineCap::Butt, clip);
    }

    // Add the requested cap to the true start and end of the polyline.
    let hw = line_width / 2.0;
    match cap {
        LineCap::Butt => {} // butt caps are flush with the body; nothing extra.
        LineCap::Round => {
            fill_disc(buf, width, height, pts[0].0, pts[0].1, hw, color, clip);
            let last = *pts.last().unwrap();
            fill_disc(buf, width, height, last.0, last.1, hw, color, clip);
        }
        LineCap::Square => {
            // Square cap: extend a square beyond each endpoint in the
            // direction of the adjacent segment.
            add_square_cap(buf, width, height, pts[0], pts[1], hw, color, clip, true);
            let n = pts.len();
            add_square_cap(buf, width, height, pts[n - 1], pts[n - 2], hw, color, clip, true);
        }
    }
}

/// Add a square cap at `tip`, extending away from `neighbour`.
fn add_square_cap(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    tip: (f64, f64),
    neighbour: (f64, f64),
    hw: f64,
    color: Color,
    clip: &Option<Vec<bool>>,
    _outward: bool,
) {
    let dx = tip.0 - neighbour.0;
    let dy = tip.1 - neighbour.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        return;
    }
    let ux = dx / len;  // unit vector tip→outward
    let uy = dy / len;
    let nx = -uy;       // unit perpendicular
    let ny = ux;
    // Rectangle from the tip outward by hw.
    let corners = [
        (tip.0 + nx * hw, tip.1 + ny * hw),
        (tip.0 - nx * hw, tip.1 - ny * hw),
        (tip.0 - nx * hw + ux * hw, tip.1 - ny * hw + uy * hw),
        (tip.0 + nx * hw + ux * hw, tip.1 + ny * hw + uy * hw),
    ];
    fill_polygon(buf, width, height, &corners, color, clip);
}

/// Fill a closed sub-path (list of points) using the scanline algorithm.
pub fn fill_subpath(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    pts: &[(f64, f64)],
    color: Color,
    clip: &Option<Vec<bool>>,
) {
    // Ensure the polygon is closed.
    let mut polygon: Vec<(f64, f64)> = pts.to_vec();
    if polygon.len() >= 2 {
        let first = polygon[0];
        let last = *polygon.last().unwrap();
        if (first.0 - last.0).abs() > 1e-9 || (first.1 - last.1).abs() > 1e-9 {
            polygon.push(first);
        }
    }
    fill_polygon(buf, width, height, &polygon, color, clip);
}

// ── Clip mask building ───────────────────────────────────────────────────────

/// Rasterise a list of sub-paths into a boolean clip mask (true = inside).
/// Uses the even-odd fill rule, intersected with any existing mask.
pub fn build_clip_mask(
    canvas_width: u32,
    canvas_height: u32,
    sub_paths: &[Vec<(f64, f64)>],
    existing: &Option<Vec<bool>>,
) -> Vec<bool> {
    let total = (canvas_width * canvas_height) as usize;
    // Start with all true (nothing clipped).
    let mut mask = vec![true; total];

    // Build a fill bitmap for the new path.
    let mut fill_buf = vec![0u8; total * 4]; // dummy RGBA buffer
    let no_clip: Option<Vec<bool>> = None;
    let fill_color = Color::rgba(255, 255, 255, 255);

    for pts in sub_paths {
        fill_subpath(&mut fill_buf, canvas_width, canvas_height, pts, fill_color, &no_clip);
    }

    // Convert fill buffer to boolean mask.
    for i in 0..total {
        let inside = fill_buf[i * 4 + 3] > 0;
        mask[i] = inside;
        // Intersect with the existing clip mask.
        if let Some(existing_mask) = existing {
            mask[i] = mask[i] && existing_mask[i];
        }
    }
    mask
}

// ── Image drawing ────────────────────────────────────────────────────────────

/// Draw an `ImageData` onto the buffer at position `(dx, dy)`.
pub fn draw_image(
    buf: &mut Vec<u8>,
    canvas_width: u32,
    canvas_height: u32,
    image: &ImageData,
    dx: f64,
    dy: f64,
    clip: &Option<Vec<bool>>,
) {
    draw_image_region(
        buf,
        canvas_width,
        canvas_height,
        image,
        0.0, 0.0, image.width as f64, image.height as f64,
        dx, dy, image.width as f64, image.height as f64,
        clip,
    );
}

/// Draw a region of `image` (sx,sy,sw,sh) scaled into (dx,dy,dw,dh).
///
/// Negative values for `sw`, `sh`, `dw`, or `dh` are supported per the
/// HTML Canvas spec: the sub-rectangle is grown in the opposite direction but
/// pixels are always processed in the original direction (no flip).
pub fn draw_image_region(
    buf: &mut Vec<u8>,
    canvas_width: u32,
    canvas_height: u32,
    image: &ImageData,
    mut sx: f64, mut sy: f64, mut sw: f64, mut sh: f64,
    mut dx: f64, mut dy: f64, mut dw: f64, mut dh: f64,
    clip: &Option<Vec<bool>>,
) {
    // Return early on zero dimensions (nothing to paint).
    if dw == 0.0 || dh == 0.0 || sw == 0.0 || sh == 0.0 {
        return;
    }
    // Normalize negative source dimensions: shift origin, keep positive size.
    if sw < 0.0 { sx += sw; sw = -sw; }
    if sh < 0.0 { sy += sh; sh = -sh; }
    // Normalize negative destination dimensions similarly (no flip per spec).
    if dw < 0.0 { dx += dw; dw = -dw; }
    if dh < 0.0 { dy += dh; dh = -dh; }

    let x0 = dx.floor() as i64;
    let y0 = dy.floor() as i64;
    let x1 = (dx + dw).ceil() as i64;
    let y1 = (dy + dh).ceil() as i64;

    let scale_x = sw / dw;
    let scale_y = sh / dh;

    for py in y0..y1 {
        for px in x0..x1 {
            // Map destination pixel back to source coordinates.
            let src_x = sx + (px as f64 - dx + 0.5) * scale_x;
            let src_y = sy + (py as f64 - dy + 0.5) * scale_y;
            let color = image.sample(src_x, src_y);
            put_pixel(buf, canvas_width, canvas_height, px, py, color, clip);
        }
    }
}

// ── Style-based rendering (supports gradients) ─────────────────────────────────

/// Set a pixel using a Style (which may be a color or gradient).
#[inline]
pub fn put_pixel_style(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    x: i64,
    y: i64,
    style: &Style,
    clip: &Option<Vec<bool>>,
) {
    if x < 0 || y < 0 || x >= width as i64 || y >= height as i64 {
        return;
    }
    let idx = (y as u32 * width + x as u32) as usize;
    if let Some(mask) = clip {
        if !mask[idx] {
            return;
        }
    }
    if let Style::Color(color) = style {
        let base = idx * 4;
        let dst_a = buf[base + 3];
        if color.a == 255 || dst_a == 0 {
            buf[base] = color.r;
            buf[base + 1] = color.g;
            buf[base + 2] = color.b;
            buf[base + 3] = color.a;
            return;
        }
    }
    // Get color at this pixel position for gradient support
    let color = style.color_at(x as f64, y as f64);
    let base = idx * 4;
    let dst = Color::rgba(buf[base], buf[base + 1], buf[base + 2], buf[base + 3]);
    let result = color.blend_onto(dst);
    buf[base] = result.r;
    buf[base + 1] = result.g;
    buf[base + 2] = result.b;
    buf[base + 3] = result.a;
}

/// Set a pixel using a Style and a coverage factor in [0, 1].
///
/// This is primarily used by text antialiasing where the destination pixel is
/// only partially covered by the source glyph.
#[inline]
pub fn put_pixel_style_coverage(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    x: i64,
    y: i64,
    style: &Style,
    coverage: f64,
    clip: &Option<Vec<bool>>,
) {
    if coverage <= 0.0 {
        return;
    }
    if x < 0 || y < 0 || x >= width as i64 || y >= height as i64 {
        return;
    }
    let idx = (y as u32 * width + x as u32) as usize;
    if let Some(mask) = clip {
        if !mask[idx] {
            return;
        }
    }

    if let Style::Color(color) = style {
        let alpha = ((color.a as f64) * coverage.clamp(0.0, 1.0)).round().clamp(0.0, 255.0) as u8;
        put_pixel_color_coverage_u8(buf, width, height, x, y, *color, alpha, clip);
        return;
    }

    let cov = coverage.clamp(0.0, 1.0);
    let mut color = style.color_at(x as f64, y as f64);
    if cov < 1.0 {
        color.a = ((color.a as f64) * cov).round().clamp(0.0, 255.0) as u8;
        if color.a == 0 {
            return;
        }
    }

    let base = idx * 4;
    let dst = Color::rgba(buf[base], buf[base + 1], buf[base + 2], buf[base + 3]);
    let result = color.blend_onto(dst);
    buf[base] = result.r;
    buf[base + 1] = result.g;
    buf[base + 2] = result.b;
    buf[base + 3] = result.a;
}

/// Set a pixel using a solid color and a coverage factor in [0, 1].
///
/// This avoids per-pixel style dispatch for hot solid-color text rendering.
#[inline]
pub fn put_pixel_color_coverage(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    x: i64,
    y: i64,
    color: Color,
    coverage: f64,
    clip: &Option<Vec<bool>>,
) {
    if coverage <= 0.0 {
        return;
    }
    let alpha = ((color.a as f64) * coverage.clamp(0.0, 1.0)).round().clamp(0.0, 255.0) as u8;
    put_pixel_color_coverage_u8(buf, width, height, x, y, color, alpha, clip);
}

/// Set a pixel using a solid color and a precomputed alpha value in [0, 255].
#[inline]
pub fn put_pixel_color_coverage_u8(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    x: i64,
    y: i64,
    color: Color,
    alpha: u8,
    clip: &Option<Vec<bool>>,
) {
    if alpha == 0 {
        return;
    }
    if x < 0 || y < 0 || x >= width as i64 || y >= height as i64 {
        return;
    }
    let idx = (y as u32 * width + x as u32) as usize;
    if let Some(mask) = clip {
        if !mask[idx] {
            return;
        }
    }

    let base = idx * 4;
    let dst_a = buf[base + 3] as u32;
    if dst_a == 0 {
        buf[base] = color.r;
        buf[base + 1] = color.g;
        buf[base + 2] = color.b;
        buf[base + 3] = alpha;
        return;
    }

    if alpha == 255 {
        buf[base] = color.r;
        buf[base + 1] = color.g;
        buf[base + 2] = color.b;
        buf[base + 3] = 255;
        return;
    }

    let sa = alpha as u32;
    let inv_sa = 255 - sa;
    let out_a = sa + dst_a * inv_sa / 255;
    if out_a == 0 {
        buf[base] = 0;
        buf[base + 1] = 0;
        buf[base + 2] = 0;
        buf[base + 3] = 0;
        return;
    }

    let dst_r = buf[base] as u32;
    let dst_g = buf[base + 1] as u32;
    let dst_b = buf[base + 2] as u32;
    buf[base] = ((color.r as u32 * sa + dst_r * dst_a * inv_sa / 255) / out_a) as u8;
    buf[base + 1] = ((color.g as u32 * sa + dst_g * dst_a * inv_sa / 255) / out_a) as u8;
    buf[base + 2] = ((color.b as u32 * sa + dst_b * dst_a * inv_sa / 255) / out_a) as u8;
    buf[base + 3] = out_a.min(255) as u8;
}

/// Fill an axis-aligned rectangle with a Style.
pub fn fill_rect_style(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    style: &Style,
    clip: &Option<Vec<bool>>,
) {
    if w == 0.0 || h == 0.0 {
        return;
    }

    let left = x.min(x + w);
    let right = x.max(x + w);
    let top = y.min(y + h);
    let bottom = y.max(y + h);

    if is_nearly_integer(left)
        && is_nearly_integer(right)
        && is_nearly_integer(top)
        && is_nearly_integer(bottom)
    {
        let x0 = left.round() as i64;
        let y0 = top.round() as i64;
        let x1 = right.round() as i64;
        let y1 = bottom.round() as i64;
        for py in y0..y1 {
            for px in x0..x1 {
                put_pixel_style(buf, width, height, px, py, style, clip);
            }
        }
        return;
    }

    let x0 = left.floor() as i64;
    let y0 = top.floor() as i64;
    let x1 = right.ceil() as i64;
    let y1 = bottom.ceil() as i64;
    for py in y0..y1 {
        let pixel_top = py as f64;
        let pixel_bottom = pixel_top + 1.0;
        let overlap_y = (bottom.min(pixel_bottom) - top.max(pixel_top)).clamp(0.0, 1.0);
        if overlap_y <= 0.0 {
            continue;
        }
        for px in x0..x1 {
            let pixel_left = px as f64;
            let pixel_right = pixel_left + 1.0;
            let overlap_x = (right.min(pixel_right) - left.max(pixel_left)).clamp(0.0, 1.0);
            let coverage = overlap_x * overlap_y;
            if coverage > 0.0 {
                put_pixel_style_coverage(buf, width, height, px, py, style, coverage, clip);
            }
        }
    }
}

/// Stroke an axis-aligned rectangle outline with a Style.
pub fn stroke_rect_style(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    style: &Style,
    line_width: f64,
    line_cap: LineCap,
    clip: &Option<Vec<bool>>,
) {
    let (x0, y0, x1, y1) = (x, y, x + w, y + h);
    draw_thick_line_style(buf, width, height, x0, y0, x1, y0, style, line_width, line_cap, clip);
    draw_thick_line_style(buf, width, height, x1, y0, x1, y1, style, line_width, line_cap, clip);
    draw_thick_line_style(buf, width, height, x1, y1, x0, y1, style, line_width, line_cap, clip);
    draw_thick_line_style(buf, width, height, x0, y1, x0, y0, style, line_width, line_cap, clip);
}

/// Fill a circle (disc) centred at `(cx, cy)` with the given `radius` using a Style.
fn fill_disc_style(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    cx: f64,
    cy: f64,
    radius: f64,
    style: &Style,
    clip: &Option<Vec<bool>>,
) {
    let r2 = radius * radius;
    let x0 = (cx - radius).floor() as i64;
    let y0 = (cy - radius).floor() as i64;
    let x1 = (cx + radius).ceil() as i64;
    let y1 = (cy + radius).ceil() as i64;

    for py in y0..=y1 {
        for px in x0..=x1 {
            let coverage = supersample_pixel_coverage(px, py, |sample_x, sample_y| {
                let dx = sample_x - cx;
                let dy = sample_y - cy;
                dx * dx + dy * dy <= r2
            });
            if coverage > 0.0 {
                put_pixel_style_coverage(buf, width, height, px, py, style, coverage, clip);
            }
        }
    }
}

/// Draw a thick line with a Style.
pub fn draw_thick_line_style(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    style: &Style,
    line_width: f64,
    cap: LineCap,
    clip: &Option<Vec<bool>>,
) {
    let hw = line_width / 2.0;

    let dx = x1 - x0;
    let dy = y1 - y0;
    if let Some((left, top, width_rect, height_rect, round_caps)) =
        axis_aligned_line_rect(x0, y0, x1, y1, hw, cap)
    {
        fill_rect_style(buf, width, height, left, top, width_rect, height_rect, style, clip);
        if round_caps {
            fill_disc_style(buf, width, height, x0, y0, hw, style, clip);
            fill_disc_style(buf, width, height, x1, y1, hw, style, clip);
        }
        return;
    }

    let len = (dx * dx + dy * dy).sqrt();

    if len < 1e-9 {
        match cap {
            LineCap::Round => fill_disc_style(buf, width, height, x0, y0, hw, style, clip),
            _ => fill_rect_style(
                buf, width, height,
                x0 - hw, y0 - hw, line_width, line_width,
                style, clip,
            ),
        }
        return;
    }

    let nx = -dy / len;
    let ny = dx / len;
    let ux = dx / len;
    let uy = dy / len;

    let (p0x, p0y, p1x, p1y) = match cap {
        LineCap::Square => (
            x0 - ux * hw,
            y0 - uy * hw,
            x1 + ux * hw,
            y1 + uy * hw,
        ),
        _ => (x0, y0, x1, y1),
    };

    let corners = [
        (p0x + nx * hw, p0y + ny * hw),
        (p0x - nx * hw, p0y - ny * hw),
        (p1x - nx * hw, p1y - ny * hw),
        (p1x + nx * hw, p1y + ny * hw),
    ];

    fill_polygon_style(buf, width, height, &corners, style, clip);

    if cap == LineCap::Round {
        fill_disc_style(buf, width, height, x0, y0, hw, style, clip);
        fill_disc_style(buf, width, height, x1, y1, hw, style, clip);
    }
}

/// Fill an arbitrary simple polygon using a Style.
pub fn fill_polygon_style(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    pts: &[(f64, f64)],
    style: &Style,
    clip: &Option<Vec<bool>>,
) {
    if pts.len() < 3 {
        return;
    }
    let min_x = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let max_x = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let min_y = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max_y = pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);

    let x_start = min_x.floor() as i64;
    let x_end = max_x.ceil() as i64;
    let y_start = min_y.floor() as i64;
    let y_end = max_y.ceil() as i64;

    for py in y_start..y_end {
        for px in x_start..x_end {
            let coverage = supersample_pixel_coverage(px, py, |sample_x, sample_y| {
                point_in_polygon_even_odd(pts, sample_x, sample_y)
            });
            if coverage > 0.0 {
                put_pixel_style_coverage(buf, width, height, px, py, style, coverage, clip);
            }
        }
    }
}

/// Stroke a polyline with a Style.
pub fn stroke_polyline_style(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    pts: &[(f64, f64)],
    style: &Style,
    line_width: f64,
    cap: LineCap,
    clip: &Option<Vec<bool>>,
) {
    if pts.len() < 2 {
        if pts.len() == 1 {
            match cap {
                LineCap::Round => {
                    fill_disc_style(buf, width, height, pts[0].0, pts[0].1, line_width / 2.0, style, clip)
                }
                _ => fill_rect_style(
                    buf, width, height,
                    pts[0].0 - line_width / 2.0,
                    pts[0].1 - line_width / 2.0,
                    line_width,
                    line_width,
                    style, clip,
                ),
            }
        }
        return;
    }

    for i in 0..pts.len() - 1 {
        let (x0, y0) = pts[i];
        let (x1, y1) = pts[i + 1];
        draw_thick_line_style(buf, width, height, x0, y0, x1, y1, style, line_width, LineCap::Butt, clip);
    }

    let hw = line_width / 2.0;
    match cap {
        LineCap::Butt => {}
        LineCap::Round => {
            fill_disc_style(buf, width, height, pts[0].0, pts[0].1, hw, style, clip);
            let last = *pts.last().unwrap();
            fill_disc_style(buf, width, height, last.0, last.1, hw, style, clip);
        }
        LineCap::Square => {
            add_square_cap_style(buf, width, height, pts[0], pts[1], hw, style, clip, true);
            let n = pts.len();
            add_square_cap_style(buf, width, height, pts[n - 1], pts[n - 2], hw, style, clip, true);
        }
    }
}

/// Add a square cap using a Style.
fn add_square_cap_style(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    tip: (f64, f64),
    neighbour: (f64, f64),
    hw: f64,
    style: &Style,
    clip: &Option<Vec<bool>>,
    _outward: bool,
) {
    let dx = tip.0 - neighbour.0;
    let dy = tip.1 - neighbour.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        return;
    }
    let ux = dx / len;
    let uy = dy / len;
    let nx = -uy;
    let ny = ux;
    let corners = [
        (tip.0 + nx * hw, tip.1 + ny * hw),
        (tip.0 - nx * hw, tip.1 - ny * hw),
        (tip.0 - nx * hw + ux * hw, tip.1 - ny * hw + uy * hw),
        (tip.0 + nx * hw + ux * hw, tip.1 + ny * hw + uy * hw),
    ];
    fill_polygon_style(buf, width, height, &corners, style, clip);
}

/// Fill a closed sub-path using a Style.
pub fn fill_subpath_style(
    buf: &mut Vec<u8>,
    width: u32,
    height: u32,
    pts: &[(f64, f64)],
    style: &Style,
    clip: &Option<Vec<bool>>,
) {
    let mut polygon: Vec<(f64, f64)> = pts.to_vec();
    if polygon.len() >= 2 {
        let first = polygon[0];
        let last = *polygon.last().unwrap();
        if (first.0 - last.0).abs() > 1e-9 || (first.1 - last.1).abs() > 1e-9 {
            polygon.push(first);
        }
    }
    fill_polygon_style(buf, width, height, &polygon, style, clip);
}

#[inline]
fn supersample_pixel_coverage<F>(px: i64, py: i64, mut contains: F) -> f64
where
    F: FnMut(f64, f64) -> bool,
{
    let mut covered = 0usize;
    let total = SHAPE_AA_GRID * SHAPE_AA_GRID;
    let pxf = px as f64;
    let pyf = py as f64;

    for sy in 0..SHAPE_AA_GRID {
        for sx in 0..SHAPE_AA_GRID {
            let sample_x = pxf + (sx as f64 + 0.5) / SHAPE_AA_GRID as f64;
            let sample_y = pyf + (sy as f64 + 0.5) / SHAPE_AA_GRID as f64;
            if contains(sample_x, sample_y) {
                covered += 1;
            }
        }
    }

    covered as f64 / total as f64
}

#[inline]
fn is_nearly_integer(value: f64) -> bool {
    (value - value.round()).abs() <= GEOMETRY_EPSILON
}

#[inline]
fn axis_aligned_line_rect(
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    hw: f64,
    cap: LineCap,
) -> Option<(f64, f64, f64, f64, bool)> {
    if (x0 - x1).abs() <= GEOMETRY_EPSILON {
        let mut top = y0.min(y1);
        let mut bottom = y0.max(y1);
        let left = x0 - hw;
        let width = hw * 2.0;
        let round_caps = cap == LineCap::Round;
        if cap == LineCap::Square {
            top -= hw;
            bottom += hw;
        }
        return Some((left, top, width, bottom - top, round_caps));
    }

    if (y0 - y1).abs() <= GEOMETRY_EPSILON {
        let mut left = x0.min(x1);
        let mut right = x0.max(x1);
        let top = y0 - hw;
        let height = hw * 2.0;
        let round_caps = cap == LineCap::Round;
        if cap == LineCap::Square {
            left -= hw;
            right += hw;
        }
        return Some((left, top, right - left, height, round_caps));
    }

    None
}

#[inline]
fn point_in_polygon_even_odd(pts: &[(f64, f64)], x: f64, y: f64) -> bool {
    let mut inside = false;
    let n = pts.len();
    for i in 0..n {
        let (x0, y0) = pts[i];
        let (x1, y1) = pts[(i + 1) % n];
        let intersects = ((y0 > y) != (y1 > y))
            && (x < (x1 - x0) * (y - y0) / ((y1 - y0).abs().max(f64::EPSILON) * (if y1 >= y0 { 1.0 } else { -1.0 })) + x0);
        if intersects {
            inside = !inside;
        }
    }
    inside
}


