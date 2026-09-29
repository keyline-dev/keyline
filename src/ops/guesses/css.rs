//! CSS value syntax, read into the scene's own shapes: `"24px"`,
//! `"12px 24px"`, box-shadow and border strings, and CSS gradient functions.

use serde_json::{Map, Value, json};

/// `"24px"` (or `"24"`) as 24.
pub(super) fn px(s: &str) -> Option<f64> {
    let s = s.trim();
    s.strip_suffix("px").unwrap_or(s).trim().parse().ok()
}

/// Values in `"12px 24px"` form: each as px.
fn px_list(s: &str) -> Option<Vec<f64>> {
    s.split_whitespace().map(px).collect()
}

/// A px string, or a CSS shorthand of several (`"12px 24px"`, three values
/// meaning top, sides, bottom), as the number or array the scene takes.
pub(super) fn px_value(v: &Value) -> Option<Value> {
    let s = v.as_str()?;
    let n = px_list(s)?;
    Some(match n.as_slice() {
        [one] => json!(one),
        [t, h, b] => json!([t, h, b, h]),
        many => json!(many),
    })
}

/// Splits on `sep` outside parentheses: the commas between gradient
/// stops, not the ones inside `rgba(…)`.
fn split_top(s: &str, sep: char) -> Vec<&str> {
    let (mut depth, mut start, mut out) = (0i32, 0, Vec::new());
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c == sep && depth == 0 => {
                out.push(s[start..i].trim());
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out.into_iter().filter(|p| !p.is_empty()).collect()
}

/// A CSS `box-shadow` or `text-shadow` list:
/// `"0 4px 12px rgba(0,0,0,.25), inset 0 0 0 1px #fff"`.
pub(super) fn shadows(s: &str) -> Option<Value> {
    let list: Option<Vec<Value>> = split_top(s, ',').into_iter().map(shadow).collect();
    let mut list = list?;
    Some(if list.len() == 1 {
        list.remove(0)
    } else {
        Value::Array(list)
    })
}

fn shadow(s: &str) -> Option<Value> {
    let mut o = Map::new();
    let mut lengths = Vec::new();
    for part in split_top(s, ' ') {
        if part == "inset" {
            o.insert("inset".into(), true.into());
        } else if let Some(n) = px(part) {
            lengths.push(n);
        } else {
            o.insert("color".into(), part.into());
        }
    }
    if !(2..=4).contains(&lengths.len()) {
        return None;
    }
    for (k, n) in ["x", "y", "blur", "spread"].iter().zip(lengths) {
        o.insert((*k).into(), n.into());
    }
    o.entry("color").or_insert_with(|| "#000000".into());
    Some(Value::Object(o))
}

/// A CSS `border` string, `"2px solid #000"`, as a stroke.
pub(super) fn border(s: &str) -> Option<Value> {
    let mut o = Map::new();
    let mut width = 1.0;
    for part in split_top(s, ' ') {
        match part {
            "solid" | "none" => {}
            "dashed" => {
                o.insert("dash".into(), "dashed".into());
            }
            "dotted" => {
                o.insert("dash".into(), "dotted".into());
            }
            p => match px(p) {
                Some(n) => width = n,
                None => {
                    o.insert("color".into(), p.into());
                }
            },
        }
    }
    // Dash lengths follow the width, as a browser draws them.
    if let Some(style) = o.remove("dash") {
        let dash = if style == "dotted" {
            [width, width]
        } else {
            [width * 3.0, width * 2.0]
        };
        o.insert("dash".into(), json!(dash));
    }
    o.insert("width".into(), width.into());
    Some(Value::Object(o))
}

/// A CSS gradient function as a gradient paint: `linear-gradient(180deg,
/// #000 0%, #fff 100%)`, `linear-gradient(to right, red, blue)`,
/// `radial-gradient(circle, …)`, `conic-gradient(from 90deg, …)`.
pub(super) fn gradient(s: &str) -> Option<Value> {
    let s = s.trim();
    let (kind, args) = s.strip_suffix(')')?.split_once('(')?;
    let kind = kind
        .trim()
        .trim_start_matches("repeating-")
        .strip_suffix("-gradient")?;
    let mut parts = split_top(args, ',');
    let mut g = Map::new();
    g.insert("type".into(), kind.into());
    // A first part that isn't a color is the direction or shape.
    if let Some(first) = parts.first().copied()
        && crate::scene::Color::parse(first.split_whitespace().next().unwrap_or("")).is_none()
    {
        parts.remove(0);
        if let Some(angle) = direction(first) {
            g.insert("angle".into(), angle.into());
        }
    }
    let stops: Vec<Value> = parts
        .iter()
        .map(|p| {
            let at = split_top(p, ' ');
            match at.as_slice() {
                [color, pos] if pos.ends_with('%') => {
                    let pct: f64 = pos.trim_end_matches('%').parse().unwrap_or(0.0);
                    json!({"at": pct / 100.0, "color": color})
                }
                _ => json!(p),
            }
        })
        .collect();
    if stops.len() < 2 {
        return None;
    }
    g.insert("stops".into(), Value::Array(stops));
    Some(json!({ "gradient": g }))
}

/// A linear gradient's `180deg`, `0.25turn` or `to bottom right`, or a
/// conic one's `from 90deg`, as degrees (0 = up, clockwise).
fn direction(s: &str) -> Option<f64> {
    let s = s.trim().trim_start_matches("from ").trim();
    if let Some(d) = s.strip_suffix("deg") {
        return d.trim().parse().ok();
    }
    if let Some(t) = s.strip_suffix("turn") {
        return t.trim().parse::<f64>().ok().map(|t| t * 360.0);
    }
    let to = s.strip_prefix("to ")?;
    let has = |w: &str| to.split_whitespace().any(|x| x == w);
    let (x, y) = (
        f64::from(u8::from(has("right"))) - f64::from(u8::from(has("left"))),
        f64::from(u8::from(has("bottom"))) - f64::from(u8::from(has("top"))),
    );
    Some(x.atan2(-y).to_degrees().rem_euclid(360.0))
}

/// The first family of a CSS font stack: `"'Playfair Display', serif"`.
pub(super) fn first_family(s: &str) -> Option<String> {
    let first = s.split(',').next()?.trim().trim_matches(['"', '\'']);
    (s.contains(',') || first.len() != s.len()).then(|| first.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{border, first_family, gradient, px_value, shadows};
    use serde_json::json;

    #[test]
    fn css_lengths_shadows_and_borders_read_as_scene_values() {
        assert_eq!(px_value(&json!("24px")), Some(json!(24.0)));
        assert_eq!(
            px_value(&json!("8px 16px 4px")),
            Some(json!([8.0, 16.0, 4.0, 16.0]))
        );
        assert_eq!(px_value(&json!("wide")), None);
        assert_eq!(
            shadows("0 4px 12px rgba(0, 0, 0, 0.25)"),
            Some(json!({"x": 0.0, "y": 4.0, "blur": 12.0, "color": "rgba(0, 0, 0, 0.25)"}))
        );
        assert_eq!(
            shadows("inset 0 0 0 1px #fff, 0 1px 2px black"),
            Some(json!([
                {"inset": true, "x": 0.0, "y": 0.0, "blur": 0.0, "spread": 1.0, "color": "#fff"},
                {"x": 0.0, "y": 1.0, "blur": 2.0, "color": "black"}]))
        );
        assert_eq!(
            border("2px dashed #D0202E"),
            Some(json!({"width": 2.0, "color": "#D0202E", "dash": [6.0, 4.0]}))
        );
        assert_eq!(
            first_family("'Playfair Display', serif"),
            Some("Playfair Display".into())
        );
        assert_eq!(first_family("Inter"), None);
    }

    #[test]
    fn css_gradient_functions_read_as_gradients() {
        assert_eq!(
            gradient("linear-gradient(180deg, #000 0%, rgba(0,0,0,0) 100%)"),
            Some(json!({"gradient": {"type": "linear", "angle": 180.0,
                "stops": [{"at": 0.0, "color": "#000"}, {"at": 1.0, "color": "rgba(0,0,0,0)"}]}}))
        );
        let g = gradient("linear-gradient(to bottom right, red, blue)").unwrap();
        assert_eq!(g["gradient"]["angle"], json!(135.0));
        assert_eq!(g["gradient"]["stops"], json!(["red", "blue"]));
        let g = gradient("radial-gradient(circle, #fff, #000)").unwrap();
        assert_eq!(g["gradient"]["type"], "radial");
        assert!(g["gradient"].get("angle").is_none());
        assert_eq!(gradient("linear-gradient(red)"), None);
    }
}
