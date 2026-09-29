//! Layout in CSS flexbox, CSS grid and Figma auto-layout words: fields
//! written on the frame itself (`padding`, `gap`, `justifyContent`) go into
//! its `stack` or `grid`, and their values take the scene's spellings.

use serde_json::{Map, Value, json};

use super::css;

/// Stack fields as CSS and Figma write them on the container.
const STACK_KEYS: &[(&str, &str)] = &[
    ("dir", "dir"),
    ("direction", "dir"),
    ("flexDirection", "dir"),
    ("layoutMode", "dir"),
    ("justify", "justify"),
    ("justifyContent", "justify"),
    ("primaryAxisAlignItems", "justify"),
    ("alignItems", "align"),
    ("counterAxisAlignItems", "align"),
    ("wrap", "wrap"),
    ("flexWrap", "wrap"),
    ("gap", "gap"),
    ("itemSpacing", "gap"),
    ("padding", "padding"),
];

/// A frame's layout fields: moved into its `stack` or `grid`, which a
/// direction (or `display: flex`) creates.
pub(super) fn frame(o: &mut Map<String, Value>) {
    let display = o.remove("display");
    if display.as_ref().is_some_and(|d| d == "grid") {
        o.entry("grid").or_insert_with(|| json!({}));
    }
    for (from, to) in [
        ("gridTemplateColumns", "columns"),
        ("gridTemplateRows", "rows"),
        ("gridTemplateAreas", "areas"),
    ] {
        if let Some(v) = o.remove(from) {
            let g = o.entry("grid").or_insert_with(|| json!({}));
            if let Some(g) = g.as_object_mut() {
                g.entry(to).or_insert(v);
            }
        }
    }
    let target = if o.contains_key("grid") {
        "grid"
    } else {
        "stack"
    };
    let moving: Vec<(&str, &str)> = STACK_KEYS
        .iter()
        .copied()
        .filter(|(from, _)| o.contains_key(*from))
        .collect();
    let flex = display.as_ref().is_some_and(|d| d == "flex");
    if moving.is_empty() && !flex {
        return;
    }
    if target == "stack" && !o.contains_key("stack") {
        // CSS flex runs in a row unless told; a frame with only padding or
        // a gap flows its children down, like a block.
        let dir = if flex { "row" } else { "column" };
        o.insert("stack".into(), json!({ "dir": dir }));
    }
    let Some(Value::Object(mut into)) = o.remove(target) else {
        return;
    };
    for (from, to) in moving {
        if target == "grid" && !matches!(to, "gap" | "padding") {
            continue;
        }
        if let Some(v) = o.remove(from) {
            into.insert(to.into(), v);
        }
    }
    o.insert(target.into(), Value::Object(into));
}

/// A stack's values in flexbox and Figma spellings.
pub(super) fn stack(s: &mut Map<String, Value>) {
    for (from, to) in STACK_KEYS {
        if from != to
            && !s.contains_key(*to)
            && let Some(v) = s.remove(*from)
        {
            s.insert((*to).into(), v);
        }
    }
    gaps(s);
    word(s, "dir", &[("horizontal", "row"), ("vertical", "column")]);
    word(s, "justify", JUSTIFY);
    word(s, "align", ALIGN);
    if let Some(w) = s.get("wrap").and_then(Value::as_str) {
        let on = w != "nowrap" && w != "no_wrap";
        s.insert("wrap".into(), on.into());
    }
    for k in ["gap", "padding"] {
        if let Some(v) = s.get(k).and_then(css::px_value) {
            s.insert(k.into(), v);
        }
    }
}

/// A grid's fields in CSS spellings: template strings and `auto-fill`.
pub(super) fn grid(g: &mut Map<String, Value>) {
    gaps(g);
    // `repeat(auto-fill, minmax(160px, 1fr))`: as many as fit, at least 160.
    if let Some(min) = g
        .get("columns")
        .and_then(Value::as_str)
        .filter(|c| c.contains("auto-fill") || c.contains("auto-fit"))
        .and_then(|c| c.split("minmax(").nth(1))
        .and_then(|m| m.split(',').next())
        .and_then(css::px)
    {
        g.insert("columns".into(), json!({ "min": min }));
    }
    // CSS `grid-template-areas: "a b" "c d"`.
    if let Some(Value::String(areas)) = g.get("areas") {
        let rows: Vec<Value> = areas
            .split('"')
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .map(Value::from)
            .collect();
        g.insert("areas".into(), Value::Array(rows));
    }
    for k in ["gap", "padding"] {
        if let Some(v) = g.get(k).and_then(css::px_value) {
            g.insert(k.into(), v);
        }
    }
}

/// `rowGap` and `columnGap` as `gap: [row, column]`.
fn gaps(s: &mut Map<String, Value>) {
    let (row, col) = (s.remove("rowGap"), s.remove("columnGap"));
    if !s.contains_key("gap") && (row.is_some() || col.is_some()) {
        let n = |v: Option<Value>| {
            v.and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(css::px)))
                .unwrap_or(0.0)
        };
        s.insert("gap".into(), json!([n(row), n(col)]));
    }
}

