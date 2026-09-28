//! Gradients: linear (by `from`/`to` points or a CSS `angle`), radial and
//! conic, with stops as `{at, color}` or plain colors spread evenly.

use serde::{Deserialize, Serialize};

use super::Color;
use super::defaults::{center, is_center, is_default, left_mid, right_mid};

/// A gradient. Points are in the box, 0–1 on each axis, like a design
/// tool's gradient handles: `from [0.5, 0]` → `to [0.5, 1]` runs top to bottom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Gradient {
    /// `linear` (default), `radial` or `conic`.
    #[serde(default, rename = "type", skip_serializing_if = "is_default")]
    pub kind: GradientKind,
    /// Linear: start point (left middle).
    #[serde(default = "left_mid")]
    pub from: [f32; 2],
    /// Linear: end point (right middle).
    #[serde(default = "right_mid")]
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
    /// Color stops, at least two: `{at, color}`, or colors spread evenly.
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
    /// Position along the gradient, 0–1.
    pub at: f32,
    /// Color at that position.
    pub color: Color,
}

/// Stops as objects (`at`, or CSS's `offset`/`position`, 0–1 or "40%"),
/// or as bare colors placed evenly from 0 to 1.
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
                    .find(|k| !["at", "offset", "position", "pos", "color"].contains(&k.as_str()))
                {
                    return Err(D::Error::custom(format!(
                        "stop {i}: unknown field {k}; use {{at, color}}"
                    )));
                }
                let color = o
                    .get("color")
                    .and_then(|c| c.as_str())
                    .and_then(Color::parse)
                    .ok_or_else(|| D::Error::custom(format!("stop {i}: needs a color")))?;
                let at = match ["at", "offset", "position", "pos"]
                    .iter()
                    .find_map(|k| o.get(*k))
                {
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
            other => Err(super::de::expected("a color or {at, color}", other)),
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

#[cfg(test)]
mod tests {
    use super::{Gradient, GradientKind};
    use serde_json::json;

    #[test]
    fn stops_take_css_names_and_percentages() {
        let g: Gradient = serde_json::from_value(json!({"stops": [
            {"offset": 0, "color": "#FFFFFF"}, {"position": "55%", "color": "#FFFFFFEE"}, {"at": 1, "color": "#FFFFFF00"}]}))
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
                .contains("stop 0: unknown field stop; use {at, color}"),
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
        let v = json!({"from": [0.0, 0.0], "to": [1.0, 1.0], "stops": [{"at": 0.0, "color": "#000000"}, {"at": 1.0, "color": "#FFFFFF"}]});
        let g: Gradient = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(&g).unwrap(), v);
    }
}
