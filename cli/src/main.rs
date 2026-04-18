use canvas::{Canvas, LinearGradient, RadialGradient};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn print_usage() {
    println!("canvas-cli - A command line tool for canvas drawing");
    println!();
    println!("Usage:");
    println!("  canvas-cli --input=<input.txt> --output=<output.png>");
    println!("  canvas-cli --input=\"canvas 1080 200; [operation] [args...]\" --output=<output.png>");
    println!("  canvas-cli --input=<input.txt> --output-data-url");
    println!();
    println!("Options:");
    println!("  --input=<path_or_commands>  - Input file path or inline commands");
    println!("  --output=<path>            - Output PNG file path");
    println!("  --output-data-url          - Output as data URL string instead of file");
    println!();
    println!("Input file format:");
    println!("  1. canvas <width> <height>");
    println!("  2. <operation> <args...>");
    println!();
    println!("Operations:");
    println!("  draw_image <path> <dx> <dy>                                       - Draw an image at (dx, dy) at natural size");
    println!("  draw_image <path> <dx> <dy> <dw> <dh>                            - Draw an image scaled to (dw, dh)");
    println!("  draw_image <path> <sx> <sy> <sw> <sh> <dx> <dy> <dw> <dh>       - Draw a sub-region of an image scaled to (dw, dh)");
    println!("  set_fill_style <color>            - Set fill style (e.g., red, #ff0000, rgb(255,0,0))");
    println!("  set_stroke_style <color>          - Set stroke style");
    println!("  set_font <size>px <family>        - Set font (e.g., 32px common)");
    println!("  set_text_antialias_grid <n>        - Set text AA grid size (1-8, e.g., 8)");
    println!("  set_text_align <align>            - Set text alignment (start, end, left, right, center)");
    println!("  set_line_width <width>            - Set line width");
    println!("  fill_rect <x> <y> <w> <h>         - Fill a rectangle");
    println!("  stroke_rect <x> <y> <w> <h>       - Stroke a rectangle");
    println!("  roundRect|round_rect <x> <y> <w> <h> <radii...> - Add a rounded rectangle to the current path");
    println!("  fill_text \"<text>\" <x> <y>       - Fill text at position (x, y)");
    println!("  begin_path                        - Begin a new path");
    println!("  move_to <x> <y>                   - Move to position");
    println!("  line_to <x> <y>                   - Draw line to position");
    println!("  arc <x> <y> <r> <start> <end>     - Draw arc");
    println!("  close_path                        - Close the current path");
    println!("  fill                              - Fill the current path");
    println!("  stroke                            - Stroke the current path");
    println!("  save                              - Save current state");
    println!("  restore                           - Restore saved state");
    println!();
    println!("Gradients:");
    println!("  create_linear_gradient <id> <x0> <y0> <x1> <y1>  - Create linear gradient");
    println!("  create_radial_gradient <id> <x0> <y0> <r0> <x1> <y1> <r1> - Create radial gradient");
    println!("  add_color_stop <gradient_id> <offset> <color>    - Add color stop to gradient");
    println!("  set_fill_gradient <gradient_id>                  - Set fill style to gradient");
    println!("  set_stroke_gradient <gradient_id>                - Set stroke style to gradient");
}

fn parse_args(args: &[String]) -> (Option<String>, Option<String>, bool) {
    let mut input: Option<String> = None;
    let mut output: Option<String> = None;
    let mut output_data_url = false;

    for arg in args {
        if arg == "--output-data-url" {
            output_data_url = true;
        } else if arg.starts_with("--input=") {
            input = Some(arg[8..].to_string());
        } else if arg.starts_with("--output=") {
            output = Some(arg[9..].to_string());
        }
    }

    (input, output, output_data_url)
}

fn parse_float(s: &str) -> f64 {
    s.parse().unwrap_or(0.0)
}

fn parse_u32(s: &str) -> u32 {
    s.parse().unwrap_or(0)
}

fn parse_quoted_string(s: &str) -> (String, usize) {
    if s.is_empty() || !s.starts_with('"') {
        // No quote, return first word
        let end = s.find(' ').unwrap_or(s.len());
        return (s[..end].to_string(), end);
    }

    let mut text = String::new();
    let mut escaped = false;

    for (idx, ch) in s[1..].char_indices() {
        if escaped {
            let unescaped = match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '\\' => '\\',
                '"' => '"',
                other => other,
            };
            text.push(unescaped);
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => return (text, idx + 2),
            _ => text.push(ch),
        }
    }

    if escaped {
        text.push('\\');
    }

    (text, s.len())
}

