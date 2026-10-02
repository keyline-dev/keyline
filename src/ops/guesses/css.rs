//! CSS written where the scene has its own form: shorthand strings
//! (`"padding": "54px 48px"`, a `box-shadow`, a `border`), the four
//! `padding*` sides, and gradient stops by `position`. Each one, refused,
//! cost a benchmark agent a resend of its whole design.

use serde_json::{Map, Value, json};

/// The fields of a layer, a style or a `media` change.
pub(super) fn fields(o: &mut Map<String, Value>) {
    padding(o);
    margin(o);
    // `place` in CSS's words: `bottom-center`, `center-left`, `middle`.
    if let Some(Value::String(p)) = o.get_mut("place") {
        let words: Vec<&str> = p.split(['-', ' ']).collect();
        let v = words.iter().find(|w| matches!(**w, "top" | "bottom"));
        let h = words.iter().find(|w| matches!(**w, "left" | "right"));
        let known = words.iter().all(|w| {
            matches!(
                *w,
                "top" | "bottom" | "left" | "right" | "center" | "centre" | "middle"
            )
        });
        if known {
            *p = match (v, h) {
                (Some(v), Some(h)) => format!("{v}-{h}"),
                (Some(v), None) => (*v).to_owned(),
                (None, Some(h)) => (*h).to_owned(),
                (None, None) => "center".to_owned(),
            };
        }
    }
    for k in ["gap", "borderRadius", "fontSize"] {
        if let Some(n) = o.get(k).and_then(Value::as_str).and_then(px) {
            o.insert(k.into(), n.into());
        }
    }
    if let Some(s) = o.get("shadow").and_then(Value::as_str).and_then(shadows) {
        o.insert("shadow".into(), s);
    }
    // CSS `border`, or a stroke written like one: `"4px solid #7C5CFF"`.
    if !o.contains_key("stroke")
        && let Some(b) = o.remove("border")
    {
        o.insert("stroke".into(), b);
    }
    if let Some(s) = o.get("stroke").and_then(Value::as_str).and_then(border) {
        o.insert("stroke".into(), s);
    }
    for k in ["fill", "stroke", "mask", "highlight"] {
        if let Some(v) = o.get_mut(k) {
            stops(v);
        }
    }
}

/// `"12px"` or `"12"` as 12.
fn px(s: &str) -> Option<f64> {
    s.trim().trim_end_matches("px").trim().parse().ok()
}

/// CSS's one to four values, `"54px 48px 38px"`, as `[top, right,
/// bottom, left]`; and `paddingTop` and its sides, over any `padding`.
fn padding(o: &mut Map<String, Value>) {
    if let Some(s) = o.get("padding").and_then(Value::as_str) {
        let v: Option<Vec<f64>> = s.split_whitespace().map(px).collect();
        let sides = match v.as_deref() {
            Some(&[a]) => Some(json!(a)),
            Some(&[a, b]) => Some(json!([a, b])),
            Some(&[t, x, b]) => Some(json!([t, x, b, x])),
            Some(&[t, r, b, l]) => Some(json!([t, r, b, l])),
            _ => None,
        };
        if let Some(p) = sides {
            o.insert("padding".into(), p);
        }
    }
    let names = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];
    if !names.iter().any(|k| o.contains_key(*k)) {
        return;
    }
    // Start from the padding given, as four sides.
    let mut sides = match o.get("padding") {
        Some(Value::Number(n)) => [n.as_f64().unwrap_or(0.0); 4],
        Some(Value::Array(a)) => {
            let n: Vec<f64> = a.iter().filter_map(Value::as_f64).collect();
            match n[..] {
                [v, h] => [v, h, v, h],
                [t, r, b, l] => [t, r, b, l],
                _ => [0.0; 4],
            }
        }
        _ => [0.0; 4],
    };
    for (i, k) in names.iter().enumerate() {
        if let Some(v) = o.remove(*k)
            && let Some(n) = v.as_f64().or_else(|| v.as_str().and_then(px))
        {
            sides[i] = n;
        }
    }
    o.insert("padding".into(), json!(sides));
}

/// CSS's margin, `[top, right, bottom, left]` (or one to four values in a
/// string), on a placed layer: the scene's `[x, y]`, the sides facing its
/// `place` edges (left unless placed right; top unless placed bottom).
fn margin(o: &mut Map<String, Value>) {
    let Some(place) = o.get("place").and_then(Value::as_str).map(str::to_owned) else {
        return;
    };
    let sides: Option<Vec<f64>> = match o.get("margin") {
        Some(Value::Array(a)) if a.len() > 2 => a.iter().map(Value::as_f64).collect(),
        Some(Value::String(s)) => s.split_whitespace().map(px).collect(),
        _ => return,
    };
    let [t, r, b, l] = match sides.as_deref() {
        Some(&[a]) => [a; 4],
        Some(&[v, h]) => [v, h, v, h],
        Some(&[t, h, b]) => [t, h, b, h],
        Some(&[t, r, b, l]) => [t, r, b, l],
        _ => return,
    };
    let x = if place.ends_with("right") || place == "right" {
        r
    } else {
        l
    };
    let y = if place.starts_with("bottom") { b } else { t };
    o.insert("margin".into(), json!([x, y]));
}

