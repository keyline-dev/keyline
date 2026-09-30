//! First guesses agents make, read as what they mean: names and values
//! from CSS, React Native, Figma, GSAP and video tools. A rejected guess
//! costs the agent a resend of its whole batch, so these are cheaper to
//! accept than to refuse.

pub(crate) mod dollar;
mod names;

use serde_json::{Map, Value};

/// A layer as written, in the scene's own names and shapes; its children
/// and `at` changes too.
pub(crate) fn normalize(v: &mut Value) {
    let Some(o) = v.as_object_mut() else { return };
    // A generic "shape" is a path when it names one, else a rect.
    if o.get("type").and_then(Value::as_str) == Some("shape") {
        let kind = if o.contains_key("d") || o.contains_key("shape") {
            "path"
        } else {
            "rect"
        };
        o.insert("type".into(), kind.into());
    }
    // An icon named as `icon`, as in icon-font markup.
    if o.get("type").and_then(Value::as_str) == Some("icon")
        && !o.contains_key("name")
        && let Some(n) = o.remove("icon")
    {
        o.insert("name".into(), n);
    }
    line_ends(o);
    shot(o);
    let kind = o.get("type").and_then(Value::as_str).map(str::to_owned);
    fields(o, kind.as_deref());
    if let Some(Value::Object(media)) = o.get_mut("media") {
        for patch in media.values_mut().filter_map(Value::as_object_mut) {
            fields(patch, kind.as_deref());
        }
    }
    if let Some(Value::Array(children)) = o.get_mut("children") {
        children.iter_mut().for_each(normalize);
    }
}

/// The fields of a layer, a style (no `kind`) or a `media` change.
pub(crate) fn fields(o: &mut Map<String, Value>, kind: Option<&str>) {
    names::fields(o, kind);
}

/// `{"type": "shot", "duration": 3, "transition": "fade", …}`: a frame that
/// fills the canvas, with its timing under `shot`.
fn shot(o: &mut Map<String, Value>) {
    if o.get("type").and_then(Value::as_str) != Some("shot") {
        return;
    }
    o.insert("type".into(), "frame".into());
    let mut timing = Map::new();
    for k in ["duration", "transition"] {
        if let Some(v) = o.remove(k) {
            timing.insert(k.into(), v);
        }
    }
    o.insert("shot".into(), Value::Object(timing));
    for side in ["width", "height"] {
        o.entry(side).or_insert_with(|| "fill".into());
    }
}

/// A line written by its ends, `x1, y1, x2, y2` (SVG's form). A line runs
/// from its box's top-left to its bottom-right, so one running another way
/// takes its box and a mirror: `flipX` for down-left, `flipY` for up-right,
/// both for up-left. The ends (and any arrowheads) stay where written.
fn line_ends(o: &mut Map<String, Value>) {
    if o.get("type").and_then(Value::as_str) != Some("line") {
        return;
    }
    let num = |k: &str| o.get(k).and_then(Value::as_f64);
    let (Some(x1), Some(y1), Some(x2), Some(y2)) = (num("x1"), num("y1"), num("x2"), num("y2"))
    else {
        return;
    };
    for k in ["x1", "y1", "x2", "y2"] {
        o.remove(k);
    }
    for (k, v) in [
        ("x", x1.min(x2)),
        ("y", y1.min(y2)),
        ("width", (x2 - x1).abs()),
        ("height", (y2 - y1).abs()),
    ] {
        o.insert(k.into(), v.into());
    }
    // Mirrored on each axis where the line runs backwards; a flip already
    // there is undone.
    for (flip, backwards) in [("flipX", x2 < x1), ("flipY", y2 < y1)] {
        if backwards {
            let was = o.get(flip).and_then(Value::as_bool).unwrap_or(false);
            o.insert(flip.into(), (!was).into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::normalize;
    use serde_json::json;

    #[test]
    fn icon_and_line_guesses_read_as_meant() {
        let mut v = json!({"type": "icon", "icon": "mail"});
        normalize(&mut v);
        assert_eq!(v, json!({"type": "icon", "name": "mail"}));
        let line = |x1: i32, y1: i32, x2: i32, y2: i32| {
            let mut v = json!({"type": "line", "x1": x1, "y1": y1, "x2": x2, "y2": y2});
            normalize(&mut v);
            v
        };
        // Down-right needs no mirror.
        assert_eq!(
            line(10, 20, 40, 90),
            json!({"type": "line", "x": 10.0, "y": 20.0, "width": 30.0, "height": 70.0})
        );
        // Down-left (a benchmark agent's envelope flap), up-right, up-left.
        let v = line(168, 797, 139, 818);
        assert_eq!((&v["x"], &v["y"]), (&json!(139.0), &json!(797.0)));
        assert_eq!((&v["width"], &v["height"]), (&json!(29.0), &json!(21.0)));
        assert_eq!((v.get("flipX"), v.get("flipY")), (Some(&json!(true)), None));
        let v = line(0, 50, 50, 0);
        assert_eq!((v.get("flipX"), v.get("flipY")), (None, Some(&json!(true))));
        let v = line(50, 50, 0, 0);
        assert_eq!(
            (v.get("flipX"), v.get("flipY")),
            (Some(&json!(true)), Some(&json!(true)))
        );
    }
}
