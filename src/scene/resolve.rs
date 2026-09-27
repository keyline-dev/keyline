//! The scene as drawn: text styles and per-size `at` changes applied.

use std::collections::BTreeMap;

use super::{Kind, Layer, Scene, Size, check_keys};

/// Text keys a style may set: everything but the content itself.
pub(super) const STYLE_KEYS: &[&str] = &[
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
    "fills",
    "strokes",
    "shadows",
];

/// `l` with its text style applied under its own fields, or `None` when it
/// has no style. A field the layer leaves at its default takes the style's.
pub(super) fn styled(
    l: &Layer,
    styles: &BTreeMap<String, serde_json::Map<String, serde_json::Value>>,
) -> Result<Option<Layer>, String> {
    let Kind::Text {
        style: Some(name), ..
    } = &l.kind
    else {
        return Ok(None);
    };
    let style = styles
        .get(name)
        .ok_or_else(|| format!("unknown style {name}"))?;
    let mut v = serde_json::to_value(l).map_err(|e| e.to_string())?;
    let obj = v.as_object_mut().ok_or("layer isn't an object")?;
    for (k, val) in style {
        obj.entry(k.clone()).or_insert_with(|| val.clone());
    }
    obj.remove("style");
    serde_json::from_value(v)
        .map(Some)
        .map_err(|e| format!("style {name}: {e}"))
}

/// Aspect classes an `at` key can name instead of a size id.
pub const ASPECT_CLASSES: [&str; 5] = ["landscape", "square", "portrait", "wide", "tall"];

/// The aspect classes a size belongs to, broadest first: `landscape`
/// (w/h > 1.1), `square` (0.9–1.1) or `portrait` (< 0.9), then `wide`
/// (2:1 or wider) or `tall` (1:2 or taller). A 4:5 post is portrait; a
/// 300 × 600 half-page is tall.
pub fn aspect_classes(size: &Size) -> Vec<&'static str> {
    let r = size.width / size.height;
    let mut out = vec![if r > 1.1 {
        "landscape"
    } else if r >= 0.9 {
        "square"
    } else {
        "portrait"
    }];
    if r >= 2.0 {
        out.push("wide");
    } else if r <= 0.5 {
        out.push("tall");
    }
    out
}

/// `l` with the changes under one `at` key merged over its own fields, or
/// `None` when it has none. `at` itself is kept for further keys.
pub(super) fn patched(l: &Layer, key: &str) -> Result<Option<Layer>, String> {
    let Some(patch) = l.at.get(key) else {
        return Ok(None);
    };
    if let Some(k) = ["id", "type", "children", "at"]
        .iter()
        .find(|k| patch.contains_key(**k))
    {
        return Err(format!("at.{key} can't change {k}"));
    }
    let mut v = serde_json::to_value(l).map_err(|e| e.to_string())?;
    crate::ops::merge_patch(&mut v, &serde_json::Value::Object(patch.clone()));
    let layer: Layer = serde_json::from_value(v.clone()).map_err(|e| format!("at.{key}: {e}"))?;
    check_keys(&v, &layer).map_err(|e| format!("at.{key}: {e}"))?;
    Ok(Some(layer))
}

/// `l` as drawn at `size`: its aspect-class changes, broadest first, then
/// its changes for the size id. `None` when none apply.
pub(super) fn sized(l: &Layer, size: &Size) -> Result<Option<Layer>, String> {
    let mut keys = aspect_classes(size);
    keys.push(&size.id);
    let mut out: Option<Layer> = None;
    for key in keys {
        if let Some(p) = patched(out.as_ref().unwrap_or(l), key)? {
            out = Some(p);
        }
    }
    Ok(out.map(|mut l| {
        l.at.clear();
        l
    }))
}

impl Scene {
    /// The scene as drawn at one size: every layer's `at` changes for it
    /// applied. Borrowed when no layer has any. Call on a validated scene.
    pub fn for_size(&self, size: &Size) -> std::borrow::Cow<'_, Scene> {
        fn go(layers: &mut [Layer], size: &Size) {
            for l in layers {
                if let Ok(Some(s)) = sized(l, size) {
                    *l = s;
                }
                if let Some(children) = l.kind.children_mut() {
                    go(children, size);
                }
            }
        }
        let mut any = false;
        self.walk(&mut |l| any |= !l.at.is_empty());
        if !any {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut layers = self.layers.clone();
        go(&mut layers, size);
        std::borrow::Cow::Owned(Scene {
            layers,
            ..self.clone()
        })
    }

