//! Unknown-key checks for layers, which serde can't do with `flatten`.

use super::{Kind, Layer};

impl Kind {
    /// Keys a layer of this kind may carry, besides the common ones.
    fn keys(&self) -> &'static [&'static str] {
        match self {
            Kind::Image { .. } => &["asset", "fit", "crop", "tileScale", "focus", "adjust"],
            Kind::Video { .. } => &[
                "asset", "fit", "crop", "focus", "adjust", "start", "delay", "speed", "loop",
                "audio",
            ],
            Kind::Text { .. } => &[
                "text",
                "ranges",
                "resize",
                "maxLines",
                "minFontScale",
                "ellipsis",
                "fontSize",
                "weight",
                "align",
                "color",
                "fontFamily",
                "letterSpacing",
                "lineHeight",
                "textCase",
                "shadow",
                "fill",
                "gradient",
                "outline",
                "italic",
                "decoration",
                "textWrap",
                "verticalAlign",
                "trim",
                "highlight",
                "direction",
                "features",
                "padding",
                "curve",
                "leader",
                "knockout",
            ],
            Kind::Rect { .. } => &["color", "gradient", "stroke", "cornerRadius"],
            Kind::Ellipse { .. } => &["color", "gradient", "stroke", "arc"],
            Kind::Polygon { .. } => &["color", "gradient", "stroke", "sides", "innerRadius"],
            Kind::Path { .. } => &[
                "color", "gradient", "stroke", "d", "shape", "fillRule", "fitPath",
            ],
            Kind::Line { .. } => &["color", "strokeWidth"],
            Kind::Icon { .. } => &["name", "set", "color", "strokeWidth"],
            Kind::Frame { .. } => &[
                "children",
                "clip",
                "color",
                "gradient",
                "stroke",
                "cornerRadius",
                "stack",
                "grid",
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
    "rotation",
    "blendMode",
    "mask",
    "at",
    "minWidth",
    "maxWidth",
    "minHeight",
    "maxHeight",
    "aspectRatio",
    "place",
    "inset",
    "hidden",
    "alignSelf",
    "grow",
    "priority",
    "position",
    "area",
    "cell",
    "span",
    "fills",
    "strokes",
    "shadows",
    "blur",
    "backdropBlur",
    "radius",
    "scale",
    "offset",
    "skew",
    "flipX",
    "flipY",
    "edges",
    "style",
    "$tokens",
    "in",
    "out",
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
    // A wrong-case key (`fontsize`) gets a suggestion; anything else, the list.
    let known = || allowed.iter().chain(COMMON_KEYS);
    let hints: Vec<String> = unknown
        .iter()
        .filter_map(|u| {
            known()
                .find(|k| k.eq_ignore_ascii_case(u))
                .map(|k| format!("{u} → {k}"))
        })
        .collect();
    let help = if hints.len() == unknown.len() {
        format!("did you mean {}", hints.join(", "))
    } else {
        format!("allowed: {}", allowed.join(", "))
    };
    Err(format!(
        "unknown field(s) {} for {} layer; {help}",
        unknown.join(", "),
        layer.kind.name()
    ))
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
        let v = json!({"type": "text", "text": "hi", "bogus": 1});
        assert!(
            check_keys(&v, &l)
                .unwrap_err()
                .contains("allowed: text, ranges")
        );
    }
}
