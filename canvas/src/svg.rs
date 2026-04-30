//! SVG parser and renderer.
//!
//! Supports a practical subset of SVG 1.1 including:
//! - Basic shapes: `<rect>`, `<circle>`, `<ellipse>`, `<line>`, `<polyline>`, `<polygon>`
//! - Paths: `<path>` with full SVG path command support
//! - Groups: `<g>` with style inheritance
//! - Presentation attributes: `fill`, `stroke`, `stroke-width`, `opacity`,
//!   `fill-opacity`, `stroke-opacity`, `fill-rule`
//! - Transform attribute: `translate`, `scale`, `rotate`, `matrix`
//! - `viewBox` for coordinate mapping

use std::f64::consts::PI;

use crate::canvas::{Canvas, Context2D};
use crate::color::{parse_color, Color};
use crate::image::ImageData;

// ── XML parser ───────────────────────────────────────────────────────────────

/// A parsed XML element with its tag name, attributes, and children.
#[derive(Debug, Clone)]
pub struct XmlElement {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<XmlNode>,
}

/// A node in the XML tree: either an element or raw text.
#[derive(Debug, Clone)]
pub enum XmlNode {
    Element(XmlElement),
    Text(String),
}

impl XmlElement {
    /// Return the value of the attribute with the given name, if present.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Parse the attribute value as an `f64`, optionally stripping a trailing
    /// `"px"` unit.
    pub fn attr_f64(&self, name: &str) -> Option<f64> {
        let v = self.attr(name)?.trim();
        let v = v.strip_suffix("px").unwrap_or(v);
        v.parse().ok()
    }

    /// Iterate over direct child elements (ignoring text nodes).
    pub fn child_elements(&self) -> impl Iterator<Item = &XmlElement> {
        self.children.iter().filter_map(|n| {
            if let XmlNode::Element(e) = n {
                Some(e)
            } else {
                None
            }
        })
    }
}

/// Parse an XML/SVG byte slice and return the root element.
pub fn parse_xml(input: &str) -> Option<XmlElement> {
    let mut pos = 0;
    skip_xml_prolog(input, &mut pos);
    parse_element(input, &mut pos)
}

fn skip_whitespace(s: &str, pos: &mut usize) {
    while *pos < s.len() && s.as_bytes()[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
}

/// Skip the XML prolog: `<?xml ... ?>` and `<!DOCTYPE ...>`.
fn skip_xml_prolog(s: &str, pos: &mut usize) {
    loop {
        skip_whitespace(s, pos);
        if s[*pos..].starts_with("<?") {
            if let Some(end) = s[*pos..].find("?>") {
                *pos += end + 2;
            } else {
                break;
            }
        } else if s[*pos..].starts_with("<!DOCTYPE") || s[*pos..].starts_with("<!doctype") {
            // Skip until closing '>'
            let mut depth = 0usize;
            while *pos < s.len() {
                match s.as_bytes()[*pos] {
                    b'<' => {
                        depth += 1;
                        *pos += 1;
                    }
                    b'>' => {
                        *pos += 1;
                        if depth <= 1 {
                            break;
                        }
                        depth -= 1;
                    }
                    _ => {
                        *pos += 1;
                    }
                }
            }
        } else {
            break;
        }
    }
}

/// Parse a single element starting at `pos`, which must point at `<`.
/// Returns `None` if parsing fails.
fn parse_element(s: &str, pos: &mut usize) -> Option<XmlElement> {
    skip_whitespace(s, pos);
    // Skip comments and processing instructions that appear between elements
    loop {
        if s[*pos..].starts_with("<!--") {
            if let Some(end) = s[*pos..].find("-->") {
                *pos += end + 3;
                skip_whitespace(s, pos);
            } else {
                return None;
            }
        } else if s[*pos..].starts_with("<?") {
            if let Some(end) = s[*pos..].find("?>") {
                *pos += end + 2;
                skip_whitespace(s, pos);
            } else {
                return None;
            }
        } else {
            break;
        }
    }

    if *pos >= s.len() || s.as_bytes()[*pos] != b'<' {
        return None;
    }
    *pos += 1; // consume '<'

    // Read tag name
    let name = read_name(s, pos);
    if name.is_empty() {
        return None;
    }

    // Read attributes
    let mut attributes = Vec::new();
    loop {
        skip_whitespace(s, pos);
        if *pos >= s.len() {
            return None;
        }
        let b = s.as_bytes()[*pos];
        if b == b'>' {
            *pos += 1;
            break;
        }
        if b == b'/' {
            // Self-closing tag
            *pos += 1;
            skip_whitespace(s, pos);
            if s.as_bytes().get(*pos) == Some(&b'>') {
                *pos += 1;
            }
            return Some(XmlElement {
                name,
                attributes,
                children: Vec::new(),
            });
        }

        // Attribute name
        let attr_name = read_name(s, pos);
        if attr_name.is_empty() {
            // Skip unrecognised character
            *pos += 1;
            continue;
        }
        skip_whitespace(s, pos);
        let attr_value = if s.as_bytes().get(*pos) == Some(&b'=') {
            *pos += 1;
            skip_whitespace(s, pos);
            read_attr_value(s, pos)
        } else {
            // Boolean attribute with no value
            attr_name.clone()
        };
        attributes.push((attr_name, attr_value));
    }

    // Parse children until we hit the closing tag
    let mut children = Vec::new();
    loop {
        skip_whitespace(s, pos);
        if *pos >= s.len() {
            break;
        }
        if s[*pos..].starts_with("</") {
            // Closing tag – consume it
            *pos += 2;
            // skip tag name
            read_name(s, pos);
            skip_whitespace(s, pos);
            if s.as_bytes().get(*pos) == Some(&b'>') {
                *pos += 1;
            }
            break;
        }
        if s[*pos..].starts_with("<!--") {
            // Skip comment
            if let Some(end) = s[*pos..].find("-->") {
                *pos += end + 3;
            } else {
                break;
            }
            continue;
        }
        if s[*pos..].starts_with("<![CDATA[") {
            *pos += 9;
            if let Some(end) = s[*pos..].find("]]>") {
                let text = s[*pos..*pos + end].to_string();
                *pos += end + 3;
                children.push(XmlNode::Text(text));
            }
            continue;
        }
        if s.as_bytes()[*pos] == b'<' {
            if let Some(child) = parse_element(s, pos) {
                children.push(XmlNode::Element(child));
            } else {
                // Skip malformed element
                break;
            }
        } else {
            // Text node
            let start = *pos;
            while *pos < s.len() && s.as_bytes()[*pos] != b'<' {
                *pos += 1;
            }
            let text = s[start..*pos].to_string();
            if !text.trim().is_empty() {
                children.push(XmlNode::Text(text));
            }
        }
    }

    Some(XmlElement {
        name,
        attributes,
        children,
    })
}

fn read_name(s: &str, pos: &mut usize) -> String {
    let start = *pos;
    while *pos < s.len() {
        let b = s.as_bytes()[*pos];
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b':' || b == b'.' {
            *pos += 1;
        } else {
            break;
        }
    }
    s[start..*pos].to_string()
}

fn read_attr_value(s: &str, pos: &mut usize) -> String {
    if *pos >= s.len() {
        return String::new();
    }
    let quote = s.as_bytes()[*pos];
    if quote == b'"' || quote == b'\'' {
        *pos += 1;
        let start = *pos;
        while *pos < s.len() && s.as_bytes()[*pos] != quote {
            *pos += 1;
        }
        let val = decode_xml_entities(&s[start..*pos]);
        if *pos < s.len() {
            *pos += 1; // consume closing quote
        }
        val
    } else {
        // Unquoted value
        let start = *pos;
        while *pos < s.len() {
            let b = s.as_bytes()[*pos];
            if b.is_ascii_whitespace() || b == b'>' || b == b'/' {
                break;
            }
            *pos += 1;
        }
        s[start..*pos].to_string()
    }
}

fn decode_xml_entities(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        result.push_str(&rest[..amp]);
        rest = &rest[amp..];
        if rest.starts_with("&amp;") {
            result.push('&');
            rest = &rest[5..];
        } else if rest.starts_with("&lt;") {
            result.push('<');
            rest = &rest[4..];
        } else if rest.starts_with("&gt;") {
            result.push('>');
            rest = &rest[4..];
        } else if rest.starts_with("&quot;") {
            result.push('"');
            rest = &rest[6..];
        } else if rest.starts_with("&apos;") {
            result.push('\'');
            rest = &rest[6..];
        } else if rest.starts_with("&#x") || rest.starts_with("&#X") {
            if let Some(semi) = rest.find(';') {
                let hex = &rest[3..semi];
                if let Ok(n) = u32::from_str_radix(hex, 16) {
                    if let Some(c) = char::from_u32(n) {
                        result.push(c);
                    }
                }
                rest = &rest[semi + 1..];
            } else {
                result.push('&');
                rest = &rest[1..];
            }
        } else if rest.starts_with("&#") {
            if let Some(semi) = rest.find(';') {
                let dec = &rest[2..semi];
                if let Ok(n) = dec.parse::<u32>() {
                    if let Some(c) = char::from_u32(n) {
                        result.push(c);
                    }
                }
                rest = &rest[semi + 1..];
            } else {
                result.push('&');
                rest = &rest[1..];
            }
        } else {
            result.push('&');
            rest = &rest[1..];
        }
    }
    result.push_str(rest);
    result
}

