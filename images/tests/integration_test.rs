use canvas::{Canvas, Color, ImageData};
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
    let url = images::to_data_url(&canvas);
    assert!(
        url.starts_with("data:image/png;base64,"),
        "URL should start with data:image/png;base64,"
    );
}

#[test]
fn to_data_url_png_header_valid() {
    let canvas = Canvas::new(2, 2);
    let url = images::to_data_url(&canvas);
    let b64 = url.strip_prefix("data:image/png;base64,").unwrap();
    // PNG signature in base64 starts with "iVBOR".
    assert!(
        b64.starts_with("iVBOR"),
        "base64 should start with PNG magic (iVBOR…)"
    );
}

// ── to_blob ──────────────────────────────────────────────────────────────────

#[test]
fn to_blob_is_valid_png() {
    let canvas = Canvas::new(4, 4);
    let bytes = images::to_blob(&canvas);
    assert_eq!(&bytes[..8], &[137, 80, 78, 71, 13, 10, 26, 10], "to_blob should return a valid PNG");
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

#[test]
fn fill_rect_is_antialiased() {
    let canvas = Canvas::new(20, 20);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_fill_style("red");
    ctx.fill_rect(3.25, 3.25, 10.5, 8.5);

    let img = canvas.get_image_data();
    let edge = img.get_pixel(3, 3);
    assert!(edge.a > 0 && edge.a < 255, "fill_rect edge should be antialiased: a={}", edge.a);
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

#[test]
fn stroke_rect_is_antialiased() {
    let canvas = Canvas::new(24, 24);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_stroke_style("blue");
    ctx.set_line_width(1.0);
    ctx.stroke_rect(4.25, 4.25, 12.5, 10.5);

    let img = canvas.get_image_data();
    let edge = img.get_pixel(8, 4);
    assert!(edge.a > 0 && edge.a < 255, "stroke_rect edge should be antialiased: a={}", edge.a);
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

#[test]
fn path_stroke_line_is_antialiased() {
    let canvas = Canvas::new(20, 20);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_stroke_style("blue");
    ctx.set_line_width(1.0);
    ctx.begin_path();
    ctx.move_to(2.5, 2.5);
    ctx.line_to(17.5, 8.5);
    ctx.stroke();

    let img = canvas.get_image_data();
    let edge = img.get_pixel(5, 3);
    assert!(edge.a > 0 && edge.a < 255, "line edge should be antialiased: a={}", edge.a);
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

#[test]
fn arc_stroke_circle_is_antialiased() {
    let canvas = Canvas::new(50, 50);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_stroke_style("blue");
    ctx.set_line_width(1.0);
    ctx.begin_path();
    ctx.arc(25.5, 25.5, 10.5, 0.0, PI * 2.0, false);
    ctx.stroke();

    let img = canvas.get_image_data();
    let edge = img.get_pixel(36, 25);
    assert!(edge.a > 0 && edge.a < 255, "arc edge should be antialiased: a={}", edge.a);
}

#[test]
fn round_rect_fill_has_rounded_corners() {
    let canvas = Canvas::new(30, 24);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_fill_style("red");
    ctx.begin_path();
    ctx.round_rect(4.0, 4.0, 20.0, 12.0, &[4.0]);
    ctx.fill();

    let img = canvas.get_image_data();
    assert_eq!(img.get_pixel(14, 10).r, 255, "rounded rect center should be filled");
    assert_eq!(img.get_pixel(4, 4).a, 0, "outer corner should remain transparent");
    assert!(img.get_pixel(14, 4).a > 0, "top edge should be filled");
}

#[test]
fn round_rect_fill_is_antialiased() {
    let canvas = Canvas::new(30, 24);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_fill_style("red");
    ctx.begin_path();
    ctx.round_rect(4.25, 4.25, 20.0, 12.0, &[4.5]);
    ctx.fill();

    let img = canvas.get_image_data();
    let edge = img.get_pixel(8, 4);
    assert!(edge.a > 0 && edge.a < 255, "round_rect edge should be antialiased: a={}", edge.a);
}

#[test]
fn round_rect_stroke_draws_outline_only() {
    let canvas = Canvas::new(30, 24);
    let mut ctx = canvas.get_context("2d").unwrap();
    ctx.set_stroke_style("blue");
    ctx.set_line_width(1.0);
    ctx.begin_path();
    ctx.round_rect(4.0, 4.0, 20.0, 12.0, &[3.0, 5.0, 3.0, 5.0]);
    ctx.stroke();

    let img = canvas.get_image_data();
    assert!(
        img.get_pixel(14, 3).a > 0 || img.get_pixel(14, 4).a > 0,
        "top outline should be stroked"
    );
    assert_eq!(img.get_pixel(14, 10).a, 0, "interior should remain transparent");
    assert_eq!(img.get_pixel(3, 3).a, 0, "outer corner should remain transparent");
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
    use canvas::color::parse_color;
    assert_eq!(parse_color("#f00"), Some(Color::rgb(255, 0, 0)));
    assert_eq!(parse_color("#0f0"), Some(Color::rgb(0, 255, 0)));
    assert_eq!(parse_color("#00f"), Some(Color::rgb(0, 0, 255)));
}

#[test]
fn color_hex_long() {
    use canvas::color::parse_color;
    assert_eq!(parse_color("#ff0000"), Some(Color::rgb(255, 0, 0)));
}

#[test]
fn color_named_all_common() {
    use canvas::color::parse_color;
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
    use images::encode_png;
    let pixels = vec![255u8, 0, 0, 255, 0, 255, 0, 255]; // 2×1 (red, green)
    let png = encode_png(2, 1, &pixels);
    // Must start with PNG signature.
    assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    // Must contain the IEND tag.
    let has_iend = png.windows(4).any(|w| w == b"IEND");
    assert!(has_iend, "PNG should contain IEND chunk");
}

#[test]
fn base64_round_trip() {
    use images::base64_encode;
    let original = b"Hello, World!";
    let encoded = base64_encode(original);
    assert_eq!(encoded, "SGVsbG8sIFdvcmxkIQ==");
}

// ── from_png (PNG decoding) ───────────────────────────────────────────────────

#[test]
fn from_png_roundtrip() {
    // Encode a canvas, then decode it back and check dimensions.
    let canvas = Canvas::new(8, 8);
    let png_bytes = images::to_blob(&canvas);
    let img = images::from_png(&png_bytes).expect("round-trip PNG decode should succeed");
    assert_eq!(img.width, 8);
    assert_eq!(img.height, 8);
    assert_eq!(img.data.len(), 8 * 8 * 4);
}

// ── fill_rect + draw image from file ─────────────────────────────────────────

#[test]
fn fill_rect_green_then_draw_image() {
    let canvas = Canvas::new(300, 300);
    let mut ctx = canvas.get_context("2d").unwrap();

    // Draw green rectangle (CSS "green" = rgb(0, 128, 0)).
    ctx.set_fill_style("green");
    ctx.fill_rect(10.0, 10.0, 150.0, 100.0);

    // Verify the rectangle is painted before the image is drawn on top.
    let snapshot = canvas.get_image_data();
    let green_px = snapshot.get_pixel(50, 50);
    assert_eq!(green_px.r, 0,   "green rect r should be 0");
    assert_eq!(green_px.g, 128, "green rect g should be 128");
    assert_eq!(green_px.b, 0,   "green rect b should be 0");
    assert_eq!(green_px.a, 255, "green rect should be fully opaque");

    // Pixel outside the rect should still be transparent at this point.
    let outside_snap = snapshot.get_pixel(5, 5);
    assert_eq!(outside_snap.a, 0, "pixel outside rect should be transparent before image draw");

    // Load tests/image_220x200.png using images::from_png and draw it on top.
    let png_bytes = std::fs::read("tests/image_220x200.png").expect("could not read PNG file");
    let img_data = images::from_png(&png_bytes).expect("could not decode PNG");
    assert_eq!(img_data.width,  220, "loaded image width should be 220");
    assert_eq!(img_data.height, 200, "loaded image height should be 200");
    ctx.draw_image(&img_data, 0.0, 0.0);

    // After drawing, a pixel outside both the image (220×200) and the green
    // rect should still be transparent.
    let result = canvas.get_image_data();
    let far_px = result.get_pixel(260, 260);
    assert_eq!(far_px.a, 0, "pixel outside image and rect should remain transparent");

    // The canvas should export to a valid PNG data URL without panicking.
    let url = images::to_data_url(&canvas);
    assert!(url.starts_with("data:image/png;base64,"), "export should produce a valid PNG data URL");
}

// ── font test ─────────────────────────────────────────────────────────────────

#[test]
fn font_render_three_lines_with_nested_rects() {
    let canvas = Canvas::new(1080, 200);
    let mut ctx = canvas.get_context("2d").unwrap();

    let png_bytes = std::fs::read("tests/image_220x200.png").expect("could not read PNG file");
    let img_data = images::from_png(&png_bytes).expect("could not decode PNG");
    ctx.draw_image_with_size(&img_data, 860.0, 0.0, 100.0, 150.0);
    ctx.draw_image_source(&img_data,50.0, 50.0, 100.0, 100.0, 260.0, 0.0, 100.0, 150.0);

    // Draw text inside the rects (30px from edge)
    ctx.set_fill_style("black");
    ctx.set_font("32px common");

    // Line 1: Chinese
    ctx.fill_text("让天下没有难生成的图。", 20.0, 50.0);

    // Line 2: English
    ctx.set_fill_style("red");
    ctx.fill_text("Make it so that no graph is difficult to generate.", 20.0, 90.0);

    // Line 3: Date
    ctx.set_fill_style("blue");
    ctx.set_font("16px common");
    ctx.fill_text("--- 2026.03.15", 20.0, 130.0);

    // Generate base64 PNG data URL
    let url = images::to_data_url(&canvas);
    assert!(url.starts_with("data:image/png;base64,"), "font test should produce a valid PNG data URL");

    // Verify the PNG is valid by decoding from blob
    let png_bytes = images::to_blob(&canvas);
    let decoded = images::from_png(&png_bytes).expect("decoded PNG should be valid");
    assert_eq!(decoded.width, 1080);
    assert_eq!(decoded.height, 200);
    println!("Generated PNG data URL:\n{}", url);
}

#[test]
fn fill_text_with_newlines_renders_as_multiple_lines() {
    let multiline_canvas = Canvas::new(120, 80);
    let mut multiline_ctx = multiline_canvas.get_context("2d").unwrap();
    multiline_ctx.set_fill_style("black");
    multiline_ctx.set_font("16px common");
    multiline_ctx.fill_text("A\nB", 10.0, 10.0);

    let manual_canvas = Canvas::new(120, 80);
    let mut manual_ctx = manual_canvas.get_context("2d").unwrap();
    manual_ctx.set_fill_style("black");
    manual_ctx.set_font("16px common");
    manual_ctx.fill_text("A", 10.0, 10.0);
    manual_ctx.fill_text("B", 10.0, 26.0);

    assert_eq!(multiline_canvas.get_image_data().data, manual_canvas.get_image_data().data);
}

// ── SVG tests ─────────────────────────────────────────────────────────────────

#[test]
fn svg_render_rect_fills_expected_pixels() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
        <rect x="0" y="0" width="100" height="100" fill="red"/>
    </svg>"#;
    let img = canvas::render_svg(svg.as_bytes(), 100, 100).expect("should render SVG");
    assert_eq!(img.width, 100);
    assert_eq!(img.height, 100);
    let px = img.get_pixel(50, 50);
    assert!(px.r > 200 && px.g < 50 && px.b < 50, "center pixel should be red, got {:?}", px);
}

#[test]
fn svg_render_circle_center_is_filled() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
        <circle cx="50" cy="50" r="40" fill="blue"/>
    </svg>"#;
    let img = canvas::render_svg(svg.as_bytes(), 100, 100).expect("should render SVG");
    let center = img.get_pixel(50, 50);
    assert!(center.b > 200 && center.r < 50, "center should be blue, got {:?}", center);
    // Corner should be transparent (outside circle)
    let corner = img.get_pixel(0, 0);
    assert_eq!(corner.a, 0, "corner should be transparent, got {:?}", corner);
}

#[test]
fn svg_render_path_triangle() {
    // Isoceles triangle: top-center, bottom-left, bottom-right
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
        <path d="M 50 10 L 90 90 L 10 90 Z" fill="green"/>
    </svg>"#;
    let img = canvas::render_svg(svg.as_bytes(), 100, 100).expect("should render SVG");
    // Center of the triangle should be green
    let center = img.get_pixel(50, 60);
    assert!(center.g > 100, "triangle interior should be green, got {:?}", center);
}

#[test]
fn svg_render_viewbox_scales_correctly() {
    // SVG with viewBox 0 0 50 50 rendered at 100x100: everything should be doubled
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 50 50">
        <rect x="0" y="0" width="50" height="50" fill="green"/>
    </svg>"#;
    let img = canvas::render_svg(svg.as_bytes(), 100, 100).expect("should render SVG");
    assert_eq!(img.width, 100);
    assert_eq!(img.height, 100);
    let px = img.get_pixel(50, 50);
    assert!(px.g > 100, "pixel should be green after viewBox scaling, got {:?}", px);
}

#[test]
fn svg_render_group_inherits_style() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
        <g fill="red">
            <rect x="10" y="10" width="30" height="30"/>
        </g>
    </svg>"#;
    let img = canvas::render_svg(svg.as_bytes(), 100, 100).expect("should render SVG");
    let px = img.get_pixel(25, 25);
    assert!(px.r > 200 && px.g < 50, "rect inside group should inherit red fill, got {:?}", px);
}

#[test]
fn svg_render_stroke_only_element() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
        <line x1="0" y1="50" x2="100" y2="50" stroke="red" stroke-width="4"/>
    </svg>"#;
    let img = canvas::render_svg(svg.as_bytes(), 100, 100).expect("should render SVG");
    let on_line = img.get_pixel(50, 50);
    assert!(on_line.r > 200, "pixel on line should be red, got {:?}", on_line);
}

