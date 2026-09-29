//! Field names and values from CSS, React Native, Figma, GSAP and video
//! tools, read as the scene's own: `fontWeight: "bold"` is `weight: 700`,
//! `borderRadius` is `radius`, `muted: true` is `audio: false`.

use serde_json::{Map, Value, json};

use super::css;

/// Renames `from` to `to` unless `to` is already given.
fn rename(o: &mut Map<String, Value>, from: &str, to: &str) {
    if !o.contains_key(to)
        && let Some(v) = o.remove(from)
    {
        o.insert(to.into(), v);
    }
}

/// Maps a string value through `pairs` (lowercased first).
fn words(o: &mut Map<String, Value>, key: &str, pairs: &[(&str, &str)]) {
    if let Some(Value::String(s)) = o.get_mut(key) {
        let low = s.to_ascii_lowercase();
        if let Some((_, to)) = pairs.iter().find(|(from, _)| *from == low) {
            *s = (*to).to_owned();
        }
    }
}

/// Fields any layer (or style, or `at` patch) may carry. `kind` is the
/// layer's `type` when known.
pub(super) fn fields(o: &mut Map<String, Value>, kind: Option<&str>) {
    let text = kind == Some("text");
    // Text.
    rename(o, "fontWeight", "weight");
    if let Some(w) = o.get("weight").and_then(weight) {
        o.insert("weight".into(), w.into());
    }
    rename(o, "textAlign", "align");
    words(o, "align", &[("start", "left"), ("end", "right")]);
    if o.remove("fontStyle").is_some_and(|st| {
        st.as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("italic") || s == "oblique")
    }) {
        o.entry("italic").or_insert(true.into());
    }
    rename(o, "textTransform", "textCase");
    words(
        o,
        "textCase",
        &[("uppercase", "upper"), ("lowercase", "lower")],
    );
    rename(o, "textDecorationLine", "decoration");
    rename(o, "textDecoration", "decoration");
    words(
        o,
        "decoration",
        &[
            ("line-through", "strike"),
            ("strikethrough", "strike"),
            ("strike-through", "strike"),
        ],
    );
    if o.get("decoration").and_then(Value::as_str) == Some("none") {
        o.remove("decoration");
    }
    for from in ["numberOfLines", "lineClamp", "maxLine"] {
        rename(o, from, "maxLines");
    }
    if let Some(f) = o
        .get("fontFamily")
        .and_then(Value::as_str)
        .and_then(css::first_family)
    {
        o.insert("fontFamily".into(), f.into());
    }
    units(o);
    // Box.
    for from in ["borderRadius", "cornerRadius"] {
        rename(o, from, "radius");
    }
    rename(o, "rotate", "rotation");
    if o.remove("visible").is_some_and(|v| v == false) {
        o.entry("hidden").or_insert(true.into());
    }
    if let Some(v) = o.remove("overflow") {
        o.entry("clip").or_insert((v != "visible").into());
    }
    for side in ["width", "height"] {
        words(
            o,
            side,
            &[
                ("auto", "hug"),
                ("fit-content", "hug"),
                ("wrap_content", "hug"),
                ("match_parent", "fill"),
                ("stretch", "fill"),
            ],
        );
    }
    // Paint: text's own `fill` and `shadow` are the common fields too.
    for (short, full) in [("fill", "fills"), ("shadow", "shadows")] {
        rename(o, short, full);
    }
    for from in ["boxShadow", "textShadow", "dropShadow"] {
        rename(o, from, "shadows");
    }
    if text {
        for from in ["backgroundColor", "background"] {
            rename(o, from, "highlight");
        }
    } else {
        for from in ["backgroundColor", "background"] {
            rename(o, from, "fills");
        }
    }
    strokes(o, kind);
    paint_strings(o);
    video(o);
    for k in ["in", "out"] {
        if let Some(Value::Object(m)) = o.get_mut(k) {
            motion(m);
        }
    }
    rename(o, "animation", "animate");
    match o.get_mut("animate") {
        Some(Value::Object(a)) => track(a),
        Some(Value::Array(list)) => list
            .iter_mut()
            .filter_map(Value::as_object_mut)
            .for_each(track),
        _ => {}
    }
    for k in [
        "padding",
        "inset",
        "radius",
        "letterSpacing",
        "fontSize",
        "blur",
        "backdropBlur",
    ] {
        if let Some(v) = o.get(k).and_then(css::px_value) {
            o.insert(k.into(), v);
        }
    }
}

