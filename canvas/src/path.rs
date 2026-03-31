use core::f64::consts::PI;

/// A single command in a 2-D path.
#[derive(Clone, Debug)]
pub enum PathCommand {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    /// arc(cx, cy, radius, start_angle, end_angle, counterclockwise)
    Arc(f64, f64, f64, f64, f64, bool),
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
