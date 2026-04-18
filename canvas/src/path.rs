use core::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundRectPath {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
    pub radii: [f64; 4],
}

/// A single command in a 2-D path.
#[derive(Clone, Debug)]
pub enum PathCommand {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    /// arc(cx, cy, radius, start_angle, end_angle, counterclockwise)
    Arc(f64, f64, f64, f64, f64, bool),
    RoundRect(RoundRectPath),
    ClosePath,
}

/// An ordered list of path commands, exactly matching the web-canvas path model.
#[derive(Clone, Debug, Default)]
pub struct Path {
    pub commands: Vec<PathCommand>,
}

impl Path {
    pub fn new() -> Self {
        Path {
            commands: Vec::new(),
        }
    }

    /// Flatten the path to a list of *sub-paths*, each being a closed or open
    /// polygon (list of (x, y) points).  Arcs are approximated with line
    /// segments.
    pub fn flatten(&self) -> Vec<Vec<(f64, f64)>> {
        let mut sub_paths: Vec<Vec<(f64, f64)>> = Vec::new();
        let mut current: Vec<(f64, f64)> = Vec::new();
        let mut pen = (0.0_f64, 0.0_f64);

        for cmd in &self.commands {
            match *cmd {
                PathCommand::MoveTo(x, y) => {
                    if current.len() >= 2 {
                        sub_paths.push(current.clone());
                    }
                    current.clear();
                    current.push((x, y));
                    pen = (x, y);
                }
                PathCommand::LineTo(x, y) => {
                    if current.is_empty() {
                        current.push(pen);
                    }
                    current.push((x, y));
                    pen = (x, y);
                }
                PathCommand::Arc(cx, cy, r, start, end, ccw) => {
                    let pts = arc_to_points(cx, cy, r, start, end, ccw);
                    if !pts.is_empty() {
                        // If we have a current open path, lineTo the first arc point.
                        if current.is_empty() {
                            current.push(pts[0]);
                        } else {
                            // implicit lineTo first arc point if different from pen
                            if (pts[0].0 - pen.0).abs() > 1e-9
                                || (pts[0].1 - pen.1).abs() > 1e-9
                            {
                                current.push(pts[0]);
                            }
                        }
                        for &pt in pts.iter().skip(1) {
                            current.push(pt);
                        }
                        pen = *pts.last().unwrap();
                    }
                }
                PathCommand::RoundRect(round_rect) => {
                    if current.len() >= 2 {
                        sub_paths.push(current.clone());
                    }
                    current.clear();

                    let pts = round_rect_to_points(round_rect);
                    if !pts.is_empty() {
                        pen = pts[0];
                        sub_paths.push(pts);
                    }
                }
                PathCommand::ClosePath => {
                    if let Some(&first) = current.first() {
                        current.push(first);
                    }
                    if current.len() >= 2 {
                        sub_paths.push(current.clone());
                    }
                    current.clear();
                    // pen stays at the start of the closed sub-path (web spec)
                    if let Some(sp) = sub_paths.last() {
                        if let Some(&p) = sp.first() {
                            pen = p;
                        }
                    }
                }
            }
        }

        if current.len() >= 2 {
            sub_paths.push(current);
        }
        sub_paths
    }

    pub fn as_round_rect(&self) -> Option<RoundRectPath> {
        match self.commands.as_slice() {
            [PathCommand::RoundRect(round_rect)] => Some(*round_rect),
            [PathCommand::RoundRect(round_rect), PathCommand::ClosePath] => Some(*round_rect),
            _ => None,
        }
    }
}

fn round_rect_to_points(round_rect: RoundRectPath) -> Vec<(f64, f64)> {
    let left = round_rect.left;
    let top = round_rect.top;
    let right = left + round_rect.width;
    let bottom = top + round_rect.height;
    let [top_left, top_right, bottom_right, bottom_left] = round_rect.radii;

    let mut pts = Vec::new();
    pts.push((left + top_left, top));
    pts.push((right - top_right, top));
    if top_right > 0.0 {
        pts.extend(arc_to_points(
            right - top_right,
            top + top_right,
            top_right,
            -std::f64::consts::FRAC_PI_2,
            0.0,
            false,
        ).into_iter().skip(1));
    }

    pts.push((right, bottom - bottom_right));
    if bottom_right > 0.0 {
        pts.extend(arc_to_points(
            right - bottom_right,
            bottom - bottom_right,
            bottom_right,
            0.0,
            std::f64::consts::FRAC_PI_2,
            false,
        ).into_iter().skip(1));
    }

    pts.push((left + bottom_left, bottom));
    if bottom_left > 0.0 {
        pts.extend(arc_to_points(
            left + bottom_left,
            bottom - bottom_left,
            bottom_left,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::PI,
            false,
        ).into_iter().skip(1));
    }

    pts.push((left, top + top_left));
    if top_left > 0.0 {
        pts.extend(arc_to_points(
            left + top_left,
            top + top_left,
            top_left,
            std::f64::consts::PI,
            std::f64::consts::PI * 1.5,
            false,
        ).into_iter().skip(1));
    }

    if let Some(&first) = pts.first() {
        pts.push(first);
    }

    pts
}

/// Approximate an arc with line segments.
/// Returns at least 2 points (start and end).
pub fn arc_to_points(
    cx: f64,
    cy: f64,
    radius: f64,
    start: f64,
    end: f64,
    counterclockwise: bool,
) -> Vec<(f64, f64)> {
    if radius <= 0.0 {
        return vec![(cx, cy)];
    }

    // Normalise the angular range following the HTML canvas specification.
    let (s, mut e) = (start, end);
    if counterclockwise {
        // CCW: sweep from start downward to end.
        while e > s {
            e -= 2.0 * PI;
        }
    } else {
        // CW: sweep from start upward to end.
        while e < s {
            e += 2.0 * PI;
        }
    }

    let sweep = (e - s).abs();
    // Aim for roughly 1-pixel resolution along the arc.
    let steps = ((sweep * radius).ceil() as usize).max(4);
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let angle = s + (e - s) * t;
        pts.push((cx + radius * angle.cos(), cy + radius * angle.sin()));
    }
    pts
}