/// `"bold"`, `"600"` or `"semibold"` as a number.
fn weight(v: &Value) -> Option<u32> {
    let s = v
        .as_str()?
        .to_ascii_lowercase()
        .replace(['-', ' ', '_'], "");
    s.parse().ok().or(match s.as_str() {
        "thin" | "hairline" => Some(100),
        "extralight" | "ultralight" => Some(200),
        "light" | "lighter" => Some(300),
        "normal" | "regular" | "book" => Some(400),
        "medium" => Some(500),
        "semibold" | "demibold" => Some(600),
        "bold" => Some(700),
        "extrabold" | "ultrabold" | "bolder" => Some(800),
        "black" | "heavy" => Some(900),
        _ => None,
    })
}

/// `lineHeight` as a multiple, and `letterSpacing` as px, from CSS and
/// Figma units: `"120%"`, `"1.5em"`, `"72px"`, `"0.05em"`, `"-2%"`. Units
/// relative to the font need the layer's own `fontSize`.
fn units(o: &mut Map<String, Value>) {
    let size = o
        .get("fontSize")
        .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(css::px)));
    if let Some(s) = o.get("lineHeight").and_then(Value::as_str).map(str::trim) {
        let ratio = if let Some(p) = s.strip_suffix('%') {
            p.parse::<f64>().ok().map(|p| p / 100.0)
        } else if let Some(e) = s.strip_suffix("em") {
            e.parse().ok()
        } else {
            css::px(s).and_then(|px| size.map(|f| px / f))
        };
        if let Some(r) = ratio {
            o.insert("lineHeight".into(), json!(r));
        }
    }
    if let Some(s) = o
        .get("letterSpacing")
        .and_then(Value::as_str)
        .map(str::trim)
    {
        let px = if let Some(e) = s.strip_suffix("em") {
            e.parse::<f64>().ok().zip(size).map(|(e, f)| e * f)
        } else if let Some(p) = s.strip_suffix('%') {
            p.parse::<f64>().ok().zip(size).map(|(p, f)| p / 100.0 * f)
        } else {
            css::px(s)
        };
        if let Some(px) = px {
            o.insert("letterSpacing".into(), json!(px));
        }
    }
}

/// CSS and React Native borders as a stroke: `border: "2px solid #000"`,
/// or `borderWidth`/`borderColor` (and `strokeWidth`/`strokeColor` where
/// the type has no stroke width of its own).
fn strokes(o: &mut Map<String, Value>, kind: Option<&str>) {
    if o.contains_key("strokes") {
        return;
    }
    if let Some(s) = o
        .get("border")
        .and_then(Value::as_str)
        .and_then(css::border)
    {
        o.remove("border");
        o.insert("strokes".into(), s);
        return;
    }
    let own_width = matches!(kind, Some("line" | "icon"));
    let width = o
        .remove("borderWidth")
        .or_else(|| (!own_width).then(|| o.remove("strokeWidth")).flatten());
    let color = o.remove("borderColor").or_else(|| o.remove("strokeColor"));
    if width.is_some() || color.is_some() {
        let mut s = Map::new();
        s.insert("width".into(), width.unwrap_or(json!(1)));
        if let Some(c) = color {
            s.insert("color".into(), c);
        }
        o.insert("strokes".into(), Value::Object(s));
    }
}

