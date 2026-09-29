//! Unknown-key checks for layers, which serde can't do with `flatten`.

use super::{Kind, Layer};

impl Kind {
    /// Keys a layer of this kind may carry, besides the common ones.
    fn keys(&self) -> &'static [&'static str] {
        match self {
            Kind::Image { .. } => &["asset", "fit", "crop", "tileScale", "focus", "filter"],
            Kind::Video { .. } => &[
                "asset",
                "fit",
                "crop",
                "focus",
                "filter",
                "trimStart",
                "delay",
                "playbackRate",
                "loop",
                "muted",
            ],
            Kind::Text { .. } => &[
                "text",
                "ranges",
                "maxLines",
                "minimumScaleFactor",
                "fontSize",
                "fontWeight",
                "textAlign",
                "color",
                "fontFamily",
                "letterSpacing",
                "lineHeight",
                "textTransform",
                "fontStyle",
                "textDecoration",
                "textWrap",
                "textAlignVertical",
                "trim",
                "highlight",
                "direction",
                "features",
                "padding",
                "curve",
                "leader",
                "knockout",
            ],
            Kind::Rect { .. } | Kind::Line { .. } => &[],
            Kind::Ellipse { .. } => &["arc"],
            Kind::Polygon { .. } => &["sides", "innerRadius"],
            Kind::Path { .. } => &["d", "shape", "fillRule", "fit"],
            Kind::Icon { .. } => &["name", "set", "color", "strokeWidth"],
            Kind::Frame { .. } => &[
                "children",
                "clipsContent",
                "flexDirection",
                "justifyContent",
                "alignItems",
                "flexWrap",
                "gap",
                "padding",
                "gridTemplateColumns",
                "gridTemplateRows",
                "gridTemplateAreas",
            ],
            Kind::Spacer { .. } => &["minLength"],
            Kind::FirstFit { .. } => &["children"],
            Kind::Use { .. } => &["component", "props", "each"],
        }
    }
}

const COMMON_KEYS: &[&str] = &[
    "type",
    "id",
    "role",
    "x",
    "y",
    "width",
    "height",
    "constraints",
    "opacity",
    "rotate",
    "blendMode",
    "mask",
    "media",
    "minWidth",
    "maxWidth",
    "minHeight",
    "maxHeight",
    "aspectRatio",
    "place",
    "margin",
    "hidden",
    "alignSelf",
    "flexGrow",
    "layoutPriority",
    "position",
    "gridArea",
    "gridRow",
    "gridColumn",
    "fill",
    "stroke",
    "shadow",
    "blur",
    "backdropBlur",
    "borderRadius",
    "scale",
    "translate",
    "skew",
    "flipX",
    "flipY",
    "edges",
    "style",
    "$tokens",
    "$defaults",
    "enter",
    "exit",
    "animate",
    "stagger",
    "split",
    "shot",
];

/// Rejects keys that don't belong to the layer's type, so a typo like
/// `fontsize` fails loudly instead of silently falling back to a default.
pub fn check_keys(json: &serde_json::Value, layer: &Layer) -> Result<(), String> {
    let Some(obj) = json.as_object() else {
        return Err("layer must be an object".into());
    };
    let allowed = layer.kind.keys();
    let unknown: Vec<&str> = obj
        .keys()
        .map(String::as_str)
        .filter(|k| !COMMON_KEYS.contains(k) && !allowed.contains(k))
        .collect();
    if unknown.is_empty() {
        return Ok(());
    }
    // Each unknown key gets the nearest known one, when one is close.
    let known: Vec<&str> = allowed
        .iter()
        .chain(COMMON_KEYS)
        .copied()
        .filter(|k| !k.starts_with('$'))
        .collect();
    let hints: Vec<String> = unknown
        .iter()
        .filter_map(|u| nearest(u, &known).map(|k| format!("{u} → {k}")))
        .collect();
    let help = if matches!(layer.kind, Kind::Use { .. }) && unknown.contains(&"children") {
        "a use layer takes props or each; put {{prop}} in the component's text, e.g. \
\"text\": \"{{name}}\" with \"each\": [{\"name\": \"Dana\"}]"
            .to_owned()
    } else if hints.is_empty() {
        let own = if allowed.is_empty() {
            String::new()
        } else {
            format!("{} fields: {}; ", layer.kind.name(), allowed.join(", "))
        };
        format!(
            "{own}any layer: x, y, width, height, place, margin, fill, stroke, shadow, borderRadius, \
opacity, rotate, style, media, enter, animate (scene.md has them all)"
        )
    } else {
        format!("did you mean {}", hints.join(", "))
    };
    Err(format!(
        "unknown field(s) {} for {} layer; {help}",
        unknown.join(", "),
        layer.kind.name()
    ))
}

/// The known key closest to `u`, when it's within a third of its length
/// in edits (case ignored): `fontsize` → `fontSize`, `fontWieght` → `fontWeight`.
fn nearest<'a>(u: &str, known: &[&'a str]) -> Option<&'a str> {
    let u = u.to_ascii_lowercase();
    known
        .iter()
        .map(|k| (edits(&u, &k.to_ascii_lowercase()), *k))
        .filter(|(d, _)| *d <= (u.len() / 3).max(1))
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| k)
}

/// Levenshtein distance.
fn edits(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use crate::scene::{Layer, check_keys};
    use serde_json::json;

    #[test]
    fn unknown_keys_are_rejected() {
        let v = json!({"type": "text", "text": "hi", "fontsize": 12});
        let l: Layer = serde_json::from_value(v.clone()).unwrap();
        let err = check_keys(&v, &l).unwrap_err();
        assert!(err.ends_with("did you mean fontsize → fontSize"), "{err}");
        let v = json!({"type": "text", "text": "hi", "fontWieght": 700, "bogus": 1});
        let err = check_keys(&v, &l).unwrap_err();
        assert!(err.contains("fontWieght → fontWeight"), "{err}");
        let v = json!({"type": "text", "text": "hi", "bogus": 1});
        assert!(
            check_keys(&v, &l)
                .unwrap_err()
                .contains("text fields: text, ranges")
        );
    }
}
