//! Gradients: linear (by `from`/`to` points or a CSS `angle`), radial and
//! conic, with stops as `{at, color}` or plain colors spread evenly.

use serde::{Deserialize, Serialize};

use super::Color;
use super::defaults::{bottom_mid, center, is_center, is_default, top_mid};

/// A gradient. Points are in the box, 0–1 on each axis, like a design
/// tool's gradient handles: `from [0.5, 0]` → `to [0.5, 1]` runs top to bottom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Gradient {
    /// `linear` (default), `radial` or `conic`.
    #[serde(default, rename = "type", skip_serializing_if = "is_default")]
    pub kind: GradientKind,
    /// Linear: start point (top middle, as CSS runs top to bottom).
    #[serde(default = "top_mid")]
    pub from: [f32; 2],
    /// Linear: end point (bottom middle).
    #[serde(default = "bottom_mid")]
    pub to: [f32; 2],
    /// Linear: direction in degrees as in CSS (0 = to top, 90 = to right);
    /// wins over `from`/`to`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f32>,
    /// Radial and conic: center ([0.5, 0.5]).
    #[serde(default = "center", skip_serializing_if = "is_center")]
    pub center: [f32; 2],
    /// Radial: radius as a share of the box's width and height ([0.5, 0.5]:
    /// an ellipse touching the box's edges).
    #[serde(default = "center", skip_serializing_if = "is_center")]
    pub radius: [f32; 2],
    /// Color stops, at least two: `{offset, color}`, or colors spread evenly.
    #[serde(deserialize_with = "stops")]
    pub stops: Vec<Stop>,
}

/// The shape of a gradient.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GradientKind {
    /// Along a line.
    #[default]
    Linear,
    /// Outward from the center.
    Radial,
    /// Around the center, clockwise from the top.
    Conic,
}

/// One color stop of a gradient.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stop {
    /// Position along the gradient, 0–1 (SVG `offset`).
    #[serde(rename = "offset")]
    pub at: f32,
    /// Color at that position.
    pub color: Color,
}

/// Stops as objects (`offset`, 0–1 or "40%"), or as bare colors placed
/// evenly from 0 to 1.
fn stops<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Stop>, D::Error> {
    use serde::de::Error;
    let raw = Vec::<serde_json::Value>::deserialize(d)?;
    let n = raw.len();
    let even = |i: usize| {
        if n > 1 {
            i as f32 / (n - 1) as f32
        } else {
            0.0
        }
    };
    raw.into_iter()
        .enumerate()
        .map(|(i, v)| match &v {
            serde_json::Value::String(s) => Color::parse(s)
                .map(|color| Stop { at: even(i), color })
                .ok_or_else(|| D::Error::custom(format!("stop {i}: bad color {s}"))),
            serde_json::Value::Object(o) => {
                if let Some(k) = o
                    .keys()
                    .find(|k| !["offset", "color"].contains(&k.as_str()))
                {
                    return Err(D::Error::custom(format!(
                        "stop {i}: unknown field {k}; use {{offset, color}}"
                    )));
                }
                let color = o
                    .get("color")
                    .and_then(|c| c.as_str())
                    .and_then(Color::parse)
                    .ok_or_else(|| D::Error::custom(format!("stop {i}: needs a color")))?;
                let at = match o.get("offset") {
                    None => even(i),
                    Some(p) => super::de::float(p)
                        .or_else(|| {
                            p.as_str()
                                .and_then(|s| s.strip_suffix('%'))
                                .and_then(|s| s.trim().parse::<f32>().ok())
                                .map(|pc| pc / 100.0)
                        })
                        .ok_or_else(|| super::de::expected("a stop position 0–1", p))?,
                };
                Ok(Stop { at, color })
            }
            other => Err(super::de::expected("a color or {offset, color}", other)),
        })
        .collect()
}

impl Gradient {
    /// Linear start and end points, 0–1 in the box: `angle` when set (CSS:
    /// the line through the center, long enough to reach the corners of a
    /// square), else `from` and `to`.
    pub fn line(&self) -> ([f32; 2], [f32; 2]) {
        match self.angle {
            Some(deg) => {
                let (s, c) = deg.to_radians().sin_cos();
                // Half-length reaching the corners, as CSS does for a square box.
                let h = (s.abs() + c.abs()) / 2.0;
                ([0.5 - s * h, 0.5 + c * h], [0.5 + s * h, 0.5 - c * h])
            }
            None => (self.from, self.to),
        }
    }
}

