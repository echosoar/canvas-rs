// Rasterizer helper functions often need many parameters (canvas buffer,
// dimensions, coordinates, color, clip).  Allow the lint globally for this
// module rather than annotating every function individually.
#![allow(clippy::too_many_arguments, clippy::ptr_arg)]

use crate::color::Color;
use crate::image::ImageData;

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
    if let Some(mask) = clip
        && !mask[idx]
    {
        return;
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
pub fn draw_image_region(
    buf: &mut Vec<u8>,
    canvas_width: u32,
    canvas_height: u32,
    image: &ImageData,
    sx: f64, sy: f64, sw: f64, sh: f64,
    dx: f64, dy: f64, dw: f64, dh: f64,
    clip: &Option<Vec<bool>>,
) {
    if dw <= 0.0 || dh <= 0.0 || sw <= 0.0 || sh <= 0.0 {
        return;
    }
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