    /// The scene as drawn: every text layer's style applied. Borrowed when
    /// there are no styles. Call on a validated scene.
    pub fn resolved(&self) -> std::borrow::Cow<'_, Scene> {
        fn go(layers: &mut [Layer], scene: &Scene) {
            for l in layers {
                if let Some(children) = l.kind.children_mut() {
                    go(children, scene);
                }
                if let Ok(Some(s)) = styled(l, &scene.styles) {
                    *l = s;
                }
                // Style-name tags in markup become spans with the style's fields.
                if let Kind::Text { text, .. } = &mut l.kind
                    && text.contains('<')
                {
                    *text = crate::text::markup::expand_styles(text, &scene.styles);
                }
            }
        }
        if self.styles.is_empty() {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut layers = self.layers.clone();
        go(&mut layers, self);
        std::borrow::Cow::Owned(Scene {
            layers,
            ..self.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::scene::{Kind, Scene};

    #[test]
    fn at_changes_a_layer_for_one_size_only() {
        let mut s: Scene = serde_json::from_value(serde_json::json!({
            "width": 100, "height": 100,
            "sizes": [{"id": "a", "width": 100, "height": 100}, {"id": "b", "width": 50, "height": 100}],
            "layers": [{"id": "t", "type": "text", "text": "Hi", "fontSize": 10,
                        "at": {"b": {"fontSize": 20, "color": "#FF0000"}}}]
        }))
        .unwrap();
        s.validate().unwrap();
        let size = |id: &str| match &s
            .for_size(s.sizes.iter().find(|z| z.id == id).unwrap())
            .layers[0]
            .kind
        {
            Kind::Text {
                font_size, color, ..
            } => (*font_size, color.to_string()),
            _ => unreachable!(),
        };
        assert_eq!(size("a"), (10.0, "#000000".into()));
        assert_eq!(size("b"), (20.0, "#FF0000".into()));

        s.layers[0].at.insert("c".into(), serde_json::Map::new());
        assert!(
            s.validate()
                .unwrap_err()
                .contains("no size or aspect class c")
        );
        s.layers[0].at.remove("c");
        let bad = serde_json::json!({"type": "rect"});
        s.layers[0]
            .at
            .insert("b".into(), bad.as_object().unwrap().clone());
        assert!(s.validate().unwrap_err().contains("at.b can't change type"));
        let typo = serde_json::json!({"fontsize": 20});
        s.layers[0]
            .at
            .insert("b".into(), typo.as_object().unwrap().clone());
        assert!(s.validate().unwrap_err().contains("fontsize → fontSize"));
    }

    #[test]
    fn styles_fill_in_what_a_text_layer_leaves_out() {
        let mut s: Scene = serde_json::from_value(serde_json::json!({
            "width": 100, "height": 100, "sizes": [{"id": "a", "width": 100, "height": 100}],
            "styles": {"label": {"fontSize": 30, "weight": 700, "color": "#FF0000"}},
            "layers": [
                {"id": "a", "type": "text", "text": "A", "style": "label"},
                {"id": "b", "type": "text", "text": "B", "style": "label", "color": "#0000FF"}
            ]
        }))
        .unwrap();
        s.validate().unwrap();
        let r = s.resolved();
        let look = |i: usize| match &r.layers[i].kind {
            Kind::Text {
                font_size,
                weight,
                color,
                ..
            } => (*font_size, *weight, color.to_string()),
            _ => unreachable!(),
        };
        assert_eq!(look(0), (30.0, 700, "#FF0000".into()));
        // The layer's own color wins over the style's.
        assert_eq!(look(1), (30.0, 700, "#0000FF".into()));

        s.styles
            .get_mut("label")
            .unwrap()
            .insert("text".into(), "x".into());
        assert!(s.validate().unwrap_err().contains("text can't be styled"));
        s.styles.clear();
        assert!(s.validate().unwrap_err().contains("unknown style label"));
    }

    #[test]
    fn at_applies_aspect_classes_broadest_first_then_the_size_id() {
        let s: Scene = serde_json::from_value(serde_json::json!({
            "width": 100, "height": 100,
            "sizes": [{"id": "sky", "width": 100, "height": 400}, {"id": "sq", "width": 100, "height": 100}],
            "layers": [{"id": "t", "type": "text", "text": "Hi", "fontSize": 10,
                        "at": {"portrait": {"fontSize": 20, "weight": 700}, "tall": {"fontSize": 30}, "sky": {"weight": 800}}}]
        }))
        .unwrap();
        s.validate().unwrap();
        let get = |i: usize| match &s.for_size(&s.sizes[i]).layers[0].kind {
            Kind::Text {
                font_size, weight, ..
            } => (*font_size, *weight),
            _ => unreachable!(),
        };
        assert_eq!(get(0), (30.0, 800));
        assert_eq!(get(1), (10.0, 400));
        let classes = |w: f32, h: f32| {
            crate::scene::aspect_classes(
                &serde_json::from_value(serde_json::json!({"id": "x", "width": w, "height": h}))
                    .unwrap(),
            )
        };
        assert_eq!(classes(1920.0, 1080.0), ["landscape"]);
        assert_eq!(classes(728.0, 90.0), ["landscape", "wide"]);
        assert_eq!(classes(1080.0, 1080.0), ["square"]);
        assert_eq!(classes(1080.0, 1920.0), ["portrait"]);
        assert_eq!(classes(1080.0, 1350.0), ["portrait"]);
        assert_eq!(classes(1350.0, 1080.0), ["landscape"]);
        assert_eq!(classes(160.0, 600.0), ["portrait", "tall"]);
        assert_eq!(classes(300.0, 600.0), ["portrait", "tall"]);
        assert_eq!(classes(1200.0, 600.0), ["landscape", "wide"]);
    }
}
