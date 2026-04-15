use crate::color::{parse_color, Color};

/// A color stop in a gradient.
#[derive(Clone, Debug)]
pub struct ColorStop {
    /// Offset between 0 and 1
    pub offset: f64,
    /// Color at this offset
    pub color: Color,
}

/// A linear gradient.
#[derive(Clone, Debug)]
pub struct LinearGradient {
    /// Start point (x0, y0)
    pub x0: f64,
    pub y0: f64,
    /// End point (x1, y1)
    pub x1: f64,
    pub y1: f64,
    /// Color stops sorted by offset
    pub stops: Vec<ColorStop>,
}

impl LinearGradient {
    /// Create a new linear gradient from (x0, y0) to (x1, y1).
    pub fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        LinearGradient {
            x0,
            y0,
            x1,
            y1,
            stops: Vec::new(),
        }
    }

    /// Add a color stop at the given offset (0-1).
    /// Invalid offsets (<0 or >1) are ignored.
    /// Invalid color strings are silently ignored.
    pub fn add_color_stop(&mut self, offset: f64, color: &str) {
        if offset < 0.0 || offset > 1.0 {
            return;
        }
        if let Some(c) = parse_color(color) {
            self.stops.push(ColorStop { offset, color: c });
            // Keep stops sorted by offset
            self.stops.sort_by(|a, b| {
                a.offset.partial_cmp(&b.offset).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    /// Get the color at a given point (x, y).
    /// Computes the position along the gradient line and interpolates colors.
    pub fn color_at(&self, x: f64, y: f64) -> Color {
        if self.stops.is_empty() {
            return Color::transparent();
        }
        if self.stops.len() == 1 {
            return self.stops[0].color;
        }

        // Compute position along gradient line (0 to 1)
        let dx = self.x1 - self.x0;
        let dy = self.y1 - self.y0;
        let len_sq = dx * dx + dy * dy;

        if len_sq < 1e-9 {
            // Degenerate gradient (points are the same)
            return self.stops[0].color;
        }

        // Project point onto gradient line
        let px = x - self.x0;
        let py = y - self.y0;
        let t = (px * dx + py * dy) / len_sq;

        // Clamp t to [0, 1] for positions outside the gradient line
        let t = t.clamp(0.0, 1.0);

        // Find the two stops to interpolate between
        self.interpolate_color(t)
    }

    /// Interpolate color at position t (0-1) between stops.
    fn interpolate_color(&self, t: f64) -> Color {
        // Find stops surrounding t
        let mut before: Option<&ColorStop> = None;
        let mut after: Option<&ColorStop> = None;

        for stop in &self.stops {
            if stop.offset <= t {
                before = Some(stop);
            }
            if stop.offset >= t && after.is_none() {
                after = Some(stop);
                break;
            }
        }

        match (before, after) {
            (Some(b), Some(a)) if b.offset == a.offset => b.color,
            (Some(b), Some(a)) => {
                // Interpolate between b and a
                let range = a.offset - b.offset;
                let ratio = if range > 0.0 { (t - b.offset) / range } else { 0.0 };
                interpolate_colors(b.color, a.color, ratio)
            },
            (Some(b), None) => b.color, // t is after last stop
            (None, Some(a)) => a.color, // t is before first stop
            (None, None) => Color::transparent(),
        }
    }
}

/// A radial gradient.
#[derive(Clone, Debug)]
pub struct RadialGradient {
    /// Start circle center (x0, y0) and radius r0
    pub x0: f64,
    pub y0: f64,
    pub r0: f64,
    /// End circle center (x1, y1) and radius r1
    pub x1: f64,
    pub y1: f64,
    pub r1: f64,
    /// Color stops sorted by offset
    pub stops: Vec<ColorStop>,
}

impl RadialGradient {
    /// Create a new radial gradient from (x0, y0, r0) to (x1, y1, r1).
    pub fn new(x0: f64, y0: f64, r0: f64, x1: f64, y1: f64, r1: f64) -> Self {
        RadialGradient {
            x0,
            y0,
            r0,
            x1,
            y1,
            r1,
            stops: Vec::new(),
        }
    }

    /// Add a color stop at the given offset (0-1).
    /// Invalid offsets (<0 or >1) are ignored.
    /// Invalid color strings are silently ignored.
    pub fn add_color_stop(&mut self, offset: f64, color: &str) {
        if offset < 0.0 || offset > 1.0 {
            return;
        }
        if let Some(c) = parse_color(color) {
            self.stops.push(ColorStop { offset, color: c });
            // Keep stops sorted by offset
            self.stops.sort_by(|a, b| {
                a.offset.partial_cmp(&b.offset).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    /// Get the color at a given point (x, y).
    /// Computes the radial position and interpolates colors.
    pub fn color_at(&self, x: f64, y: f64) -> Color {
        if self.stops.is_empty() {
            return Color::transparent();
        }
        if self.stops.len() == 1 {
            return self.stops[0].color;
        }

        // Compute position t along the radial gradient
        // The gradient goes from inner circle to outer circle
        // t = 0 at inner circle, t = 1 at outer circle

        // Distance from the inner circle center
        let dx_inner = x - self.x0;
        let dy_inner = y - self.y0;
        let dist_from_inner = (dx_inner * dx_inner + dy_inner * dy_inner).sqrt();

        // For a standard radial gradient (concentric circles), use distance ratio
        // But the Web Canvas spec uses a more complex formula for non-concentric

        if self.r1 <= self.r0 && (self.x0 == self.x1 && self.y0 == self.y1) {
            // Degenerate case: inner circle >= outer circle with same center
            return self.stops[0].color;
        }

        // Simplified: use concentric approach when centers are the same
        if self.x0 == self.x1 && self.y0 == self.y1 {
            let t = if self.r1 > self.r0 {
                ((dist_from_inner - self.r0) / (self.r1 - self.r0)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return self.interpolate_color(t);
        }

        // Non-concentric: use the distance from outer center as primary factor
        // This is a simplified approximation - the actual Canvas spec uses complex cone geometry
        let dx_outer = x - self.x1;
        let dy_outer = y - self.y1;
        let dist_from_outer = (dx_outer * dx_outer + dy_outer * dy_outer).sqrt();

        // Interpolate based on position relative to both circles
        let inner_radius = self.r0;
        let outer_radius = self.r1;

        if dist_from_outer >= outer_radius {
            // Outside outer circle
            return self.interpolate_color(1.0);
        }
        if dist_from_inner <= inner_radius {
            // Inside inner circle
            return self.interpolate_color(0.0);
        }

        // Approximate interpolation
        let range = outer_radius - inner_radius;
        let t = if range > 0.0 {
            ((dist_from_inner - inner_radius) / range).clamp(0.0, 1.0)
        } else {
            0.0
        };

        self.interpolate_color(t)
    }

    /// Interpolate color at position t (0-1) between stops.
    fn interpolate_color(&self, t: f64) -> Color {
        let mut before: Option<&ColorStop> = None;
        let mut after: Option<&ColorStop> = None;

        for stop in &self.stops {
            if stop.offset <= t {
                before = Some(stop);
            }
            if stop.offset >= t && after.is_none() {
                after = Some(stop);
                break;
            }
        }

        match (before, after) {
            (Some(b), Some(a)) if b.offset == a.offset => b.color,
            (Some(b), Some(a)) => {
                let range = a.offset - b.offset;
                let ratio = if range > 0.0 { (t - b.offset) / range } else { 0.0 };
                interpolate_colors(b.color, a.color, ratio)
            },
            (Some(b), None) => b.color,
            (None, Some(a)) => a.color,
            (None, None) => Color::transparent(),
        }
    }
}

/// A style that can be used for fill or stroke.
#[derive(Clone, Debug)]
pub enum Style {
    Color(Color),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
}

impl Style {
    /// Get the color at a given point for this style.
    /// For solid colors, returns the same color regardless of position.
    /// For gradients, computes the color at that position.
    pub fn color_at(&self, x: f64, y: f64) -> Color {
        match self {
            Style::Color(c) => *c,
            Style::LinearGradient(g) => g.color_at(x, y),
            Style::RadialGradient(g) => g.color_at(x, y),
        }
    }

    /// Create a Style from a color.
    pub fn from_color(color: Color) -> Self {
        Style::Color(color)
    }

    /// Create a Style from a CSS color string.
    pub fn from_color_str(s: &str) -> Option<Self> {
        parse_color(s).map(Style::Color)
    }
}

/// Interpolate between two colors.
fn interpolate_colors(c0: Color, c1: Color, t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::rgba(
        (c0.r as f64 + (c1.r as f64 - c0.r as f64) * t).round() as u8,
        (c0.g as f64 + (c1.g as f64 - c0.g as f64) * t).round() as u8,
        (c0.b as f64 + (c1.b as f64 - c0.b as f64) * t).round() as u8,
        (c0.a as f64 + (c1.a as f64 - c0.a as f64) * t).round() as u8,
    )
}