/// A CSS `linear-gradient(…)` or `radial-gradient(…)` as the gradient's
/// fields: every recreate run wrote its fade this way. `None` for other
/// strings. A radial's shape and position are dropped (it's centered).
pub(super) fn from_css(s: &str) -> Option<serde_json::Value> {
    let s = s.trim();
    let (kind, inner) = [
        ("linear", "linear-gradient("),
        ("radial", "radial-gradient("),
    ]
    .into_iter()
    .find_map(|(k, p)| Some((k, s.strip_prefix(p)?.strip_suffix(')')?)))?;
    // Split at the commas outside `rgba(…)`.
    let (mut parts, mut depth, mut start) = (Vec::new(), 0_i32, 0);
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(inner[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(inner[start..].trim());
    let mut angle = None;
    let first = parts.first()?;
    if kind == "linear" {
        angle = first
            .strip_suffix("deg")
            .and_then(|a| a.trim().parse::<f32>().ok());
        if angle.is_none() {
            angle = first.strip_prefix("to ").map(|to| {
                let has = |w: &str| to.split_whitespace().any(|t| t == w);
                let (x, y) = (
                    if has("right") {
                        1.0
                    } else if has("left") {
                        -1.0
                    } else {
                        0.0
                    },
                    if has("bottom") {
                        1.0
                    } else if has("top") {
                        -1.0
                    } else {
                        0.0
                    },
                );
                // CSS angles: 0 up, clockwise.
                f32::atan2(x, -y).to_degrees().rem_euclid(360.0)
            });
        }
    }
    let skip = usize::from(
        angle.is_some()
            || (kind == "radial" && Color::parse(first).is_none() && !first.contains('(')),
    );
    let stops: Vec<serde_json::Value> = parts[skip..]
        .iter()
        .map(|p| match p.rsplit_once(' ') {
            Some((c, at)) if at.ends_with('%') => {
                serde_json::json!({"color": c.trim(), "offset": at})
            }
            _ => serde_json::json!(p),
        })
        .collect();
    let mut g = serde_json::json!({"type": kind, "stops": stops});
    if let Some(a) = angle {
        g["angle"] = a.into();
    }
    if kind == "radial" {
        let (center, radius) = radial_head(if skip == 1 { first } else { "" })?;
        g["center"] = serde_json::json!(center);
        g["radius"] = serde_json::json!(radius);
    }
    Some(g)
}

/// A CSS radial gradient's `[shape] [size] [at position]`, as the box
/// fractions `center` and `radius`. CSS's default is an ellipse reaching the
/// farthest corner from the center. `None` for what it can't read (lengths
/// in px, which need the box's size).
// ponytail: `circle` is drawn as an ellipse of the box's shape, since radii
// are fractions of each axis; a true circle needs the long form's radius.
fn radial_head(head: &str) -> Option<([f32; 2], [f32; 2])> {
    let (size, at) = match head.split_once("at ") {
        Some((s, p)) => (s, p),
        None => (head, ""),
    };
    let pct = |t: &str| {
        t.strip_suffix('%')?
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| v / 100.0)
    };
    // Position: percentages in x, y order, or keywords in any order.
    let mut center = [0.5_f32, 0.5];
    let mut axis = 0;
    for t in at.split_whitespace() {
        match t {
            "left" | "right" => {
                center[0] = if t == "left" { 0.0 } else { 1.0 };
                axis = 1;
            }
            "top" => center[1] = 0.0,
            "bottom" => center[1] = 1.0,
            "center" => axis += 1,
            _ => {
                *center.get_mut(axis)? = pct(t)?;
                axis += 1;
            }
        }
    }
    let side = |far: bool| {
        let pick = |c: f32| if far { c.max(1.0 - c) } else { c.min(1.0 - c) };
        [pick(center[0]), pick(center[1])]
    };
    let corner = |far: bool| side(far).map(|r| r * std::f32::consts::SQRT_2);
    let mut radius = corner(true);
    let mut given = Vec::new();
    for t in size.split_whitespace() {
        match t {
            "circle" | "ellipse" => {}
            "closest-side" => radius = side(false),
            "farthest-side" => radius = side(true),
            "closest-corner" => radius = corner(false),
            "farthest-corner" => radius = corner(true),
            _ => given.push(pct(t)?),
        }
    }
    match given[..] {
        [] => {}
        [r] => radius = [r, r],
        [x, y] => radius = [x, y],
        _ => return None,
    }
    Some((center, radius))
}

#[cfg(test)]
mod tests {
    use super::{Gradient, GradientKind, from_css};
    use serde_json::json;

    fn radial(css: &str) -> ([f32; 2], [f32; 2]) {
        let g: Gradient = serde_json::from_value(from_css(css).unwrap()).unwrap();
        assert_eq!(g.kind, GradientKind::Radial);
        (g.center, g.radius)
    }

    #[test]
    fn a_css_radial_keeps_its_size_and_position() {
        // A corner glow, as agents write it in CSS.
        assert_eq!(
            radial("radial-gradient(60% 55% at 92% 4%, rgba(124,92,255,0.3) 0%, transparent 70%)"),
            ([0.92, 0.04], [0.6, 0.55])
        );
        assert_eq!(
            radial("radial-gradient(circle at top right, #fff, #000)").0,
            [1.0, 0.0]
        );
        assert_eq!(
            radial("radial-gradient(at left 30%, #fff, #000)").0,
            [0.0, 0.3]
        );
        // Sizes by keyword, measured from the center.
        assert_eq!(
            radial("radial-gradient(closest-side at 20% 50%, #fff, #000)"),
            ([0.2, 0.5], [0.2, 0.5])
        );
        assert_eq!(
            radial("radial-gradient(farthest-side at 20% 50%, #fff, #000)").1,
            [0.8, 0.5]
        );
        // CSS's default reaches the farthest corner.
        let (c, r) = radial("radial-gradient(#fff, #000)");
        assert_eq!(c, [0.5, 0.5]);
        assert!(
            (r[0] - 0.5 * std::f32::consts::SQRT_2).abs() < 1e-6,
            "{r:?}"
        );
        // A length in px needs the box: not read, so the caller's error shows.
        assert!(from_css("radial-gradient(200px at 50% 50%, #fff, #000)").is_none());
    }

    #[test]
    fn stops_take_offsets_and_percentages() {
        let g: Gradient = serde_json::from_value(json!({"stops": [
            {"offset": 0, "color": "#FFFFFF"}, {"offset": "55%", "color": "#FFFFFFEE"}, {"offset": 1, "color": "#FFFFFF00"}]}))
        .unwrap();
        assert_eq!(
            g.stops.iter().map(|s| s.at).collect::<Vec<_>>(),
            [0.0, 0.55, 1.0]
        );
        let e = serde_json::from_value::<Gradient>(
            json!({"stops": [{"stop": 0, "color": "#000"}, "#FFF"]}),
        )
        .unwrap_err();
        assert!(
            e.to_string()
                .contains("stop 0: unknown field stop; use {offset, color}"),
            "{e}"
        );
    }

    #[test]
    fn stops_may_be_bare_colors_spread_evenly() {
        let g: Gradient =
            serde_json::from_value(json!({"stops": ["#000000", "#FF0000", "#FFFFFF"]})).unwrap();
        assert_eq!(
            g.stops.iter().map(|s| s.at).collect::<Vec<_>>(),
            [0.0, 0.5, 1.0]
        );
        assert_eq!(g.kind, GradientKind::Linear);
    }

    #[test]
    fn css_angles_become_points() {
        let g = |a: f32| {
            serde_json::from_value::<Gradient>(json!({"angle": a, "stops": ["#000", "#FFF"]}))
                .unwrap()
                .line()
        };
        let close = |(a, b): ([f32; 2], [f32; 2]), (c, d): ([f32; 2], [f32; 2])| {
            a.iter()
                .chain(&b)
                .zip(c.iter().chain(&d))
                .all(|(x, y)| (x - y).abs() < 1e-5)
        };
        assert!(
            close(g(180.0), ([0.5, 0.0], [0.5, 1.0])),
            "180° runs top to bottom"
        );
        assert!(
            close(g(90.0), ([0.0, 0.5], [1.0, 0.5])),
            "90° runs left to right"
        );
        assert!(
            close(g(135.0), ([0.0, 0.0], [1.0, 1.0])),
            "135° runs corner to corner"
        );
    }

    #[test]
    fn short_form_gradients_round_trip_unchanged() {
        let v = json!({"from": [0.0, 0.0], "to": [1.0, 1.0], "stops": [{"offset": 0.0, "color": "#000000"}, {"offset": 1.0, "color": "#FFFFFF"}]});
        let g: Gradient = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(&g).unwrap(), v);
    }
}