/// CSS strings where a paint or a shadow goes: gradient functions in
/// `fills` or `color`, box-shadow strings in `shadows`.
fn paint_strings(o: &mut Map<String, Value>) {
    // An image paint names its image `image`; text's old `fill` said `asset`.
    if let Some(Value::Object(f)) = o.get_mut("fills")
        && !f.contains_key("image")
    {
        rename(f, "asset", "image");
    }
    let grad = |v: &Value| v.as_str().and_then(css::gradient);
    if let Some(g) = o.get("color").and_then(grad) {
        o.remove("color");
        o.entry("fills").or_insert(g);
    }
    if let Some(g) = o.get("gradient").and_then(grad) {
        o.insert("gradient".into(), g["gradient"].clone());
    }
    match o.get_mut("fills") {
        Some(v @ Value::String(_)) => {
            if let Some(g) = grad(v) {
                *v = g;
            }
        }
        Some(Value::Array(list)) => {
            for v in list.iter_mut() {
                if let Some(g) = grad(v) {
                    *v = g;
                }
            }
        }
        _ => {}
    }
    match o.get_mut("shadows") {
        Some(v @ Value::String(_)) => {
            if let Some(s) = v.as_str().and_then(css::shadows) {
                *v = s;
            }
        }
        Some(Value::Object(s)) => figma_shadow(s),
        Some(Value::Array(list)) => list
            .iter_mut()
            .filter_map(Value::as_object_mut)
            .for_each(figma_shadow),
        _ => {}
    }
}

/// Figma's `{offset: {x, y}, radius}` and React Native's `shadowOffset`,
/// `blurRadius`, `spreadRadius`.
fn figma_shadow(s: &mut Map<String, Value>) {
    if let Some(Value::Object(off)) = s.remove("offset").or_else(|| s.remove("shadowOffset")) {
        for (from, to) in [("x", "x"), ("y", "y"), ("width", "x"), ("height", "y")] {
            if let Some(v) = off.get(from) {
                s.entry(to).or_insert(v.clone());
            }
        }
    }
    for (from, to) in [
        ("radius", "blur"),
        ("blurRadius", "blur"),
        ("spreadRadius", "spread"),
    ] {
        rename(s, from, to);
    }
    if s.remove("type")
        .is_some_and(|t| t == "INNER_SHADOW" || t == "inner")
    {
        s.entry("inset").or_insert(true.into());
    }
}

/// A clip's names in other video tools.
fn video(o: &mut Map<String, Value>) {
    for from in ["trimStart", "trim_start", "startFrom", "trimBefore"] {
        rename(o, from, "start");
    }
    for from in ["playbackRate", "playback_rate"] {
        rename(o, from, "speed");
    }
    if let Some(m) = o.remove("muted") {
        o.entry("audio").or_insert((m == false).into());
    }
    rename(o, "volume", "audio");
    if let Some(v) = o.get("audio").and_then(Value::as_f64) {
        o.insert("audio".into(), (v > 0.0).into());
    }
}

/// An enter or exit: `delay` (GSAP, CSS) is its start, `type` its effect.
fn motion(m: &mut Map<String, Value>) {
    rename(m, "delay", "at");
    rename(m, "type", "effect");
    rename(m, "easing", "ease");
}

/// A keyframe track in GSAP's words: `delay`, `x`/`y`, `rotate`.
fn track(a: &mut Map<String, Value>) {
    motion(a);
    rename(a, "rotate", "rotation");
    rename(a, "iterations", "repeat");
    if a.contains_key("offset") {
        return;
    }
    let (x, y) = (a.remove("x"), a.remove("y"));
    if x.is_none() && y.is_none() {
        return;
    }
    let pair = |x: &Value, y: &Value| json!([x, y]);
    let zero = json!(0);
    let offset = match (x.as_ref(), y.as_ref()) {
        (Some(Value::Array(xs)), ys) => {
            let ys: Vec<Value> = match ys {
                Some(Value::Array(ys)) => ys.clone(),
                _ => vec![zero.clone(); xs.len()],
            };
            Value::Array(
                xs.iter()
                    .zip(ys.iter().chain(std::iter::repeat(&zero)))
                    .map(|(x, y)| pair(x, y))
                    .collect(),
            )
        }
        (None, Some(Value::Array(ys))) => Value::Array(ys.iter().map(|y| pair(&zero, y)).collect()),
        (x, y) => {
            // `{from, to}` objects, or one value each: GSAP's `x: 100` is a
            // move to 100.
            let get = |v: Option<&Value>, k: &str| match v {
                Some(Value::Object(o)) => o.get(k).cloned(),
                Some(n) if k == "to" => Some(n.clone()),
                _ => None,
            };
            let mut out = Map::new();
            for k in ["from", "to"] {
                let (gx, gy) = (get(x, k), get(y, k));
                if gx.is_some() || gy.is_some() {
                    out.insert(
                        k.into(),
                        pair(&gx.unwrap_or(zero.clone()), &gy.unwrap_or(zero.clone())),
                    );
                }
            }
            Value::Object(out)
        }
    };
    a.insert("offset".into(), offset);
}

