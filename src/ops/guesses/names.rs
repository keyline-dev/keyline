//! The slips models make most often, read as meant: Figma's plurals,
//! `color` on a shape, `fontWeight: "bold"`, `display: "flex"`. Anything
//! else unknown is an error with a suggestion.

use serde_json::{Map, Value, json};

/// Renames `from` to `to` unless `to` is already given.
fn rename(o: &mut Map<String, Value>, from: &str, to: &str) {
    if !o.contains_key(to)
        && let Some(v) = o.remove(from)
    {
        o.insert(to.into(), v);
    }
}

/// Fields any layer (or style, or `media` change) may carry. `kind` is the
/// layer's `type` when known.
pub(super) fn fields(o: &mut Map<String, Value>, kind: Option<&str>) {
    // Figma's plurals, and its corner name.
    for (from, to) in [
        ("fills", "fill"),
        ("strokes", "stroke"),
        ("shadows", "shadow"),
        ("cornerRadius", "borderRadius"),
    ] {
        rename(o, from, to);
    }
    // A shape's or frame's color is its fill; text and icons keep `color`.
    // A line's color and width are its stroke's; SVG's strokeWidth too.
    if kind == Some("line") || (kind != Some("icon") && o.contains_key("strokeWidth")) {
        let (color, width) = (
            if kind == Some("line") {
                o.remove("color")
            } else {
                None
            },
            o.remove("strokeWidth"),
        );
        let mut s = match o.remove("stroke") {
            Some(Value::String(c)) => json!({ "color": c }),
            None => json!({}),
            // An object, or a list of strokes, is left as written.
            Some(other) => other,
        };
        if let Value::Object(m) = &mut s {
            if let Some(c) = color {
                m.entry("color").or_insert(c);
            }
            if let Some(w) = width {
                m.entry("width").or_insert(w);
            }
        }
        if s.as_object().is_none_or(|m| !m.is_empty()) {
            o.insert("stroke".into(), s);
        }
    }
    let own_color = matches!(kind, Some("text" | "icon") | None);
    if !own_color {
        rename(o, "color", "fill");
    }
    for from in ["backgroundColor", "background"] {
        if kind == Some("text") {
            rename(o, from, "highlight");
        } else {
            rename(o, from, "fill");
        }
    }
    // A `layout` word for the stack's direction (`stack` stacks down);
    // any other value stays, for its error.
    if let Some(Value::String(l)) = o.get("layout") {
        let dir = match l.as_str() {
            "row" | "horizontal" | "hstack" => Some("row"),
            "column" | "vertical" | "vstack" | "stack" | "flex" => Some("column"),
            _ => None,
        };
        if let Some(d) = dir {
            o.remove("layout");
            o.entry("flexDirection").or_insert_with(|| d.into());
        }
    }
    // CSS `overflow` is Figma's clipsContent.
    if let Some(v) = o.remove("overflow") {
        o.entry("clipsContent").or_insert((v != "visible").into());
    }
    // CSS `display`: flex is a row unless told, none is hidden; others are
    // dropped (a grid is made by its templates).
    match o.remove("display").as_ref().and_then(Value::as_str) {
        Some("flex" | "inline-flex") => {
            o.entry("flexDirection").or_insert_with(|| "row".into());
        }
        Some("none") => {
            o.entry("hidden").or_insert(true.into());
        }
        _ => {}
    }
    if o.remove("visible").is_some_and(|v| v == false) {
        o.entry("hidden").or_insert(true.into());
    }
    // Priority words, and CSS flex-shrink 0 (keep the size): give way last.
    if let Some(Value::String(p)) = o.get("layoutPriority") {
        let n = match p.as_str() {
            "low" => -1,
            "high" => 1,
            _ => 0,
        };
        o.insert("layoutPriority".into(), n.into());
    }
    if let Some(s) = o.remove("flexShrink")
        && s == 0
    {
        o.entry("layoutPriority").or_insert(1.into());
    }
    // A spacer's minimum is its length along the stack.
    if kind == Some("spacer") {
        for from in ["minHeight", "minWidth"] {
            rename(o, from, "minLength");
        }
    }
    // `flex: 1` on a child: its share of the free space.
    if let Some(n) = o.remove("flex") {
        o.entry("flexGrow").or_insert(n);
    }
    if let Some(w) = o.get("fontWeight").and_then(weight) {
        o.insert("fontWeight".into(), w.into());
    }
    for k in ["alignItems", "justifyContent", "alignSelf"] {
        if let Some(Value::String(s)) = o.get_mut(k)
            && matches!(s.as_str(), "start" | "end")
        {
            *s = format!("flex-{s}");
        }
    }
    match o.get_mut("animate") {
        Some(Value::Object(a)) => track(a),
        Some(Value::Array(list)) => list
            .iter_mut()
            .filter_map(Value::as_object_mut)
            .for_each(track),
        _ => {}
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

/// A keyframe track in GSAP's words: `x`/`y`, `rotation`, `drawSVG`; After
/// Effects' `trimPath`; a `counter`.
fn track(a: &mut Map<String, Value>) {
    rename(a, "rotation", "rotate");
    rename(a, "drawSVG", "draw");
    rename(a, "trimPath", "draw");
    rename(a, "counter", "count");
    if a.contains_key("translate") {
        return;
    }
    let (x, y) = (a.remove("x"), a.remove("y"));
    if x.is_none() && y.is_none() {
        return;
    }
    let zero = json!(0);
    let pair = |x: &Value, y: &Value| json!([x, y]);
    let translate = match (x.as_ref(), y.as_ref()) {
        (Some(Value::Array(xs)), ys) => {
            let ys: Vec<Value> = match ys {
                Some(Value::Array(ys)) => ys.clone(),
                _ => Vec::new(),
            };
            Value::Array(
                xs.iter()
                    .enumerate()
                    .map(|(i, x)| pair(x, ys.get(i).unwrap_or(&zero)))
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
                        pair(
                            &gx.unwrap_or_else(|| zero.clone()),
                            &gy.unwrap_or_else(|| zero.clone()),
                        ),
                    );
                }
            }
            Value::Object(out)
        }
    };
    a.insert("translate".into(), translate);
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
    fn common_slips_read_as_meant() {
        let v = norm(
            json!({"fills": "#fff", "cornerRadius": 8, "visible": false,
                   "flex": 1, "alignItems": "start", "strokes": "#000"}),
            Some("rect"),
        );
        assert_eq!(
            v,
            json!({"fill": "#fff", "borderRadius": 8, "hidden": true, "flexGrow": 1,
                   "alignItems": "flex-start", "stroke": "#000"})
        );
        let v = norm(json!({"display": "flex", "gap": 8}), Some("frame"));
        assert_eq!(v, json!({"flexDirection": "row", "gap": 8}));
        let v = norm(json!({"overflow": "visible"}), Some("frame"));
        assert_eq!(v, json!({"clipsContent": false}));
        let v = norm(
            json!({"layoutPriority": "low", "flexShrink": 0}),
            Some("text"),
        );
        assert_eq!(v, json!({"layoutPriority": -1}));
        let v = norm(json!({"flexShrink": 0}), Some("icon"));
        assert_eq!(v, json!({"layoutPriority": 1}));
        let v = norm(json!({"minHeight": 8, "flexGrow": 1}), Some("spacer"));
        assert_eq!(v, json!({"minLength": 8, "flexGrow": 1}));
        let v = norm(json!({"display": "none"}), Some("frame"));
        assert_eq!(v, json!({"hidden": true}));
        let v = norm(json!({"color": "#f00", "strokeWidth": 2}), Some("line"));
        assert_eq!(v, json!({"stroke": {"color": "#f00", "width": 2}}));
        let v = norm(json!({"stroke": "#000", "strokeWidth": 3}), Some("rect"));
        assert_eq!(v, json!({"stroke": {"color": "#000", "width": 3}}));
        let list = json!({"stroke": [{"color": "#000"}, {"color": "#fff", "width": 4}]});
        assert_eq!(norm(list.clone(), Some("line")), list);
        let v = norm(json!({"color": "#000"}), Some("rect"));
        assert_eq!(v, json!({"fill": "#000"}));
        let v = norm(
            json!({"color": "#000", "fontWeight": "semibold"}),
            Some("text"),
        );
        assert_eq!(v, json!({"color": "#000", "fontWeight": 600}));
        let v = norm(json!({"backgroundColor": "#FFE600"}), Some("text"));
        assert_eq!(v, json!({"highlight": "#FFE600"}));
    }

    #[test]
    fn gsap_track_names_read_as_scene_fields() {
        let v = norm(
            json!({"animate": {"x": [0, 100], "rotation": {"from": -10}}}),
            None,
        );
        assert_eq!(
            v["animate"],
            json!({"translate": [[0, 0], [100, 0]], "rotate": {"from": -10}})
        );
        let v = norm(json!({"animate": {"y": {"from": 40}}}), None);
        assert_eq!(v["animate"]["translate"], json!({"from": [0, 40]}));
    }

    #[test]
    fn trim_paths_and_counters_read_as_draw_and_count() {
        let v = norm(json!({"animate": {"drawSVG": [0, 1]}}), None);
        assert_eq!(v["animate"], json!({"draw": [0, 1]}));
        let v = norm(
            json!({"animate": [{"trimPath": [0, 1]}, {"counter": [0, 9]}]}),
            None,
        );
        assert_eq!(v["animate"], json!([{"draw": [0, 1]}, {"count": [0, 9]}]));
    }
}