// ── SVG transform ─────────────────────────────────────────────────────────────

/// A 2-D affine transform stored as the 6-component matrix `[a b c d e f]`
/// mapping `(x, y)` → `(a*x + c*y + e, b*x + d*y + f)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Transform {
    pub fn identity() -> Self {
        Transform {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    pub fn translate(tx: f64, ty: f64) -> Self {
        Transform {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: tx,
            f: ty,
        }
    }

    pub fn scale(sx: f64, sy: f64) -> Self {
        Transform {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            e: 0.0,
            f: 0.0,
        }
    }

    pub fn rotate(angle_deg: f64) -> Self {
        let (sin, cos) = angle_deg.to_radians().sin_cos();
        Transform {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Concatenate (pre-multiply) `other` onto `self`, i.e. `self * other`.
    pub fn concat(&self, other: &Transform) -> Self {
        Transform {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            e: self.a * other.e + self.c * other.f + self.e,
            f: self.b * other.e + self.d * other.f + self.f,
        }
    }

    /// Apply the transform to a point.
    #[inline]
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// Parse an SVG transform attribute string like
    /// `"translate(10 20) scale(2) rotate(45)"`.
    pub fn parse(s: &str) -> Self {
        let mut result = Transform::identity();
        let mut rest = s.trim();

        while !rest.is_empty() {
            rest = rest.trim_start();
            if rest.is_empty() {
                break;
            }

            // Find the function name
            let paren = match rest.find('(') {
                Some(p) => p,
                None => break,
            };
            let func = rest[..paren].trim().to_ascii_lowercase();
            let after_paren = &rest[paren + 1..];
            let close = match after_paren.find(')') {
                Some(c) => c,
                None => break,
            };
            let args_str = &after_paren[..close];
            rest = after_paren[close + 1..].trim_start_matches(',');

            let args = parse_number_list(args_str);
            let t = match func.as_str() {
                "translate" => {
                    let tx = args.first().copied().unwrap_or(0.0);
                    let ty = args.get(1).copied().unwrap_or(0.0);
                    Transform::translate(tx, ty)
                }
                "scale" => {
                    let sx = args.first().copied().unwrap_or(1.0);
                    let sy = args.get(1).copied().unwrap_or(sx);
                    Transform::scale(sx, sy)
                }
                "rotate" => {
                    let angle = args.first().copied().unwrap_or(0.0);
                    let cx = args.get(1).copied().unwrap_or(0.0);
                    let cy = args.get(2).copied().unwrap_or(0.0);
                    // rotate(angle, cx, cy) = translate(cx,cy) * rotate(angle) * translate(-cx,-cy)
                    if args.len() >= 3 {
                        Transform::translate(cx, cy)
                            .concat(&Transform::rotate(angle))
                            .concat(&Transform::translate(-cx, -cy))
                    } else {
                        Transform::rotate(angle)
                    }
                }
                "matrix" if args.len() >= 6 => Transform {
                    a: args[0],
                    b: args[1],
                    c: args[2],
                    d: args[3],
                    e: args[4],
                    f: args[5],
                },
                "skewx" => {
                    let angle = args.first().copied().unwrap_or(0.0);
                    Transform {
                        a: 1.0,
                        b: 0.0,
                        c: angle.to_radians().tan(),
                        d: 1.0,
                        e: 0.0,
                        f: 0.0,
                    }
                }
                "skewy" => {
                    let angle = args.first().copied().unwrap_or(0.0);
                    Transform {
                        a: 1.0,
                        b: angle.to_radians().tan(),
                        c: 0.0,
                        d: 1.0,
                        e: 0.0,
                        f: 0.0,
                    }
                }
                _ => Transform::identity(),
            };
            result = result.concat(&t);
        }

        result
    }
}

// ── SVG style ─────────────────────────────────────────────────────────────────

/// The resolved presentation style for an SVG element.
#[derive(Clone, Debug)]
pub struct SvgStyle {
    /// Fill paint – `None` means "inherit", `Some(None)` means `none`/`transparent`.
    pub fill: Option<Option<Color>>,
    /// Stroke paint.
    pub stroke: Option<Option<Color>>,
    pub stroke_width: Option<f64>,
    /// Element-level opacity (0.0–1.0).
    pub opacity: f64,
    /// Fill opacity override (0.0–1.0, applied on top of color alpha).
    pub fill_opacity: Option<f64>,
    /// Stroke opacity override.
    pub stroke_opacity: Option<f64>,
    pub fill_rule: Option<String>,
    pub stroke_linecap: Option<String>,
    pub stroke_linejoin: Option<String>,
}

impl SvgStyle {
    /// The default CSS/SVG initial values.
    fn initial() -> Self {
        SvgStyle {
            fill: Some(Some(Color::black())),
            stroke: Some(None),
            stroke_width: Some(1.0),
            opacity: 1.0,
            fill_opacity: Some(1.0),
            stroke_opacity: Some(1.0),
            fill_rule: Some("nonzero".to_string()),
            stroke_linecap: Some("butt".to_string()),
            stroke_linejoin: Some("miter".to_string()),
        }
    }

    /// Inherit styles from `parent`, then apply values from `element`.
    fn inherit_and_apply(parent: &SvgStyle, element: &XmlElement) -> SvgStyle {
        let mut s = parent.clone();
        s.opacity = 1.0; // opacity is not inherited, reset to 1

        // Parse inline style="" attribute first for lower precedence
        if let Some(style_str) = element.attr("style") {
            apply_css_style_string(&mut s, style_str);
        }

        // Presentation attributes override inherited values
        apply_presentation_attrs(&mut s, element);

        s
    }

    /// Resolve the effective fill color (applying fill-opacity and opacity).
    fn effective_fill(&self) -> Option<Color> {
        let base = self.fill.as_ref()?.as_ref()?.clone();
        let fill_opacity = self.fill_opacity.unwrap_or(1.0);
        let opacity = self.opacity;
        let alpha = (base.a as f64 / 255.0 * fill_opacity * opacity).clamp(0.0, 1.0);
        Some(Color::rgba(base.r, base.g, base.b, (alpha * 255.0).round() as u8))
    }

    /// Resolve the effective stroke color (applying stroke-opacity and opacity).
    fn effective_stroke(&self) -> Option<Color> {
        let base = self.stroke.as_ref()?.as_ref()?.clone();
        let stroke_opacity = self.stroke_opacity.unwrap_or(1.0);
        let opacity = self.opacity;
        let alpha = (base.a as f64 / 255.0 * stroke_opacity * opacity).clamp(0.0, 1.0);
        Some(Color::rgba(base.r, base.g, base.b, (alpha * 255.0).round() as u8))
    }

    fn effective_stroke_width(&self) -> f64 {
        self.stroke_width.unwrap_or(1.0)
    }
}

fn apply_css_style_string(s: &mut SvgStyle, css: &str) {
    for decl in css.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        let colon = match decl.find(':') {
            Some(c) => c,
            None => continue,
        };
        let prop = decl[..colon].trim().to_ascii_lowercase();
        let val = decl[colon + 1..].trim();
        apply_single_style_prop(s, &prop, val);
    }
}

fn apply_presentation_attrs(s: &mut SvgStyle, el: &XmlElement) {
    let props = [
        "fill",
        "stroke",
        "stroke-width",
        "opacity",
        "fill-opacity",
        "stroke-opacity",
        "fill-rule",
        "stroke-linecap",
        "stroke-linejoin",
    ];
    for prop in &props {
        if let Some(val) = el.attr(prop) {
            apply_single_style_prop(s, prop, val);
        }
    }
}

fn apply_single_style_prop(s: &mut SvgStyle, prop: &str, val: &str) {
    let val = val.trim();
    match prop {
        "fill" => {
            if val == "inherit" {
                // leave as is
            } else {
                s.fill = Some(parse_svg_paint(val));
            }
        }
        "stroke" => {
            if val != "inherit" {
                s.stroke = Some(parse_svg_paint(val));
            }
        }
        "stroke-width" => {
            if let Some(w) = parse_svg_length(val) {
                s.stroke_width = Some(w);
            }
        }
        "opacity" => {
            if let Ok(v) = val.trim_end_matches('%').parse::<f64>() {
                s.opacity = if val.ends_with('%') {
                    v / 100.0
                } else {
                    v
                }
                .clamp(0.0, 1.0);
            }
        }
        "fill-opacity" => {
            if let Ok(v) = val.trim_end_matches('%').parse::<f64>() {
                s.fill_opacity = Some(
                    if val.ends_with('%') { v / 100.0 } else { v }.clamp(0.0, 1.0),
                );
            }
        }
        "stroke-opacity" => {
            if let Ok(v) = val.trim_end_matches('%').parse::<f64>() {
                s.stroke_opacity = Some(
                    if val.ends_with('%') { v / 100.0 } else { v }.clamp(0.0, 1.0),
                );
            }
        }
        "fill-rule" => {
            s.fill_rule = Some(val.to_string());
        }
        "stroke-linecap" => {
            s.stroke_linecap = Some(val.to_string());
        }
        "stroke-linejoin" => {
            s.stroke_linejoin = Some(val.to_string());
        }
        _ => {}
    }
}

fn parse_svg_paint(val: &str) -> Option<Color> {
    let val = val.trim();
    if val == "none" || val == "transparent" {
        None
    } else if val.starts_with("url(") {
        // Gradient references not yet supported in this path
        None
    } else {
        parse_color(val)
    }
}

fn parse_svg_length(val: &str) -> Option<f64> {
    let val = val.trim();
    // Strip known length units
    for unit in &["px", "pt", "pc", "mm", "cm", "em", "rem", "%"] {
        if let Some(num) = val.strip_suffix(unit) {
            return num.trim().parse().ok();
        }
    }
    val.parse().ok()
}

// ── Number list / path data helpers ──────────────────────────────────────────

/// Parse a whitespace-and-comma-separated list of numbers.
pub fn parse_number_list(s: &str) -> Vec<f64> {
    let mut nums = Vec::new();
    let mut rest = s.trim();
    while !rest.is_empty() {
        rest = rest.trim_start_matches(|c: char| c == ',' || c.is_ascii_whitespace());
        if rest.is_empty() {
            break;
        }
        let end = rest
            .find(|c: char| {
                // A new token starts at whitespace, comma, or a sign that follows a digit/dot
                c == ',' || c.is_ascii_whitespace()
            })
            .unwrap_or(rest.len());
        if let Ok(n) = rest[..end].parse::<f64>() {
            nums.push(n);
        }
        rest = &rest[end..];
    }
    nums
}

// ── SVG path data parser ──────────────────────────────────────────────────────

/// A flattened path segment ready for rendering.
#[derive(Clone, Debug)]
pub enum FlatCmd {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    ClosePath,
}

/// Parse SVG path `d` attribute and return a list of flat draw commands.
/// Bezier curves are approximated with line segments.
pub fn parse_svg_path(d: &str) -> Vec<FlatCmd> {
    let mut cmds: Vec<FlatCmd> = Vec::new();
    let raw_tokens: Vec<PathToken> = tokenize_path(d);
    let mut ti = 0; // token index

    let mut cur_x = 0.0_f64;
    let mut cur_y = 0.0_f64;
    let mut start_x = 0.0_f64;
    let mut start_y = 0.0_f64;
    // For smooth curves: last control point
    let mut last_ctrl = (0.0_f64, 0.0_f64);

    while ti < raw_tokens.len() {
        let cmd = match &raw_tokens[ti] {
            PathToken::Cmd(c) => {
                ti += 1;
                *c
            }
            _ => {
                ti += 1;
                continue;
            }
        };

        let relative = cmd.is_ascii_lowercase();

        macro_rules! next_f {
            () => {{
                while ti < raw_tokens.len() {
                    if let PathToken::Num(_) = &raw_tokens[ti] {
                        break;
                    }
                    // If we hit another command char, stop consuming numbers for this command
                    if let PathToken::Cmd(_) = &raw_tokens[ti] {
                        break;
                    }
                    ti += 1;
                }
                if ti < raw_tokens.len() {
                    if let PathToken::Num(n) = raw_tokens[ti] {
                        ti += 1;
                        n
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            }};
        }

        macro_rules! has_more_nums {
            () => {{
                let j = ti;
                j < raw_tokens.len() && matches!(&raw_tokens[j], PathToken::Num(_))
            }};
        }

        macro_rules! abs {
            ($rx:expr, $ry:expr) => {
                if relative {
                    (cur_x + $rx, cur_y + $ry)
                } else {
                    ($rx, $ry)
                }
            };
        }

        match cmd.to_ascii_uppercase() {
            'M' => {
                let rx = next_f!();
                let ry = next_f!();
                let (ax, ay) = abs!(rx, ry);
                cur_x = ax;
                cur_y = ay;
                start_x = ax;
                start_y = ay;
                cmds.push(FlatCmd::MoveTo(ax, ay));
                // Subsequent coordinate pairs are treated as implicit L/l
                while has_more_nums!() {
                    let rx2 = next_f!();
                    let ry2 = next_f!();
                    let (ax2, ay2) = abs!(rx2, ry2);
                    cur_x = ax2;
                    cur_y = ay2;
                    cmds.push(FlatCmd::LineTo(ax2, ay2));
                }
                last_ctrl = (cur_x, cur_y);
            }
            'Z' => {
                cmds.push(FlatCmd::ClosePath);
                cur_x = start_x;
                cur_y = start_y;
                last_ctrl = (cur_x, cur_y);
            }
            'L' => {
                loop {
                    let rx = next_f!();
                    let ry = next_f!();
                    let (ax, ay) = abs!(rx, ry);
                    cur_x = ax;
                    cur_y = ay;
                    cmds.push(FlatCmd::LineTo(ax, ay));
                    last_ctrl = (cur_x, cur_y);
                    if !has_more_nums!() {
                        break;
                    }
                }
            }
            'H' => {
                loop {
                    let rx = next_f!();
                    let ax = if relative { cur_x + rx } else { rx };
                    cur_x = ax;
                    cmds.push(FlatCmd::LineTo(cur_x, cur_y));
                    last_ctrl = (cur_x, cur_y);
                    if !has_more_nums!() {
                        break;
                    }
                }
            }
            'V' => {
                loop {
                    let ry = next_f!();
                    let ay = if relative { cur_y + ry } else { ry };
                    cur_y = ay;
                    cmds.push(FlatCmd::LineTo(cur_x, cur_y));
                    last_ctrl = (cur_x, cur_y);
                    if !has_more_nums!() {
                        break;
                    }
                }
            }
            'C' => {
                // Cubic bezier: C x1 y1 x2 y2 x y
                loop {
                    let rx1 = next_f!();
                    let ry1 = next_f!();
                    let rx2 = next_f!();
                    let ry2 = next_f!();
                    let rx = next_f!();
                    let ry = next_f!();
                    let (ax1, ay1) = abs!(rx1, ry1);
                    let (ax2, ay2) = abs!(rx2, ry2);
                    let (ax, ay) = abs!(rx, ry);
                    flatten_cubic(
                        cur_x, cur_y, ax1, ay1, ax2, ay2, ax, ay, &mut cmds,
                    );
                    last_ctrl = (ax2, ay2);
                    cur_x = ax;
                    cur_y = ay;
                    if !has_more_nums!() {
                        break;
                    }
                }
            }
            'S' => {
                // Smooth cubic: S x2 y2 x y
                loop {
                    let rx2 = next_f!();
                    let ry2 = next_f!();
                    let rx = next_f!();
                    let ry = next_f!();
                    let (ax2, ay2) = abs!(rx2, ry2);
                    let (ax, ay) = abs!(rx, ry);
                    // Implicit first control point: reflection of last_ctrl over cur
                    let ax1 = 2.0 * cur_x - last_ctrl.0;
                    let ay1 = 2.0 * cur_y - last_ctrl.1;
                    flatten_cubic(cur_x, cur_y, ax1, ay1, ax2, ay2, ax, ay, &mut cmds);
                    last_ctrl = (ax2, ay2);
                    cur_x = ax;
                    cur_y = ay;
                    if !has_more_nums!() {
                        break;
                    }
                }
            }
            'Q' => {
                // Quadratic bezier: Q x1 y1 x y
                loop {
                    let rx1 = next_f!();
                    let ry1 = next_f!();
                    let rx = next_f!();
                    let ry = next_f!();
                    let (ax1, ay1) = abs!(rx1, ry1);
                    let (ax, ay) = abs!(rx, ry);
                    flatten_quadratic(cur_x, cur_y, ax1, ay1, ax, ay, &mut cmds);
                    last_ctrl = (ax1, ay1);
                    cur_x = ax;
                    cur_y = ay;
                    if !has_more_nums!() {
                        break;
                    }
                }
            }
            'T' => {
                // Smooth quadratic: T x y
                loop {
                    let rx = next_f!();
                    let ry = next_f!();
                    let (ax, ay) = abs!(rx, ry);
                    let ax1 = 2.0 * cur_x - last_ctrl.0;
                    let ay1 = 2.0 * cur_y - last_ctrl.1;
                    flatten_quadratic(cur_x, cur_y, ax1, ay1, ax, ay, &mut cmds);
                    last_ctrl = (ax1, ay1);
                    cur_x = ax;
                    cur_y = ay;
                    if !has_more_nums!() {
                        break;
                    }
                }
            }
            'A' => {
                // Elliptical arc: A rx ry x-rotation large-arc-flag sweep-flag x y
                loop {
                    let arc_rx = next_f!().abs();
                    let arc_ry = next_f!().abs();
                    let x_rotation = next_f!();
                    let large_arc = next_f!() != 0.0;
                    let sweep = next_f!() != 0.0;
                    let rx = next_f!();
                    let ry = next_f!();
                    let (ax, ay) = abs!(rx, ry);
                    flatten_arc(
                        cur_x, cur_y, arc_rx, arc_ry, x_rotation, large_arc, sweep, ax, ay,
                        &mut cmds,
                    );
                    last_ctrl = (cur_x, cur_y);
                    cur_x = ax;
                    cur_y = ay;
                    if !has_more_nums!() {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    cmds
}

#[derive(Clone, Debug)]
enum PathToken {
    Cmd(char),
    Num(f64),
}

fn tokenize_path(d: &str) -> Vec<PathToken> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = d.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_whitespace() || c == ',' {
            i += 1;
            continue;
        }
        if c.is_ascii_alphabetic() {
            tokens.push(PathToken::Cmd(c));
            i += 1;
            continue;
        }
        // Try to parse a number (including optional sign and exponent)
        if c == '-' || c == '+' || c == '.' || c.is_ascii_digit() {
            let start = i;
            if c == '-' || c == '+' {
                i += 1;
            }
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i < chars.len() && chars[i] == '.' {
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                i += 1;
                if i < chars.len() && (chars[i] == '-' || chars[i] == '+') {
                    i += 1;
                }
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let num_str: String = chars[start..i].iter().collect();
            if let Ok(n) = num_str.parse::<f64>() {
                tokens.push(PathToken::Num(n));
            }
            continue;
        }
        i += 1;
    }
    tokens
}

/// Flatten a cubic bezier curve into line segments with adaptive subdivision.
fn flatten_cubic(
    x0: f64, y0: f64,
    x1: f64, y1: f64,
    x2: f64, y2: f64,
    x3: f64, y3: f64,
    cmds: &mut Vec<FlatCmd>,
) {
    // Adaptive subdivision: recurse if the control-point deviation exceeds threshold.
    const THRESHOLD: f64 = 0.5;

    fn subdivide(
        x0: f64, y0: f64, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64,
        cmds: &mut Vec<FlatCmd>, depth: u32,
    ) {
        if depth > 8 {
            cmds.push(FlatCmd::LineTo(x3, y3));
            return;
        }
        // Estimate deviation of control points from chord
        let dx = x3 - x0;
        let dy = y3 - y0;
        let d1 = ((x1 - x0) * dy - (y1 - y0) * dx).abs();
        let d2 = ((x2 - x0) * dy - (y2 - y0) * dx).abs();
        let len2 = dx * dx + dy * dy;
        if (d1 + d2) * (d1 + d2) <= THRESHOLD * THRESHOLD * len2 * 16.0 || len2 < 1e-10 {
            cmds.push(FlatCmd::LineTo(x3, y3));
            return;
        }
        // De Casteljau midpoint subdivision
        let mx01 = (x0 + x1) * 0.5;
        let my01 = (y0 + y1) * 0.5;
        let mx12 = (x1 + x2) * 0.5;
        let my12 = (y1 + y2) * 0.5;
        let mx23 = (x2 + x3) * 0.5;
        let my23 = (y2 + y3) * 0.5;
        let mx012 = (mx01 + mx12) * 0.5;
        let my012 = (my01 + my12) * 0.5;
        let mx123 = (mx12 + mx23) * 0.5;
        let my123 = (my12 + my23) * 0.5;
        let mx0123 = (mx012 + mx123) * 0.5;
        let my0123 = (my012 + my123) * 0.5;
        subdivide(x0, y0, mx01, my01, mx012, my012, mx0123, my0123, cmds, depth + 1);
        subdivide(mx0123, my0123, mx123, my123, mx23, my23, x3, y3, cmds, depth + 1);
    }

    subdivide(x0, y0, x1, y1, x2, y2, x3, y3, cmds, 0);
}

/// Flatten a quadratic bezier to line segments.
fn flatten_quadratic(
    x0: f64, y0: f64,
    x1: f64, y1: f64,
    x2: f64, y2: f64,
    cmds: &mut Vec<FlatCmd>,
) {
    // Elevate to cubic
    let cx1 = x0 + (x1 - x0) * 2.0 / 3.0;
    let cy1 = y0 + (y1 - y0) * 2.0 / 3.0;
    let cx2 = x2 + (x1 - x2) * 2.0 / 3.0;
    let cy2 = y2 + (y1 - y2) * 2.0 / 3.0;
    flatten_cubic(x0, y0, cx1, cy1, cx2, cy2, x2, y2, cmds);
}

/// Flatten an SVG elliptical arc to line segments.
#[allow(clippy::too_many_arguments)]
fn flatten_arc(
    x1: f64, y1: f64,
    mut rx: f64, mut ry: f64,
    x_rotation_deg: f64,
    large_arc: bool,
    sweep: bool,
    x2: f64, y2: f64,
    cmds: &mut Vec<FlatCmd>,
) {
    if (x1 - x2).abs() < 1e-10 && (y1 - y2).abs() < 1e-10 {
        return;
    }
    if rx < 1e-10 || ry < 1e-10 {
        cmds.push(FlatCmd::LineTo(x2, y2));
        return;
    }

    let phi = x_rotation_deg.to_radians();
    let (sin_phi, cos_phi) = phi.sin_cos();

    // Endpoint to center parameterization (SVG spec §B.2.4)
    let dx = (x1 - x2) / 2.0;
    let dy = (y1 - y2) / 2.0;
    let x1p = cos_phi * dx + sin_phi * dy;
    let y1p = -sin_phi * dx + cos_phi * dy;

    // Ensure radii are large enough
    let x1p2 = x1p * x1p;
    let y1p2 = y1p * y1p;
    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let lambda = x1p2 / rx2 + y1p2 / ry2;
    if lambda > 1.0 {
        let lambda_sqrt = lambda.sqrt();
        rx *= lambda_sqrt;
        ry *= lambda_sqrt;
    }
    let rx2 = rx * rx;
    let ry2 = ry * ry;

    let num = (rx2 * ry2 - rx2 * y1p2 - ry2 * x1p2).max(0.0);
    let den = rx2 * y1p2 + ry2 * x1p2;
    let sq = if den < 1e-15 { 0.0 } else { (num / den).sqrt() };
    let sign = if large_arc == sweep { -1.0 } else { 1.0 };
    let cxp = sign * sq * rx * y1p / ry;
    let cyp = -sign * sq * ry * x1p / rx;

    let cx = cos_phi * cxp - sin_phi * cyp + (x1 + x2) / 2.0;
    let cy = sin_phi * cxp + cos_phi * cyp + (y1 + y2) / 2.0;

    let ux = (x1p - cxp) / rx;
    let uy = (y1p - cyp) / ry;
    let vx = (-x1p - cxp) / rx;
    let vy = (-y1p - cyp) / ry;

    let start_angle = angle_between(1.0, 0.0, ux, uy);
    let mut d_angle = angle_between(ux, uy, vx, vy);

    if !sweep && d_angle > 0.0 {
        d_angle -= 2.0 * PI;
    } else if sweep && d_angle < 0.0 {
        d_angle += 2.0 * PI;
    }

    let steps = ((d_angle.abs() * rx.max(ry)).ceil() as usize).max(4).min(512);
    for i in 1..=steps {
        let t = i as f64 / steps as f64;
        let angle = start_angle + d_angle * t;
        let xp = rx * angle.cos();
        let yp = ry * angle.sin();
        let x = cos_phi * xp - sin_phi * yp + cx;
        let y = sin_phi * xp + cos_phi * yp + cy;
        cmds.push(FlatCmd::LineTo(x, y));
    }
}

fn angle_between(ux: f64, uy: f64, vx: f64, vy: f64) -> f64 {
    let dot = ux * vx + uy * vy;
    let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
    let angle = if len < 1e-15 {
        0.0
    } else {
        (dot / len).clamp(-1.0, 1.0).acos()
    };
    if ux * vy - uy * vx < 0.0 {
        -angle
    } else {
        angle
    }
}

// ── ViewBox ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
struct ViewBox {
    min_x: f64,
    min_y: f64,
    width: f64,
    height: f64,
}

/// Parse `viewBox="min-x min-y width height"`.
fn parse_viewbox(s: &str) -> Option<ViewBox> {
    let nums = parse_number_list(s);
    if nums.len() >= 4 {
        Some(ViewBox {
            min_x: nums[0],
            min_y: nums[1],
            width: nums[2],
            height: nums[3],
        })
    } else {
        None
    }
}

// ── SVG renderer ──────────────────────────────────────────────────────────────

/// Render an SVG byte slice at the given destination size and return the
/// resulting `ImageData`.  Returns `None` if the input cannot be parsed as
/// valid SVG.
///
/// `dest_width` / `dest_height` specify the output pixel size.  If both are
/// `0`, the SVG's own intrinsic dimensions (from `width`/`height` attributes
/// or `viewBox`) are used.
pub fn render_svg(
    svg_bytes: &[u8],
    dest_width: u32,
    dest_height: u32,
) -> Option<ImageData> {
    let svg_str = std::str::from_utf8(svg_bytes).ok()?;
    render_svg_str(svg_str, dest_width, dest_height)
}

/// Render an SVG string at the given destination size.
pub fn render_svg_str(
    svg_str: &str,
    dest_width: u32,
    dest_height: u32,
) -> Option<ImageData> {
    let root = parse_xml(svg_str)?;

    // Find the <svg> element (the root or a direct child)
    let svg_el = if root.name.eq_ignore_ascii_case("svg") {
        &root
    } else {
        root.child_elements()
            .find(|e| e.name.eq_ignore_ascii_case("svg"))?
    };

    // Determine intrinsic dimensions
    let intrinsic_w = svg_el
        .attr_f64("width")
        .or_else(|| {
            svg_el
                .attr("viewBox")
                .and_then(parse_viewbox)
                .map(|vb| vb.width)
        })
        .unwrap_or(100.0);

    let intrinsic_h = svg_el
        .attr_f64("height")
        .or_else(|| {
            svg_el
                .attr("viewBox")
                .and_then(parse_viewbox)
                .map(|vb| vb.height)
        })
        .unwrap_or(100.0);

    let out_w = if dest_width > 0 {
        dest_width
    } else {
        intrinsic_w.ceil() as u32
    };
    let out_h = if dest_height > 0 {
        dest_height
    } else {
        intrinsic_h.ceil() as u32
    };
    if out_w == 0 || out_h == 0 {
        return None;
    }

    let canvas = Canvas::new(out_w, out_h);
    let mut ctx = canvas.get_context("2d")?;

    // Compute the viewBox → canvas transform
    let base_transform = if let Some(vb) = svg_el.attr("viewBox").and_then(parse_viewbox) {
        if vb.width > 0.0 && vb.height > 0.0 {
            // preserveAspectRatio="xMidYMid meet" (default)
            let scale_x = out_w as f64 / vb.width;
            let scale_y = out_h as f64 / vb.height;
            let scale = scale_x.min(scale_y);
            let tx = (out_w as f64 - vb.width * scale) / 2.0 - vb.min_x * scale;
            let ty = (out_h as f64 - vb.height * scale) / 2.0 - vb.min_y * scale;
            Transform::translate(tx, ty).concat(&Transform::scale(scale, scale))
        } else {
            // No viewBox → plain pixel scale from intrinsic dimensions
            let sx = out_w as f64 / intrinsic_w;
            let sy = out_h as f64 / intrinsic_h;
            Transform::scale(sx, sy)
        }
    } else {
        let sx = out_w as f64 / intrinsic_w;
        let sy = out_h as f64 / intrinsic_h;
        Transform::scale(sx, sy)
    };

    let initial_style = SvgStyle::initial();
    render_element(svg_el, &mut ctx, &initial_style, &base_transform);

    Some(canvas.get_image_data())
}

/// Recursively render one SVG element onto `ctx`.
fn render_element(
    el: &XmlElement,
    ctx: &mut Context2D,
    parent_style: &SvgStyle,
    transform: &Transform,
) {
    let style = SvgStyle::inherit_and_apply(parent_style, el);

    // Local transform from this element's "transform" attribute
    let local_tf = el
        .attr("transform")
        .map(Transform::parse)
        .unwrap_or(Transform::identity());
    let transform = transform.concat(&local_tf);

    match el.name.to_ascii_lowercase().as_str() {
        "svg" | "g" | "symbol" => {
            for child in el.child_elements() {
                render_element(child, ctx, &style, &transform);
            }
        }
        "defs" => {
            // Definitions are not rendered directly (gradient defs etc.)
        }
        "rect" => render_rect(el, ctx, &style, &transform),
        "circle" => render_circle(el, ctx, &style, &transform),
        "ellipse" => render_ellipse(el, ctx, &style, &transform),
        "line" => render_line(el, ctx, &style, &transform),
        "polyline" => render_polyline(el, ctx, &style, &transform, false),
        "polygon" => render_polyline(el, ctx, &style, &transform, true),
        "path" => render_path(el, ctx, &style, &transform),
        "use" => {
            // <use> is not yet supported
        }
        _ => {
            // Unknown element – recurse in case it wraps known children
            for child in el.child_elements() {
                render_element(child, ctx, &style, &transform);
            }
        }
    }
}

/// Apply a `Transform` to a list of (x, y) points.
fn transform_points(pts: &[(f64, f64)], tf: &Transform) -> Vec<(f64, f64)> {
    pts.iter().map(|&(x, y)| tf.apply(x, y)).collect()
}

/// Convert flat path commands (with transform applied) to canvas sub-paths.
fn flat_cmds_to_subpaths(cmds: &[FlatCmd], tf: &Transform) -> Vec<Vec<(f64, f64)>> {
    let mut sub_paths: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut current: Vec<(f64, f64)> = Vec::new();
    let mut start: (f64, f64) = (0.0, 0.0);

    for cmd in cmds {
        match *cmd {
            FlatCmd::MoveTo(x, y) => {
                if current.len() >= 2 {
                    sub_paths.push(current.clone());
                }
                current.clear();
                let p = tf.apply(x, y);
                current.push(p);
                start = p;
            }
            FlatCmd::LineTo(x, y) => {
                if current.is_empty() {
                    current.push(start);
                }
                current.push(tf.apply(x, y));
            }
            FlatCmd::ClosePath => {
                if !current.is_empty() {
                    current.push(start);
                    sub_paths.push(current.clone());
                    current.clear();
                }
            }
        }
    }
    if current.len() >= 2 {
        sub_paths.push(current);
    }
    sub_paths
}

/// Apply fill and/or stroke to a set of sub-paths on the context.
fn paint_subpaths(
    sub_paths: &[Vec<(f64, f64)>],
    ctx: &mut Context2D,
    style: &SvgStyle,
    close_for_fill: bool,
) {
    if sub_paths.is_empty() {
        return;
    }

    // Fill
    if let Some(fill_color) = style.effective_fill() {
        ctx.set_fill_style(&color_to_css(fill_color));
        ctx.begin_path();
        for pts in sub_paths {
            if pts.is_empty() {
                continue;
            }
            ctx.move_to(pts[0].0, pts[0].1);
            for &(x, y) in &pts[1..] {
                ctx.line_to(x, y);
            }
            if close_for_fill {
                ctx.close_path();
            }
        }
        ctx.fill();
    }

    // Stroke
    if let Some(stroke_color) = style.effective_stroke() {
        let sw = style.effective_stroke_width();
        ctx.set_stroke_style(&color_to_css(stroke_color));
        ctx.set_line_width(sw);
        if let Some(cap) = &style.stroke_linecap {
            ctx.set_line_cap(cap);
        }
        ctx.begin_path();
        for pts in sub_paths {
            if pts.is_empty() {
                continue;
            }
            ctx.move_to(pts[0].0, pts[0].1);
            for &(x, y) in &pts[1..] {
                ctx.line_to(x, y);
            }
        }
        ctx.stroke();
    }
}

fn color_to_css(c: Color) -> String {
    format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a as f64 / 255.0)
}

fn render_rect(el: &XmlElement, ctx: &mut Context2D, style: &SvgStyle, tf: &Transform) {
    let x = el.attr_f64("x").unwrap_or(0.0);
    let y = el.attr_f64("y").unwrap_or(0.0);
    let w = el.attr_f64("width").unwrap_or(0.0);
    let h = el.attr_f64("height").unwrap_or(0.0);
    if w <= 0.0 || h <= 0.0 {
        return;
    }

    let rx_attr = el.attr_f64("rx");
    let ry_attr = el.attr_f64("ry");
    let rx = rx_attr.or(ry_attr).unwrap_or(0.0).min(w / 2.0);
    let ry = ry_attr.or(rx_attr).unwrap_or(0.0).min(h / 2.0);

    if rx <= 0.0 && ry <= 0.0 {
        // Plain rectangle – build as polygon
        let pts = vec![
            (x, y),
            (x + w, y),
            (x + w, y + h),
            (x, y + h),
            (x, y),
        ];
        let tpts = transform_points(&pts, tf);
        let sub_paths = vec![tpts];
        paint_subpaths(&sub_paths, ctx, style, true);
    } else {
        // Rounded rectangle – approximate with line segments
        let r = rx.min(ry);
        let flat = rounded_rect_flat(x, y, w, h, r);
        let sub_paths = flat_cmds_to_subpaths(&flat, tf);
        paint_subpaths(&sub_paths, ctx, style, true);
    }
}

fn rounded_rect_flat(x: f64, y: f64, w: f64, h: f64, r: f64) -> Vec<FlatCmd> {
    let mut d = String::new();
    d.push_str(&format!("M {},{}", x + r, y));
    d.push_str(&format!(" L {},{}", x + w - r, y));
    d.push_str(&format!(" A {r},{r} 0 0 1 {},{}", x + w, y + r));
    d.push_str(&format!(" L {},{}", x + w, y + h - r));
    d.push_str(&format!(" A {r},{r} 0 0 1 {},{}", x + w - r, y + h));
    d.push_str(&format!(" L {},{}", x + r, y + h));
    d.push_str(&format!(" A {r},{r} 0 0 1 {},{}", x, y + h - r));
    d.push_str(&format!(" L {},{}", x, y + r));
    d.push_str(&format!(" A {r},{r} 0 0 1 {},{}", x + r, y));
    d.push_str(" Z");
    parse_svg_path(&d)
}

fn render_circle(el: &XmlElement, ctx: &mut Context2D, style: &SvgStyle, tf: &Transform) {
    let cx = el.attr_f64("cx").unwrap_or(0.0);
    let cy = el.attr_f64("cy").unwrap_or(0.0);
    let r = el.attr_f64("r").unwrap_or(0.0);
    if r <= 0.0 {
        return;
    }
    let flat = circle_flat(cx, cy, r);
    let sub_paths = flat_cmds_to_subpaths(&flat, tf);
    paint_subpaths(&sub_paths, ctx, style, true);
}

fn circle_flat(cx: f64, cy: f64, r: f64) -> Vec<FlatCmd> {
    let d = format!(
        "M {},{} A {r},{r} 0 1 0 {},{} A {r},{r} 0 1 0 {},{} Z",
        cx + r,
        cy,
        cx - r,
        cy,
        cx + r,
        cy,
    );
    parse_svg_path(&d)
}

fn render_ellipse(el: &XmlElement, ctx: &mut Context2D, style: &SvgStyle, tf: &Transform) {
    let cx = el.attr_f64("cx").unwrap_or(0.0);
    let cy = el.attr_f64("cy").unwrap_or(0.0);
    let rx = el.attr_f64("rx").unwrap_or(0.0);
    let ry = el.attr_f64("ry").unwrap_or(0.0);
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let d = format!(
        "M {},{} A {rx},{ry} 0 1 0 {},{} A {rx},{ry} 0 1 0 {},{} Z",
        cx + rx,
        cy,
        cx - rx,
        cy,
        cx + rx,
        cy,
    );
    let flat = parse_svg_path(&d);
    let sub_paths = flat_cmds_to_subpaths(&flat, tf);
    paint_subpaths(&sub_paths, ctx, style, true);
}

fn render_line(el: &XmlElement, ctx: &mut Context2D, style: &SvgStyle, tf: &Transform) {
    let x1 = el.attr_f64("x1").unwrap_or(0.0);
    let y1 = el.attr_f64("y1").unwrap_or(0.0);
    let x2 = el.attr_f64("x2").unwrap_or(0.0);
    let y2 = el.attr_f64("y2").unwrap_or(0.0);

    let pts = transform_points(&[(x1, y1), (x2, y2)], tf);
    let sub_paths = vec![pts];
    // Lines only get stroked, not filled
    let line_style = SvgStyle {
        fill: Some(None), // no fill for lines
        ..style.clone()
    };
    paint_subpaths(&sub_paths, ctx, &line_style, false);
}

fn render_polyline(
    el: &XmlElement,
    ctx: &mut Context2D,
    style: &SvgStyle,
    tf: &Transform,
    close: bool,
) {
    let pts_str = el.attr("points").unwrap_or("");
    let nums = parse_number_list(pts_str);
    if nums.len() < 2 {
        return;
    }
    let mut pts: Vec<(f64, f64)> = nums.chunks(2).map(|c| (c[0], c[1])).collect();
    if close && !pts.is_empty() {
        pts.push(pts[0]);
    }
    let tpts = transform_points(&pts, tf);
    let sub_paths = vec![tpts];
    paint_subpaths(&sub_paths, ctx, style, close);
}

fn render_path(el: &XmlElement, ctx: &mut Context2D, style: &SvgStyle, tf: &Transform) {
    let d = el.attr("d").unwrap_or("");
    if d.is_empty() {
        return;
    }
    let flat = parse_svg_path(d);
    let sub_paths = flat_cmds_to_subpaths(&flat, tf);
    paint_subpaths(&sub_paths, ctx, style, false);
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Draw an SVG file onto `ctx` at `(dx, dy)` with the given `dw × dh`
/// pixel dimensions.  If `dw` or `dh` is `0` the SVG's intrinsic size is
/// used for that dimension.
///
/// Returns `true` on success, `false` if the SVG could not be decoded.
pub fn draw_svg(
    ctx: &mut Context2D,
    svg_bytes: &[u8],
    dx: f64,
    dy: f64,
    dw: u32,
    dh: u32,
) -> bool {
    let img = match render_svg(svg_bytes, dw, dh) {
        Some(i) => i,
        None => return false,
    };
    ctx.draw_image(&img, dx, dy);
    true
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_xml_simple() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect x="10" y="10" width="80" height="80" fill="red"/>
        </svg>"#;
        let el = parse_xml(svg).unwrap();
        assert_eq!(el.name.to_ascii_lowercase(), "svg");
        assert_eq!(el.attr("width"), Some("100"));
        let rect = el.child_elements().next().unwrap();
        assert_eq!(rect.name.to_ascii_lowercase(), "rect");
        assert_eq!(rect.attr("fill"), Some("red"));
    }

    #[test]
    fn test_transform_identity() {
        let t = Transform::identity();
        let (x, y) = t.apply(3.0, 4.0);
        assert!((x - 3.0).abs() < 1e-9);
        assert!((y - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_transform_translate() {
        let t = Transform::parse("translate(10, 20)");
        let (x, y) = t.apply(0.0, 0.0);
        assert!((x - 10.0).abs() < 1e-9);
        assert!((y - 20.0).abs() < 1e-9);
    }

    #[test]
    fn test_transform_scale() {
        let t = Transform::parse("scale(2)");
        let (x, y) = t.apply(3.0, 4.0);
        assert!((x - 6.0).abs() < 1e-9);
        assert!((y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn test_parse_svg_path_moveto_lineto() {
        let cmds = parse_svg_path("M 10 20 L 30 40 Z");
        assert!(matches!(cmds[0], FlatCmd::MoveTo(x, y) if (x-10.0).abs()<1e-9 && (y-20.0).abs()<1e-9));
        assert!(matches!(cmds[1], FlatCmd::LineTo(x, y) if (x-30.0).abs()<1e-9 && (y-40.0).abs()<1e-9));
        assert!(matches!(cmds[2], FlatCmd::ClosePath));
    }

    #[test]
    fn test_render_svg_returns_image_data() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
            <rect x="0" y="0" width="50" height="50" fill="blue"/>
        </svg>"#;
        let img = render_svg(svg.as_bytes(), 50, 50).unwrap();
        assert_eq!(img.width, 50);
        assert_eq!(img.height, 50);
        // The top-left pixel should be blue (roughly)
        let px = img.get_pixel(25, 25);
        assert!(px.b > 200, "Expected blue pixel, got {:?}", px);
    }

    #[test]
    fn test_render_svg_circle() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <circle cx="50" cy="50" r="40" fill="red"/>
        </svg>"#;
        let img = render_svg(svg.as_bytes(), 100, 100).unwrap();
        let center = img.get_pixel(50, 50);
        assert!(center.r > 200, "Center should be red, got {:?}", center);
    }

    #[test]
    fn test_render_svg_with_viewbox() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
            <rect x="0" y="0" width="100" height="100" fill="green"/>
        </svg>"#;
        let img = render_svg(svg.as_bytes(), 200, 200).unwrap();
        assert_eq!(img.width, 200);
        assert_eq!(img.height, 200);
        let px = img.get_pixel(100, 100);
        assert!(px.g > 100, "Expected green pixel, got {:?}", px);
    }

    #[test]
    fn test_xml_entities() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <rect fill="&lt;test&gt;" width="10" height="10"/>
        </svg>"#;
        let el = parse_xml(svg).unwrap();
        let rect = el.child_elements().next().unwrap();
        assert_eq!(rect.attr("fill"), Some("<test>"));
    }

    #[test]
    fn test_svg_path_horizontal_vertical() {
        let cmds = parse_svg_path("M 0 0 H 10 V 10 Z");
        let has_lineto = cmds.iter().any(|c| matches!(c, FlatCmd::LineTo(_, _)));
        assert!(has_lineto);
    }

    #[test]
    fn test_svg_path_relative() {
        let cmds = parse_svg_path("M 10 10 l 10 0 l 0 10 z");
        // Should have a MoveTo, two LineTos, and a ClosePath
        assert!(matches!(cmds[0], FlatCmd::MoveTo(10.0, 10.0)));
        assert!(matches!(cmds[1], FlatCmd::LineTo(x, _) if (x - 20.0).abs() < 1e-9));
    }
}
