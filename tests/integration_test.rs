use canvas_rs::{Canvas, Color, ImageData};
use std::f64::consts::PI;

// ── Canvas creation ──────────────────────────────────────────────────────────

#[test]
fn canvas_dimensions() {
    let canvas = Canvas::new(320, 240);
    assert_eq!(canvas.width(), 320);
    assert_eq!(canvas.height(), 240);
}

#[test]
fn canvas_starts_transparent() {
    let canvas = Canvas::new(4, 4);
    let img = canvas.get_image_data();
    // All pixels should be fully transparent.
    for &b in &img.data {
        assert_eq!(b, 0, "canvas should be transparent on creation");
    }
}

#[test]
fn get_context_2d_returns_some() {
    let canvas = Canvas::new(10, 10);
    assert!(canvas.get_context("2d").is_some());
}

#[test]
fn get_context_unknown_returns_none() {
    let canvas = Canvas::new(10, 10);
    assert!(canvas.get_context("webgl").is_none());
}

// ── to_data_url ──────────────────────────────────────────────────────────────

#[test]
fn to_data_url_starts_with_correct_prefix() {
    let canvas = Canvas::new(8, 8);
    let url = canvas.to_data_url();
    assert!(
        url.starts_with("data:image/png;base64,"),
        "URL should start with data:image/png;base64,"
    );
}

#[test]
fn to_data_url_with_options_respects_type() {
    let canvas = Canvas::new(4, 4);
    let url = canvas.to_data_url_with_options("image/png", 1.0);
    assert!(url.starts_with("data:image/png;base64,"));
}

#[test]
fn to_data_url_png_header_valid() {
    let canvas = Canvas::new(2, 2);
    let url = canvas.to_data_url();
    let b64 = url.strip_prefix("data:image/png;base64,").unwrap();
    // Decode first few bytes manually to check the PNG signature.
    // PNG signature in base64 starts with "iVBOR".
    assert!(
        b64.starts_with("iVBOR"),
        "base64 should start with PNG magic (iVBOR…)"
    );
}

// ── fillStyle / strokeStyle properties ───────────────────────────────────────

#[test]
fn fill_style_defaults_to_black() {
    let canvas = Canvas::new(10, 10);
    let ctx = canvas.get_context("2d").unwrap();
    let fs = ctx.fill_style();
    // Default is black = rgba(0,0,0,255/255) = rgba(0,0,0,1)
    assert!(fs.starts_with("rgba(0,0,0,"));
}