const JUSTIFY: &[(&str, &str)] = &[
    ("space-between", "between"),
    ("space_between", "between"),
    ("space-around", "around"),
    ("space-evenly", "evenly"),
    ("flex-start", "start"),
    ("flex-end", "end"),
    ("min", "start"),
    ("max", "end"),
    ("left", "start"),
    ("right", "end"),
    ("top", "start"),
    ("bottom", "end"),
];

const ALIGN: &[(&str, &str)] = &[
    ("flex-start", "start"),
    ("flex-end", "end"),
    ("min", "start"),
    ("max", "end"),
    ("left", "start"),
    ("right", "end"),
    ("top", "start"),
    ("bottom", "end"),
];

/// Lowercases a word value and maps it through `pairs`.
fn word(o: &mut Map<String, Value>, key: &str, pairs: &[(&str, &str)]) {
    if let Some(Value::String(s)) = o.get_mut(key) {
        let low = s.to_ascii_lowercase().replace('_', "-");
        *s = pairs
            .iter()
            .find(|(from, _)| *from == low)
            .map_or(low, |(_, to)| (*to).to_owned());
    }
}

/// A child's layout fields in flexbox, grid and Figma spellings.
pub(super) fn child(o: &mut Map<String, Value>) {
    word(o, "alignSelf", ALIGN);
    for from in ["flexGrow", "layoutGrow"] {
        if !o.contains_key("grow")
            && let Some(v) = o.remove(from)
        {
            o.insert("grow".into(), v);
        }
    }
    if !o.contains_key("area")
        && let Some(v) = o.remove("gridArea")
    {
        o.insert("area".into(), v);
    }
    if let Some(Value::Object(c)) = o.get_mut("constraints") {
        for (from, to) in [("horizontal", "h"), ("vertical", "v")] {
            if !c.contains_key(to)
                && let Some(v) = c.remove(from)
            {
                c.insert(to.into(), v);
            }
        }
        // Figma's constraint words: MIN and MAX are the near and far edge.
        word(
            c,
            "h",
            &[("min", "left"), ("max", "right"), ("left-right", "stretch")],
        );
        word(
            c,
            "v",
            &[("min", "top"), ("max", "bottom"), ("top-bottom", "stretch")],
        );
    }
    // `place`: "top left", "center-left", "middle" …
    if let Some(Value::String(p)) = o.get_mut("place") {
        *p = spot(p);
    }
}

/// One of the nine spots from the ways it's written.
fn spot(p: &str) -> String {
    let low = p.to_ascii_lowercase().replace('_', "-");
    let words: Vec<&str> = low
        .split([' ', '-'])
        .map(|w| if w == "middle" { "center" } else { w })
        .filter(|w| !w.is_empty())
        .collect();
    let v = words.iter().find(|w| matches!(**w, "top" | "bottom"));
    let h = words.iter().find(|w| matches!(**w, "left" | "right"));
    match (v, h) {
        (Some(v), Some(h)) => format!("{v}-{h}"),
        (Some(one), None) | (None, Some(one)) => (*one).to_owned(),
        (None, None) if words.iter().all(|w| *w == "center") => "center".into(),
        _ => p.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{child, frame, grid, spot, stack};
    use serde_json::{Value, json};

    #[test]
    fn frame_level_flex_fields_go_into_the_stack() {
        let mut o = json!({"display": "flex", "flexDirection": "column", "justifyContent": "space-between",
            "alignItems": "flex-start", "gap": "12px", "padding": "8px 16px"});
        let m = o.as_object_mut().unwrap();
        frame(m);
        let s = m.get_mut("stack").unwrap().as_object_mut().unwrap();
        stack(s);
        assert_eq!(
            Value::Object(s.clone()),
            json!({"dir": "column", "justify": "between", "align": "start", "gap": 12.0, "padding": [8.0, 16.0]})
        );
        // Padding alone: a column, as a block flows.
        let mut o = json!({"padding": 24});
        frame(o.as_object_mut().unwrap());
        assert_eq!(o, json!({"stack": {"dir": "column", "padding": 24}}));
        // With a grid, gap and padding go into it.
        let mut o = json!({"grid": {"columns": 2}, "gap": 8});
        frame(o.as_object_mut().unwrap());
        assert_eq!(o, json!({"grid": {"columns": 2, "gap": 8}}));
    }

    #[test]
    fn css_grid_and_child_spellings_read_as_scene_values() {
        let mut g = json!({"columns": "repeat(auto-fill, minmax(160px, 1fr))", "areas": "\"a b\" \"c d\"",
            "rowGap": 8, "columnGap": "16px"});
        grid(g.as_object_mut().unwrap());
        assert_eq!(
            g,
            json!({"columns": {"min": 160.0}, "areas": ["a b", "c d"], "gap": [8.0, 16.0]})
        );
        let mut c = json!({"alignSelf": "flex-end", "flexGrow": 2, "place": "Top Left",
            "constraints": {"horizontal": "LEFT_RIGHT", "vertical": "TOP"}});
        child(c.as_object_mut().unwrap());
        assert_eq!(
            c,
            json!({"alignSelf": "end", "grow": 2, "place": "top-left",
                   "constraints": {"h": "stretch", "v": "top"}})
        );
        assert_eq!(spot("center-left"), "left");
        assert_eq!(spot("middle"), "center");
        assert_eq!(spot("bottom center"), "bottom");
    }
}