#[cfg(test)]
mod tests {
    use super::fields;
    use serde_json::{Value, json};

    fn norm(v: Value, kind: Option<&str>) -> Value {
        let mut v = v;
        fields(v.as_object_mut().unwrap(), kind);
        v
    }

    #[test]
    fn css_and_react_native_text_names_read_as_scene_fields() {
        let v = norm(
            json!({"fontWeight": "semibold", "textAlign": "center", "fontStyle": "italic",
                   "textTransform": "uppercase", "textDecoration": "line-through",
                   "numberOfLines": 2, "fontFamily": "'Playfair Display', serif",
                   "fontSize": "40px", "lineHeight": "120%", "letterSpacing": "0.05em"}),
            Some("text"),
        );
        assert_eq!(
            v,
            json!({"weight": 600, "align": "center", "italic": true, "textCase": "upper",
                   "decoration": "strike", "maxLines": 2, "fontFamily": "Playfair Display",
                   "fontSize": 40.0, "lineHeight": 1.2, "letterSpacing": 2.0})
        );
        assert_eq!(
            norm(json!({"lineHeight": "60px", "fontSize": 40}), None)["lineHeight"],
            1.5
        );
    }

    #[test]
    fn box_paint_and_border_names_read_as_scene_fields() {
        let v = norm(
            json!({"borderRadius": "8px", "backgroundColor": "linear-gradient(to right, red, blue)",
                   "border": "2px solid #000", "boxShadow": "0 4px 12px rgba(0,0,0,.25)",
                   "visible": false, "overflow": "hidden", "width": "auto"}),
            Some("frame"),
        );
        assert_eq!(v["radius"], 8.0);
        assert_eq!(v["fills"]["gradient"]["angle"], 90.0);
        assert_eq!(v["strokes"], json!({"width": 2.0, "color": "#000"}));
        assert_eq!(v["shadows"]["blur"], 12.0);
        assert_eq!(
            (&v["hidden"], &v["clip"], &v["width"]),
            (&json!(true), &json!(true), &json!("hug"))
        );
        let v = norm(
            json!({"shadow": {"offset": {"x": 0, "y": 2}, "radius": 4, "color": "#0004"}}),
            None,
        );
        assert_eq!(
            v["shadows"],
            json!({"x": 0, "y": 2, "blur": 4, "color": "#0004"})
        );
    }

    #[test]
    fn video_and_motion_names_read_as_scene_fields() {
        let v = norm(
            json!({"trimStart": 2, "muted": true, "playbackRate": 0.5}),
            Some("video"),
        );
        assert_eq!(v, json!({"start": 2, "audio": false, "speed": 0.5}));
        let v = norm(
            json!({"in": {"type": "fade-up", "delay": 0.4},
                   "animate": {"x": [0, 100], "rotate": {"from": -10}, "delay": 1}}),
            None,
        );
        assert_eq!(v["in"], json!({"effect": "fade-up", "at": 0.4}));
        assert_eq!(
            v["animate"],
            json!({"offset": [[0, 0], [100, 0]], "rotation": {"from": -10}, "at": 1})
        );
        let v = norm(json!({"animate": {"y": {"from": 40}}}), None);
        assert_eq!(v["animate"]["offset"], json!({"from": [0, 40]}));
    }
}