fn split_next_token(s: &str) -> Option<(&str, &str)> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let token_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let token = &trimmed[..token_end];
    let rest = trimmed[token_end..].trim();
    Some((token, rest))
}

fn is_ignored_command_line(s: &str) -> bool {
    let trimmed = s.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

fn first_meaningful_command_index(commands: &[String]) -> Option<usize> {
    commands
        .iter()
        .position(|cmd| !is_ignored_command_line(cmd))
}

fn parse_number_list(s: &str) -> Vec<f64> {
    s.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(parse_float)
        .collect()
}

fn parse_round_rect_args(s: &str) -> Option<(f64, f64, f64, f64, Vec<f64>)> {
    let (x, rest) = split_next_token(s)?;
    let (y, rest) = split_next_token(rest)?;
    let (width, rest) = split_next_token(rest)?;
    let (height, rest) = split_next_token(rest)?;
    let radii = parse_number_list(rest);
    if radii.is_empty() {
        return None;
    }

    Some((
        parse_float(x),
        parse_float(y),
        parse_float(width),
        parse_float(height),
        radii,
    ))
}

/// Stores either a linear or radial gradient
enum Gradient {
    Linear(LinearGradient),
    Radial(RadialGradient),
}

fn execute_commands(ctx: &mut canvas::Context2D, commands: &[String], base_path: &Path) {
    // Store gradients by ID
    let mut gradients: HashMap<String, Gradient> = HashMap::new();

    for cmd in commands {
        let cmd = cmd.trim();
        if is_ignored_command_line(cmd) {
            continue;
        }

        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let op = parts[0];

        match op {
            "draw_image" => {
                // Supported forms:
                //   draw_image <path> <dx> <dy>
                //   draw_image <path> <dx> <dy> <dw> <dh>
                //   draw_image <path> <sx> <sy> <sw> <sh> <dx> <dy> <dw> <dh>
                if parts.len() >= 4 {
                    let path = parts[1];

                    // Resolve path relative to input file
                    let image_path = if Path::new(path).is_absolute() {
                        path.to_string()
                    } else {
                        base_path.join(path).to_string_lossy().to_string()
                    };

                    if let Ok(png_bytes) = fs::read(&image_path) {
                        if let Ok(img_data) = images::from_png(&png_bytes) {
                            if parts.len() >= 10 {
                                // 9-arg: sx sy sw sh dx dy dw dh
                                let sx = parse_float(parts[2]);
                                let sy = parse_float(parts[3]);
                                let sw = parse_float(parts[4]);
                                let sh = parse_float(parts[5]);
                                let dx = parse_float(parts[6]);
                                let dy = parse_float(parts[7]);
                                let dw = parse_float(parts[8]);
                                let dh = parse_float(parts[9]);
                                ctx.draw_image_source(&img_data, sx, sy, sw, sh, dx, dy, dw, dh);
                            } else if parts.len() >= 6 {
                                // 5-arg: dx dy dw dh
                                let dx = parse_float(parts[2]);
                                let dy = parse_float(parts[3]);
                                let dw = parse_float(parts[4]);
                                let dh = parse_float(parts[5]);
                                ctx.draw_image_with_size(&img_data, dx, dy, dw, dh);
                            } else {
                                // 3-arg: dx dy
                                let x = parse_float(parts[2]);
                                let y = parse_float(parts[3]);
                                ctx.draw_image(&img_data, x, y);
                            }
                        } else {
                            eprintln!("Warning: Failed to decode image: {}", image_path);
                        }
                    } else {
                        eprintln!("Warning: Failed to read image: {}", image_path);
                    }
                }
            }
            "set_fill_style" => {
                let style = cmd[op.len()..].trim();
                if !style.is_empty() {
                    ctx.set_fill_style(style);
                }
            }
            "set_stroke_style" => {
                let style = cmd[op.len()..].trim();
                if !style.is_empty() {
                    ctx.set_stroke_style(style);
                }
            }
            "set_font" => {
                if parts.len() >= 3 {
                    // Reconstruct font string like "32px common"
                    let font_str = parts[1..].join(" ");
                    ctx.set_font(&font_str);
                }
            }
            "set_text_antialias_grid" => {
                if parts.len() >= 2 {
                    let grid = parse_u32(parts[1]);
                    ctx.set_text_antialias_grid(grid);
                }
            }
            "set_text_align" => {
                if parts.len() >= 2 {
                    ctx.set_text_align(parts[1]);
                }
            }
            "set_line_width" => {
                if parts.len() >= 2 {
                    ctx.set_line_width(parse_float(parts[1]));
                }
            }
            "fill_rect" => {
                if parts.len() >= 5 {
                    let x = parse_float(parts[1]);
                    let y = parse_float(parts[2]);
                    let w = parse_float(parts[3]);
                    let h = parse_float(parts[4]);
                    ctx.fill_rect(x, y, w, h);
                }
            }
            "stroke_rect" => {
                if parts.len() >= 5 {
                    let x = parse_float(parts[1]);
                    let y = parse_float(parts[2]);
                    let w = parse_float(parts[3]);
                    let h = parse_float(parts[4]);
                    ctx.stroke_rect(x, y, w, h);
                }
            }
            "round_rect" | "roundRect" => {
                if let Some((x, y, width, height, radii)) = parse_round_rect_args(cmd[op.len()..].trim()) {
                    ctx.round_rect(x, y, width, height, &radii);
                }
            }
            "fill_text" => {
                // Re-parse the full command to properly handle quoted text
                let full_cmd = cmd.trim();
                // Skip "fill_text" and find the quoted string
                let after_op = full_cmd["fill_text".len()..].trim();

                let (text, consumed) = parse_quoted_string(after_op);

                // Parse x, y from remaining
                let remaining = after_op[consumed..].trim();
                let coords: Vec<&str> = remaining.split_whitespace().collect();

                if coords.len() >= 2 {
                    let x = parse_float(coords[0]);
                    let y = parse_float(coords[1]);
                    ctx.fill_text(&text, x, y);
                }
            }
            "begin_path" => {
                ctx.begin_path();
            }
            "move_to" => {
                if parts.len() >= 3 {
                    ctx.move_to(parse_float(parts[1]), parse_float(parts[2]));
                }
            }
            "line_to" => {
                if parts.len() >= 3 {
                    ctx.line_to(parse_float(parts[1]), parse_float(parts[2]));
                }
            }
            "arc" => {
                if parts.len() >= 6 {
                    let x = parse_float(parts[1]);
                    let y = parse_float(parts[2]);
                    let r = parse_float(parts[3]);
                    let start = parse_float(parts[4]);
                    let end = parse_float(parts[5]);
                    ctx.arc(x, y, r, start, end, false);
                }
            }
            "close_path" => {
                ctx.close_path();
            }
            "fill" => {
                ctx.fill();
            }
            "stroke" => {
                ctx.stroke();
            }
            "save" => {
                ctx.save();
            }
            "restore" => {
                ctx.restore();
            }
            "create_linear_gradient" => {
                if parts.len() >= 6 {
                    let id = parts[1];
                    let x0 = parse_float(parts[2]);
                    let y0 = parse_float(parts[3]);
                    let x1 = parse_float(parts[4]);
                    let y1 = parse_float(parts[5]);
                    let gradient = ctx.create_linear_gradient(x0, y0, x1, y1);
                    gradients.insert(id.to_string(), Gradient::Linear(gradient));
                }
            }
            "create_radial_gradient" => {
                if parts.len() >= 8 {
                    let id = parts[1];
                    let x0 = parse_float(parts[2]);
                    let y0 = parse_float(parts[3]);
                    let r0 = parse_float(parts[4]);
                    let x1 = parse_float(parts[5]);
                    let y1 = parse_float(parts[6]);
                    let r1 = parse_float(parts[7]);
                    let gradient = ctx.create_radial_gradient(x0, y0, r0, x1, y1, r1);
                    gradients.insert(id.to_string(), Gradient::Radial(gradient));
                }
            }
            "add_color_stop" => {
                let rest = cmd[op.len()..].trim();
                if let Some((id, rest)) = split_next_token(rest) {
                    if let Some((offset, color)) = split_next_token(rest) {
                        if color.is_empty() {
                            continue;
                        }
                        if let Some(gradient) = gradients.get_mut(id) {
                            match gradient {
                                Gradient::Linear(g) => g.add_color_stop(parse_float(offset), color),
                                Gradient::Radial(g) => g.add_color_stop(parse_float(offset), color),
                            }
                        } else {
                            eprintln!("Warning: Gradient '{}' not found", id);
                        }
                    }
                }
            }
            "set_fill_gradient" => {
                if parts.len() >= 2 {
                    let id = parts[1];
                    if let Some(gradient) = gradients.get(id) {
                        match gradient {
                            Gradient::Linear(g) => ctx.set_fill_style_gradient(g),
                            Gradient::Radial(g) => ctx.set_fill_style_radial_gradient(g),
                        }
                    } else {
                        eprintln!("Warning: Gradient '{}' not found", id);
                    }
                }
            }
            "set_stroke_gradient" => {
                if parts.len() >= 2 {
                    let id = parts[1];
                    if let Some(gradient) = gradients.get(id) {
                        match gradient {
                            Gradient::Linear(g) => ctx.set_stroke_style_gradient(g),
                            Gradient::Radial(g) => ctx.set_stroke_style_radial_gradient(g),
                        }
                    } else {
                        eprintln!("Warning: Gradient '{}' not found", id);
                    }
                }
            }
            _ => {
                eprintln!("Warning: Unknown operation: {}", op);
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let (input, output, output_data_url) = parse_args(&args[1..]);

    let input = match input {
        Some(i) => i,
        None => {
            eprintln!("Error: --input is required");
            std::process::exit(1);
        }
    };

    // Check that either --output or --output-data-url is specified
    if output.is_none() && !output_data_url {
        eprintln!("Error: Either --output or --output-data-url is required");
        std::process::exit(1);
    }

    // Check if input is a file path or inline commands
    let (commands, base_path): (Vec<String>, std::path::PathBuf) = if Path::new(&input).exists() {
        // Read from file
        let content = fs::read_to_string(&input).expect("Failed to read input file");
        let cmds: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let base_path = Path::new(&input).parent().unwrap_or(Path::new("."));
        (cmds, base_path.to_path_buf())
    } else {
        // Parse inline commands (separated by semicolons)
        let cmds: Vec<String> = input.split(';').map(|s| s.to_string()).collect();
        (cmds, Path::new(".").to_path_buf())
    };

    // First line should be "canvas width height"
    if commands.is_empty() {
        eprintln!("Error: No commands found");
        std::process::exit(1);
    }

    let canvas_index = match first_meaningful_command_index(&commands) {
        Some(index) => index,
        None => {
            eprintln!("Error: No commands found");
            std::process::exit(1);
        }
    };

    let first_line = commands[canvas_index].trim();
    let canvas_parts: Vec<&str> = first_line.split_whitespace().collect();

    if canvas_parts.is_empty() || canvas_parts[0] != "canvas" {
        eprintln!("Error: First line must be 'canvas <width> <height>'");
        std::process::exit(1);
    }

    if canvas_parts.len() < 3 {
        eprintln!("Error: Canvas dimensions required: canvas <width> <height>");
        std::process::exit(1);
    }

    let width = parse_u32(canvas_parts[1]);
    let height = parse_u32(canvas_parts[2]);

    let canvas = Canvas::new(width, height);
    let mut ctx = canvas.get_context("2d").expect("Failed to get 2d context");

    // Execute remaining commands
    execute_commands(&mut ctx, &commands[canvas_index + 1..], &base_path);

    // Output
    let png_bytes = images::to_blob(&canvas);

    if output_data_url {
        // Output as data URL
        let base64 = base64_encode(&png_bytes);
        println!("data:image/png;base64,{}", base64);
    } else if let Some(output_path) = output {
        fs::write(&output_path, &png_bytes).expect("Failed to write output file");
        println!("Output saved to: {}", output_path);
    }
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        result.push(ALPHABET[b0 >> 2] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_meaningful_command_index_skips_blank_and_comment_lines() {
        let commands = vec![
            "".to_string(),
            "   ".to_string(),
            "# generated by script".to_string(),
            "  # keep this note".to_string(),
            "canvas 200 100".to_string(),
            "fill_rect 0 0 10 10".to_string(),
        ];

        assert_eq!(first_meaningful_command_index(&commands), Some(4));
    }

    #[test]
    fn first_meaningful_command_index_returns_none_when_only_comments_exist() {
        let commands = vec!["".to_string(), " # comment".to_string(), "   ".to_string()];

        assert_eq!(first_meaningful_command_index(&commands), None);
    }

    #[test]
    fn split_next_token_preserves_remaining_text() {
        let (token, rest) = split_next_token("gradient 0.5 rgba(255, 0, 0, 0.08)").unwrap();
        assert_eq!(token, "gradient");
        assert_eq!(rest, "0.5 rgba(255, 0, 0, 0.08)");
    }

    #[test]
    fn execute_commands_accepts_fill_style_with_spaces() {
        let canvas = Canvas::new(1, 1);
        let mut ctx = canvas.get_context("2d").unwrap();
        let commands = vec![
            "set_fill_style rgba(255, 0, 0, 0.08)".to_string(),
            "fill_rect 0 0 1 1".to_string(),
        ];

        execute_commands(&mut ctx, &commands, Path::new("."));

        let pixel = canvas.get_image_data().get_pixel(0, 0);
        assert_eq!(pixel.r, 255);
        assert_eq!(pixel.g, 0);
        assert_eq!(pixel.b, 0);
        assert_eq!(pixel.a, 20);
    }

    #[test]
    fn execute_commands_accepts_gradient_color_stop_with_spaces() {
        let canvas = Canvas::new(1, 1);
        let mut ctx = canvas.get_context("2d").unwrap();
        let commands = vec![
            "create_linear_gradient g 0 0 1 0".to_string(),
            "add_color_stop g 0 rgba(255, 0, 0, 0.08)".to_string(),
            "add_color_stop g 1 rgba(0, 0, 255, 0.16)".to_string(),
            "set_fill_gradient g".to_string(),
            "fill_rect 0 0 1 1".to_string(),
        ];

        execute_commands(&mut ctx, &commands, Path::new("."));

        let pixel = canvas.get_image_data().get_pixel(0, 0);
        assert_eq!(pixel.r, 255);
        assert_eq!(pixel.g, 0);
        assert_eq!(pixel.b, 0);
        assert_eq!(pixel.a, 20);
    }

    #[test]
    fn execute_commands_supports_round_rect_path() {
        let canvas = Canvas::new(16, 16);
        let mut ctx = canvas.get_context("2d").unwrap();
        let commands = vec![
            "set_fill_style red".to_string(),
            "begin_path".to_string(),
            "roundRect 2 2 12 12 4,4,4,4".to_string(),
            "fill".to_string(),
        ];

        execute_commands(&mut ctx, &commands, Path::new("."));

        let image = canvas.get_image_data();
        assert_eq!(image.get_pixel(8, 8).r, 255);
        assert_eq!(image.get_pixel(2, 2).a, 0);
    }

    #[test]
    fn parse_quoted_string_unescapes_common_sequences() {
        let (text, consumed) = parse_quoted_string("\"A\\nB\\t\\\"C\\\"\" 10 20");

        assert_eq!(text, "A\nB\t\"C\"");
        assert_eq!(consumed, 13);
    }

    #[test]
    fn execute_commands_fill_text_unescapes_newline_sequences() {
        let multiline_canvas = Canvas::new(64, 48);
        let mut multiline_ctx = multiline_canvas.get_context("2d").unwrap();
        let multiline_commands = vec![
            "set_fill_style black".to_string(),
            "set_font 16px common".to_string(),
            "fill_text \"A\\nB\" 4 4".to_string(),
        ];

        execute_commands(&mut multiline_ctx, &multiline_commands, Path::new("."));

        let manual_canvas = Canvas::new(64, 48);
        let mut manual_ctx = manual_canvas.get_context("2d").unwrap();
        let manual_commands = vec![
            "set_fill_style black".to_string(),
            "set_font 16px common".to_string(),
            "fill_text \"A\" 4 4".to_string(),
            "fill_text \"B\" 4 20".to_string(),
        ];

        execute_commands(&mut manual_ctx, &manual_commands, Path::new("."));

        assert_eq!(
            multiline_canvas.get_image_data().data,
            manual_canvas.get_image_data().data,
        );
    }
}