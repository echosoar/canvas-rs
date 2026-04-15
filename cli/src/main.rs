use canvas::Canvas;
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
    println!("  draw_image <path> <x> <y>        - Draw an image at position (x, y)");
    println!("  set_fill_style <color>            - Set fill style (e.g., red, #ff0000, rgb(255,0,0))");
    println!("  set_stroke_style <color>          - Set stroke style");
    println!("  set_font <size>px <family>        - Set font (e.g., 32px common)");
    println!("  set_line_width <width>            - Set line width");
    println!("  fill_rect <x> <y> <w> <h>         - Fill a rectangle");
    println!("  stroke_rect <x> <y> <w> <h>       - Stroke a rectangle");
    println!("  fill_text \"<text>\" <x> <y>       - Fill text at position (x, y)");
    println!("  begin_path                        - Begin a new path");
    println!("  move_to <x> <y>                   - Move to position");
    println!("  line_to <x> <y>                   - Draw line to position");
    println!("  arc <x> <y> <r> <start> <end>     - Draw arc");
    println!("  close_path                        - Close the current path");
    println!("  fill                              - Fill the current path");
    println!("  stroke                            - Stroke the current path");
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

    // Find closing quote
    let rest = &s[1..]; // Skip opening quote
    if let Some(close_pos) = rest.find('"') {
        let text = &rest[..close_pos];
        // Return byte index: 1 (opening quote) + text bytes + 1 (closing quote)
        let consumed = 1 + text.len() + 1;
        (text.to_string(), consumed)
    } else {
        // No closing quote, take all
        (rest.to_string(), s.len())
    }
}

fn execute_commands(ctx: &mut canvas::Context2D, commands: &[String], base_path: &Path) {
    for cmd in commands {
        let cmd = cmd.trim();
        if cmd.is_empty() {
            continue;
        }

        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let op = parts[0];

        match op {
            "draw_image" => {
                if parts.len() >= 4 {
                    let path = parts[1];
                    let x = parse_float(parts[2]);
                    let y = parse_float(parts[3]);

                    // Resolve path relative to input file
                    let image_path = if Path::new(path).is_absolute() {
                        path.to_string()
                    } else {
                        base_path.join(path).to_string_lossy().to_string()
                    };

                    if let Ok(png_bytes) = fs::read(&image_path) {
                        if let Ok(img_data) = images::from_png(&png_bytes) {
                            ctx.draw_image(&img_data, x, y);
                        } else {
                            eprintln!("Warning: Failed to decode image: {}", image_path);
                        }
                    } else {
                        eprintln!("Warning: Failed to read image: {}", image_path);
                    }
                }
            }
            "set_fill_style" => {
                if parts.len() >= 2 {
                    ctx.set_fill_style(parts[1]);
                }
            }
            "set_stroke_style" => {
                if parts.len() >= 2 {
                    ctx.set_stroke_style(parts[1]);
                }
            }
            "set_font" => {
                if parts.len() >= 3 {
                    // Reconstruct font string like "32px common"
                    let font_str = parts[1..].join(" ");
                    ctx.set_font(&font_str);
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

    let first_line = commands[0].trim();
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
    execute_commands(&mut ctx, &commands[1..], &base_path);

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