/// Splits at the commas outside parentheses, as in `rgba(…)`.
fn split_top(s: &str, sep: char) -> Vec<&str> {
    let (mut out, mut depth, mut start) = (Vec::new(), 0_i32, 0);
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c == sep && depth == 0 => {
                out.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// A CSS `box-shadow`: `"0 12px 32px rgba(…)"`, `inset` and several
/// comma-separated, as shadow objects. `None` when it isn't one.
fn shadows(s: &str) -> Option<Value> {
    let list: Option<Vec<Value>> = split_top(s, ',')
        .into_iter()
        .map(|one| {
            let (mut nums, mut color, mut inset) = (Vec::new(), None, false);
            for t in split_top(one.trim(), ' ')
                .into_iter()
                .filter(|t| !t.is_empty())
            {
                if t == "inset" {
                    inset = true;
                } else if let Some(n) = px(t) {
                    nums.push(n);
                } else {
                    color = Some(t.to_owned());
                }
            }
            let (x, y) = (*nums.first()?, *nums.get(1)?);
            let mut m =
                json!({"x": x, "y": y, "color": color.unwrap_or_else(|| "#00000040".into())});
            if let Some(b) = nums.get(2) {
                m["blur"] = json!(b);
            }
            if let Some(sp) = nums.get(3) {
                m["spread"] = json!(sp);
            }
            if inset {
                m["inset"] = json!(true);
            }
            Some(m)
        })
        .collect();
    let mut list = list?;
    Some(if list.len() == 1 {
        list.remove(0)
    } else {
        Value::Array(list)
    })
}

/// A CSS `border`, `"4px solid #7C5CFF"`, as a stroke. `None` for a plain
/// color, which a stroke already takes.
fn border(s: &str) -> Option<Value> {
    let parts = split_top(s.trim(), ' ');
    let width = px(parts.first()?)?;
    let mut m = json!({"width": width});
    for t in parts.iter().skip(1).filter(|t| !t.is_empty()) {
        match *t {
            "solid" => {}
            "dashed" => m["dash"] = json!([width * 3.0, width * 2.0]),
            "dotted" => m["dash"] = json!([width, width]),
            c => m["color"] = json!(c),
        }
    }
    Some(m)
}

/// Gradient stops written `{position, color}` (as in many design tools),
/// anywhere in a paint.
fn stops(v: &mut Value) {
    match v {
        Value::Object(m) => {
            // A gradient's own fields written beside it: `{gradient: {…}, angle}`.
            if m.get("gradient").is_some_and(Value::is_object) {
                for k in ["angle", "type", "from", "to", "center", "radius"] {
                    if let Some(v) = m.remove(k)
                        && let Some(Value::Object(g)) = m.get_mut("gradient")
                    {
                        g.entry(k).or_insert(v);
                    }
                }
            }
            if let Some(Value::Array(list)) = m.get_mut("stops") {
                // `[offset, color]` pairs.
                for stop in list.iter_mut() {
                    if let Value::Array(pair) = stop
                        && let [o, c] = &pair[..]
                        && o.is_number()
                        && c.is_string()
                    {
                        *stop = json!({"offset": o, "color": c});
                    }
                }
                for stop in list.iter_mut().filter_map(Value::as_object_mut) {
                    for from in ["position", "at"] {
                        if !stop.contains_key("offset")
                            && let Some(p) = stop.remove(from)
                        {
                            stop.insert("offset".into(), p);
                        }
                    }
                }
                // `type: "linear-gradient"`, the CSS function's name.
                if let Some(Value::String(t)) = m.get_mut("type")
                    && let Some(kind) = t.strip_suffix("-gradient")
                {
                    *t = kind.to_owned();
                }
            }
            m.values_mut().for_each(stops);
        }
        Value::Array(list) => list.iter_mut().for_each(stops),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::fields;
    use serde_json::{Value, json};

    fn read(v: Value) -> Value {
        let mut v = v;
        fields(v.as_object_mut().unwrap());
        v
    }

    #[test]
    fn css_shorthands_read_as_the_scene_writes_them() {
        // The benchmark agents' own slips.
        assert_eq!(
            read(json!({"padding": "54px 48px 38px 48px"})),
            json!({"padding": [54.0, 48.0, 38.0, 48.0]})
        );
        assert_eq!(
            read(json!({"padding": "64px 72px 48px"})),
            json!({"padding": [64.0, 72.0, 48.0, 72.0]})
        );
        assert_eq!(
            read(json!({"padding": "19px 38px"})),
            json!({"padding": [19.0, 38.0]})
        );
        assert_eq!(
            read(json!({"padding": [10, 20], "paddingTop": 4, "paddingLeft": "6px"})),
            json!({"padding": [4.0, 20.0, 10.0, 6.0]})
        );
        assert_eq!(
            read(json!({"gap": "12px", "borderRadius": "999px"})),
            json!({"gap": 12.0, "borderRadius": 999.0})
        );
        assert_eq!(
            read(json!({"shadow": "0 12px 32px rgba(124,92,255,0.35)"})),
            json!({"shadow": {"x": 0.0, "y": 12.0, "blur": 32.0, "color": "rgba(124,92,255,0.35)"}})
        );
        assert_eq!(
            read(json!({"shadow": "inset 0 1px 0 #fff3, 0 4px 8px 2px #0004"})),
            json!({"shadow": [
                {"x": 0.0, "y": 1.0, "blur": 0.0, "color": "#fff3", "inset": true},
                {"x": 0.0, "y": 4.0, "blur": 8.0, "spread": 2.0, "color": "#0004"}]})
        );
        assert_eq!(
            read(json!({"stroke": "4px solid #7C5CFF"})),
            json!({"stroke": {"width": 4.0, "color": "#7C5CFF"}})
        );
        assert_eq!(
            read(json!({"border": "2px dashed #000"})),
            json!({"stroke": {"width": 2.0, "dash": [6.0, 4.0], "color": "#000"}})
        );
        assert_eq!(
            read(
                json!({"fill": {"gradient": {"type": "radial", "stops": [{"position": 0, "color": "#fff"}, {"position": 1, "color": "#000"}]}}})
            ),
            json!({"fill": {"gradient": {"type": "radial", "stops": [{"offset": 0, "color": "#fff"}, {"offset": 1, "color": "#000"}]}}})
        );
    }

    #[test]
    fn what_isnt_css_stays_as_written() {
        // A plain stroke color, a shadow object, a CSS gradient string.
        let same = json!({"stroke": "#000", "shadow": {"y": 4}, "fill": "linear-gradient(#fff, #000)", "padding": [8, 16]});
        assert_eq!(read(same.clone()), same);
        // Not a length: left for the scene's own error.
        assert_eq!(read(json!({"padding": "auto"})), json!({"padding": "auto"}));
    }

    #[test]
    fn round_two_spellings_read_as_meant() {
        // From the subject-test runs: CSS place names, an angle beside its
        // gradient, stops as [offset, color] pairs.
        assert_eq!(
            read(json!({"place": "bottom-center"})),
            json!({"place": "bottom"})
        );
        assert_eq!(
            read(json!({"place": "center-left"})),
            json!({"place": "left"})
        );
        assert_eq!(read(json!({"place": "middle"})), json!({"place": "center"}));
        assert_eq!(
            read(json!({"place": "left-top"})),
            json!({"place": "top-left"})
        );
        assert_eq!(
            read(json!({"place": "nowhere"})),
            json!({"place": "nowhere"})
        );
        assert_eq!(
            read(json!({"fill": {"gradient": {"stops": ["#000", "#fff"]}, "angle": 180}})),
            json!({"fill": {"gradient": {"stops": ["#000", "#fff"], "angle": 180}}})
        );
        assert_eq!(
            read(
                json!({"fill": {"gradient": {"stops": [[0, "rgba(246,239,227,0)"], [1, "#F6EFE3"]]}}})
            ),
            json!({"fill": {"gradient": {"stops": [{"offset": 0, "color": "rgba(246,239,227,0)"}, {"offset": 1, "color": "#F6EFE3"}]}}})
        );
        assert_eq!(
            read(
                json!({"fill": {"type": "linear-gradient", "stops": [{"at": 0, "color": "#000"}, "#fff"]}})
            ),
            json!({"fill": {"type": "linear", "stops": [{"offset": 0, "color": "#000"}, "#fff"]}})
        );
    }

    #[test]
    fn a_css_margin_on_a_placed_layer_reads_as_its_x_y() {
        // From the subject-test runs.
        assert_eq!(
            read(json!({"place": "bottom", "margin": [0, 0, 215, 90]})),
            json!({"place": "bottom", "margin": [90.0, 215.0]})
        );
        assert_eq!(
            read(json!({"place": "top-right", "margin": "40px 56px 0 0"})),
            json!({"place": "top-right", "margin": [56.0, 40.0]})
        );
        // Not placed: left for its error, which says what to do instead.
        assert_eq!(
            read(json!({"margin": [34, 0, 0, 0]})),
            json!({"margin": [34, 0, 0, 0]})
        );
    }
}