#[test]
fn set_fill_style_named_color() {
    let canvas = Canvas::new(10, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_fill_style("red");
    let fs = ctx.fill_style();
    assert!(fs.starts_with("rgba(255,0,0,"));
}

#[test]
fn set_fill_style_hex() {
    let canvas = Canvas::new(10, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_fill_style("#00ff00");
    let fs = ctx.fill_style();
    assert!(fs.starts_with("rgba(0,255,0,"));
}

#[test]
fn set_fill_style_rgb() {
    let canvas = Canvas::new(10, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_fill_style("rgb(10,20,30)");
    let fs = ctx.fill_style();
    assert!(fs.starts_with("rgba(10,20,30,"));
}

#[test]
fn set_fill_style_rgba() {
    let canvas = Canvas::new(10, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_fill_style("rgba(10,20,30,0.5)");
    let c = ctx.fill_style();
    // Alpha should be ≈ 0.5 * 255 ≈ 128 (rounded).
    assert!(c.contains("rgba(10,20,30,"));
}

#[test]
fn invalid_fill_style_keeps_previous() {
    let canvas = Canvas::new(10, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_fill_style("red");
    ctx.set_fill_style("not-a-color");
    let fs = ctx.fill_style();
    assert!(fs.starts_with("rgba(255,0,0,"));
}

#[test]
fn set_stroke_style_works() {
    let canvas = Canvas::new(10, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_stroke_style("blue");
    let ss = ctx.stroke_style();
    assert!(ss.starts_with("rgba(0,0,255,"));
}

// ── lineWidth and lineCap ─────────────────────────────────────────────────────

#[test]
fn line_width_default_is_one() {
    let canvas = Canvas::new(10, 10);
    let ctx = canvas.get_context("2d").unwrap();
    assert_eq!(ctx.line_width(), 1.0);
}

#[test]
fn set_line_width_works() {
    let canvas = Canvas::new(10, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_line_width(3.0);
    assert_eq!(ctx.line_width(), 3.0);
}

#[test]
fn set_line_width_ignores_zero() {
    let canvas = Canvas::new(10, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_line_width(5.0);
    ctx.set_line_width(0.0);
    assert_eq!(ctx.line_width(), 5.0);
}

#[test]
fn line_cap_default_is_butt() {
    let canvas = Canvas::new(10, 10);
    let ctx = canvas.get_context("2d").unwrap();
    assert_eq!(ctx.line_cap(), "butt");
}

#[test]
fn set_line_cap_round() {
    let canvas = Canvas::new(10, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_line_cap("round");
    assert_eq!(ctx.line_cap(), "round");
}

#[test]
fn set_line_cap_square() {
    let canvas = Canvas::new(10, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_line_cap("square");
    assert_eq!(ctx.line_cap(), "square");
}

// ── fill_rect ────────────────────────────────────────────────────────────────

#[test]
fn fill_rect_fills_pixels() {
    let canvas = Canvas::new(10, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_fill_style("red");
    ctx.fill_rect(2.0, 2.0, 4.0, 4.0);
    let img = canvas.get_image_data();

    // Inside the rectangle: red.
    let px = img.get_pixel(3, 3);
    assert_eq!(px.r, 255, "inside fill_rect should be red (r=255)");
    assert_eq!(px.g, 0, "inside fill_rect should be red (g=0)");
    assert_eq!(px.b, 0, "inside fill_rect should be red (b=0)");

    // Outside: still transparent.
    let px2 = img.get_pixel(0, 0);
    assert_eq!(px2.a, 0, "outside fill_rect should be transparent");
}

#[test]
fn fill_rect_full_canvas() {
    let canvas = Canvas::new(8, 8);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_fill_style("white");
    ctx.fill_rect(0.0, 0.0, 8.0, 8.0);
    let img = canvas.get_image_data();
    for chunk in img.data.chunks(4) {
        assert_eq!(chunk, [255, 255, 255, 255]);
    }
}

// ── clear_rect ───────────────────────────────────────────────────────────────

#[test]
fn clear_rect_erases_pixels() {
    let canvas = Canvas::new(10, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_fill_style("blue");
    ctx.fill_rect(0.0, 0.0, 10.0, 10.0);
    ctx.clear_rect(2.0, 2.0, 4.0, 4.0);
    let img = canvas.get_image_data();

    // Cleared area: transparent.
    let px = img.get_pixel(3, 3);
    assert_eq!(px.a, 0, "cleared pixel should be transparent");

    // Uncleared area: still blue.
    let px2 = img.get_pixel(0, 0);
    assert_eq!(px2.b, 255, "uncleared pixel should be blue");
}

// ── stroke_rect ──────────────────────────────────────────────────────────────

#[test]
fn stroke_rect_draws_border() {
    let canvas = Canvas::new(20, 20);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_stroke_style("black");
    ctx.set_line_width(1.0);
    ctx.stroke_rect(5.0, 5.0, 10.0, 10.0);
    let img = canvas.get_image_data();

    // Corner pixels of the border should be black.
    let px = img.get_pixel(5, 5);
    assert!(px.a > 0, "border corner should be drawn");
    assert_eq!(px.r, 0);

    // Interior pixel should be transparent.
    let interior = img.get_pixel(10, 10);
    assert_eq!(interior.a, 0, "interior of stroke_rect should be transparent");
}

// ── path fill & stroke ───────────────────────────────────────────────────────

#[test]
fn path_fill_triangle() {
    let canvas = Canvas::new(20, 20);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_fill_style("green");
    ctx.begin_path();
    ctx.move_to(10.0, 1.0);
    ctx.line_to(19.0, 19.0);
    ctx.line_to(1.0, 19.0);
    ctx.close_path();
    ctx.fill();
    let img = canvas.get_image_data();

    // Centre of the triangle should be filled.
    let centre = img.get_pixel(10, 14);
    assert!(centre.a > 0, "centre of triangle should be filled");
}

#[test]
fn path_stroke_line() {
    let canvas = Canvas::new(30, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_stroke_style("blue");
    ctx.set_line_width(2.0);
    ctx.begin_path();
    ctx.move_to(0.0, 5.0);
    ctx.line_to(29.0, 5.0);
    ctx.stroke();
    let img = canvas.get_image_data();

    // Pixel along the centre-line should be blue.
    let px = img.get_pixel(15, 5);
    assert_eq!(px.b, 255, "stroked line should be blue");
    assert!(px.a > 0, "stroked line pixel should not be transparent");
}

// ── arc ──────────────────────────────────────────────────────────────────────

#[test]
fn arc_full_circle_fills() {
    let canvas = Canvas::new(50, 50);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_fill_style("red");
    ctx.begin_path();
    ctx.arc(25.0, 25.0, 20.0, 0.0, PI * 2.0, false);
    ctx.fill();
    let img = canvas.get_image_data();

    // Centre should be red.
    let px = img.get_pixel(25, 25);
    assert_eq!(px.r, 255, "circle centre should be red");
    assert_eq!(px.a, 255);

    // Far corner should be transparent.
    let corner = img.get_pixel(0, 0);
    assert_eq!(corner.a, 0, "outside circle should be transparent");
}

#[test]
fn arc_stroke_circle() {
    let canvas = Canvas::new(50, 50);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_stroke_style("blue");
    ctx.set_line_width(2.0);
    ctx.begin_path();
    ctx.arc(25.0, 25.0, 20.0, 0.0, PI * 2.0, false);
    ctx.stroke();
    let img = canvas.get_image_data();

    // A point on the circumference should be blue.
    let on_circle = img.get_pixel(45, 25); // rightmost point ≈ (25+20, 25)
    assert!(on_circle.a > 0 || img.get_pixel(44, 25).a > 0,
        "point on circle circumference should be drawn");
}

// ── drawImage ────────────────────────────────────────────────────────────────

#[test]
fn draw_image_copies_pixels() {
    // Create a small red image.
    let src_data: Vec<u8> = (0..4 * 4 * 4).map(|i| match i % 4 {
        0 => 255, // R
        3 => 255, // A
        _ => 0,
    }).collect();
    let img = ImageData::from_rgba(4, 4, src_data);

    let canvas = Canvas::new(10, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.draw_image(&img, 3.0, 3.0);

    let result = canvas.get_image_data();
    let px = result.get_pixel(3, 3);
    assert_eq!(px.r, 255, "draw_image should copy red pixels");
    assert_eq!(px.a, 255);
}

#[test]
fn draw_image_with_size_scales() {
    // 1×1 blue pixel.
    let img = ImageData::from_rgba(1, 1, vec![0, 0, 255, 255]);

    let canvas = Canvas::new(10, 10);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.draw_image_with_size(&img, 0.0, 0.0, 5.0, 5.0);

    let result = canvas.get_image_data();
    // The 5×5 area should all be blue.
    for py in 0..5u32 {
        for px in 0..5u32 {
            let c = result.get_pixel(px, py);
            assert_eq!(c.b, 255, "scaled image should fill destination blue");
        }
    }
}

// ── clip ─────────────────────────────────────────────────────────────────────

#[test]
fn clip_restricts_drawing() {
    let canvas = Canvas::new(20, 20);
    let mut ctx = canvas.get_context("2d").unwrap();

    // Clip to the left half.
    ctx.begin_path();
    ctx.move_to(0.0, 0.0);
    ctx.line_to(10.0, 0.0);
    ctx.line_to(10.0, 20.0);
    ctx.line_to(0.0, 20.0);
    ctx.close_path();
    ctx.clip();

    // Fill the whole canvas red.
    ctx.set_fill_style("red");
    ctx.fill_rect(0.0, 0.0, 20.0, 20.0);

    let img = canvas.get_image_data();

    // Left half: red.
    let left = img.get_pixel(4, 10);
    assert_eq!(left.r, 255, "left half should be red (clipped)");

    // Right half: transparent (clipped out).
    let right = img.get_pixel(15, 10);
    assert_eq!(right.a, 0, "right half should be transparent (clipped out)");
}

// ── draw_canvas ───────────────────────────────────────────────────────────────

#[test]
fn draw_canvas_copies_another_canvas() {
    let src = Canvas::new(5, 5);
    {
        let mut ctx = src.get_context("2d").unwrap();
        ctx.set_fill_style("yellow");
        ctx.fill_rect(0.0, 0.0, 5.0, 5.0);
    }

    let dst = Canvas::new(20, 20);
    {
        let mut ctx = dst.get_context("2d").unwrap();
        ctx.draw_canvas(&src, 10.0, 10.0);
    }

    let img = dst.get_image_data();
    let px = img.get_pixel(12, 12);
    assert_eq!(px.r, 255, "copied yellow pixel should have r=255");
    assert_eq!(px.g, 255, "copied yellow pixel should have g=255");
    assert_eq!(px.b, 0,   "copied yellow pixel should have b=0");
}

// ── alpha blending ────────────────────────────────────────────────────────────

#[test]
fn alpha_blending_on_fill_rect() {
    let canvas = Canvas::new(4, 4);
    let mut ctx = canvas.get_context("2d").unwrap();

    // Fill white.
    ctx.set_fill_style("white");
    ctx.fill_rect(0.0, 0.0, 4.0, 4.0);

    // Overlay 50% transparent red.
    ctx.set_fill_style("rgba(255,0,0,0.5)");
    ctx.fill_rect(0.0, 0.0, 4.0, 4.0);

    let img = canvas.get_image_data();
    let px = img.get_pixel(1, 1);
    // Result should be pinkish (red > 128, green > 0).
    assert!(px.r > 200, "blended pixel should be reddish: r={}", px.r);
    assert!(px.g > 100, "blended pixel should keep some green: g={}", px.g);
    assert_eq!(px.a, 255, "blended pixel should be fully opaque");
}

// ── Color parsing edge cases ──────────────────────────────────────────────────

#[test]
fn color_hex_short() {
    use canvas_rs::color::parse_color;
    assert_eq!(parse_color("#f00"), Some(Color::rgb(255, 0, 0)));
    assert_eq!(parse_color("#0f0"), Some(Color::rgb(0, 255, 0)));
    assert_eq!(parse_color("#00f"), Some(Color::rgb(0, 0, 255)));
}

#[test]
fn color_hex_long() {
    use canvas_rs::color::parse_color;
    assert_eq!(parse_color("#ff0000"), Some(Color::rgb(255, 0, 0)));
}

#[test]
fn color_named_all_common() {
    use canvas_rs::color::parse_color;
    let colors = ["red","green","blue","white","black","transparent",
                  "yellow","cyan","magenta","orange","purple","pink",
                  "gray","silver","lime","navy","teal","coral","gold"];
    for &name in &colors {
        assert!(parse_color(name).is_some(), "named color '{}' should parse", name);
    }
}

// ── PNG encoding sanity ───────────────────────────────────────────────────────

#[test]
fn png_encode_nonempty() {
    use canvas_rs::png::encode_png;
    let pixels = vec![255u8, 0, 0, 255, 0, 255, 0, 255]; // 2×1 (red, green)
    let png = encode_png(2, 1, &pixels);
    // Must start with PNG signature.
    assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    // Must contain the IEND tag (4 bytes length=0, then "IEND", then 4-byte CRC).
    // Find "IEND" in the file.
    let has_iend = png.windows(4).any(|w| w == b"IEND");
    assert!(has_iend, "PNG should contain IEND chunk");
}

#[test]
fn base64_round_trip() {
    use canvas_rs::png::base64_encode;
    let original = b"Hello, World!";
    let encoded = base64_encode(original);
    assert_eq!(encoded, "SGVsbG8sIFdvcmxkIQ==");
}