#[test]
fn svg_render_ellipse() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="60">
        <ellipse cx="50" cy="30" rx="40" ry="20" fill="purple"/>
    </svg>"#;
    let img = canvas::render_svg(svg.as_bytes(), 100, 60).expect("should render SVG");
    let center = img.get_pixel(50, 30);
    assert!(center.r > 100 && center.b > 100, "center should be purple-ish, got {:?}", center);
}

#[test]
fn svg_render_polygon() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
        <polygon points="50,10 90,90 10,90" fill="orange"/>
    </svg>"#;
    let img = canvas::render_svg(svg.as_bytes(), 100, 100).expect("should render SVG");
    // Center of the triangle should have some orange-ish color (high R, medium G, low B)
    let center = img.get_pixel(50, 60);
    assert!(center.r > 200 && center.b < 50, "polygon center should be orange, got {:?}", center);
}

#[test]
fn svg_draw_svg_onto_context() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
        <rect x="0" y="0" width="50" height="50" fill="cyan"/>
    </svg>"#;
    let canvas = Canvas::new(100, 100);
    let mut ctx = canvas.get_context("2d").unwrap();
    let result = canvas::draw_svg(&mut ctx, svg.as_bytes(), 25.0, 25.0, 50, 50);
    assert!(result, "draw_svg should succeed");
    let img = canvas.get_image_data();
    // The drawn SVG starts at (25, 25) so pixel at (50, 50) should be cyan
    let px = img.get_pixel(50, 50);
    assert!(px.g > 200 && px.b > 200, "pixel should be cyan, got {:?}", px);
    // Pixel at (10, 10) is outside the drawn SVG and should be transparent
    let outside = img.get_pixel(10, 10);
    assert_eq!(outside.a, 0, "pixel outside SVG should be transparent, got {:?}", outside);
}

#[test]
fn svg_render_opacity() {
    // A fully-opaque white background plus a semi-transparent red rect on top
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
        <rect x="0" y="0" width="50" height="50" fill="white"/>
        <rect x="0" y="0" width="50" height="50" fill="red" opacity="0.5"/>
    </svg>"#;
    let img = canvas::render_svg(svg.as_bytes(), 50, 50).expect("should render SVG");
    let px = img.get_pixel(25, 25);
    // Should be a blend of red and white (pink-ish)
    assert!(px.r > 200 && px.g > 100, "blended color should be pink-ish, got {:?}", px);
}

