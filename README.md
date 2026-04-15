# canvas-rs
pure rust implemented drawer library( api like canvas), and no dependencies, super lightweight, can be used in any rust project, including wasm and embedded.

![](./test/cover.png)


## Usage

```rust
use canvas_rs::Canvas;
use canvas_rs::images;

func main() {
    let canvas = Canvas::new(200, 200);
    let canvas = Canvas::new(1080, 200);
    let mut ctx = canvas.get_context("2d").unwrap();

    let png_bytes = std::fs::read("tests/image_220x200.png").expect("could not read PNG file");
    let img_data = images::from_png(&png_bytes).expect("could not decode PNG");
    ctx.draw_image(&img_data, 860.0, 0.0);

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
}


```