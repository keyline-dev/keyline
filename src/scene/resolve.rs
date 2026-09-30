//! The scene as drawn: text styles and per-size `media` changes applied.

use super::{Kind, Layer, Scene, Size, check_keys};

/// Keys a style can't set: what makes a layer this layer.
pub(super) const UNSTYLABLE: &[&str] = &[
    "id",
    "type",
    "text",
    "children",
    "style",
    "component",
    "props",
    "each",
    "$tokens",
    "$defaults",
];

/// `l` with its styles applied under its own fields, or `None` when it has
/// no style. A field the layer leaves at its default takes the style's; of
/// several styles, a later one wins.
pub(super) fn styled(l: &Layer, scene: &Scene) -> Result<Option<Layer>, String> {
    let Some(names) = l.style.as_ref().map(super::StyleRef::names) else {
        return Ok(None);
    };
    let mut v = serde_json::to_value(l).map_err(|e| e.to_string())?;
    let obj = v.as_object_mut().ok_or("layer isn't an object")?;
    // Own fields set to their default aren't stored; they still win.
    if let Some(serde_json::Value::Object(own)) = obj.remove("$defaults") {
        for (k, val) in own {
            obj.entry(k).or_insert(val);
        }
    }
    for name in names.iter().rev() {
        let style = scene.styles.get(name).ok_or_else(|| {
            let known: Vec<&str> = scene.styles.keys().map(String::as_str).collect();
            format!("unknown style {name}; styles: {}", known.join(", "))
        })?;
        let mut style = serde_json::Value::Object(style.clone());
        crate::reuse::tokens::substitute(&mut style, &scene.tokens)
            .map_err(|e| format!("style {name}: {e}"))?;
        if let serde_json::Value::Object(style) = style {
            for (k, val) in style {
                obj.entry(k).or_insert(val);
            }
        }
    }
    obj.remove("style");
    // A style's frame fields (`padding`, `gap`) go where the layer keeps them.
    crate::ops::normalize(&mut v);
    let layer: Layer = serde_json::from_value(v.clone()).map_err(|e| format!("style: {e}"))?;
    check_keys(&v, &layer).map_err(|e| format!("style {}: {e}", names.join(", ")))?;
    Ok(Some(layer))
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
    // `style` is applied before `media`, so a size can't swap it.
    if let Some(k) = ["id", "type", "children", "media", "style"]
        .iter()
        .find(|k| patch.contains_key(**k))
    {
        return Err(format!("media.{key} can't change {k}"));
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

/// Styles by name.
type Styles = std::collections::BTreeMap<String, serde_json::Map<String, serde_json::Value>>;

impl Scene {
    /// The scene as drawn at one size: every layer's `at` changes for it
    /// applied. Borrowed when no layer has any. Call on a validated scene.
    pub fn for_size(&self, size: &Size) -> std::borrow::Cow<'_, Scene> {
        fn go(layers: &mut [Layer], size: &Size, styles: &Styles) {
            for l in layers {
                if let Ok(Some(s)) = sized(l, size) {
                    *l = s;
                    // A size's own text may name styles as tags too.
                    if let Kind::Text { text, .. } = &mut l.kind
                        && text.contains('<')
                        && !styles.is_empty()
                    {
                        *text = crate::text::markup::expand_styles(text, styles);
                    }
                }
                if let Some(children) = l.kind.children_mut() {
                    go(children, size, styles);
                }
            }
        }
        let mut any = false;
        self.walk(&mut |l| any |= !l.at.is_empty());
        if !any {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut layers = self.layers.clone();
        go(&mut layers, size, &self.styles);
        std::borrow::Cow::Owned(Scene {
            layers,
            ..self.clone()
        })
    }

    /// The scene as drawn: components expanded, styles applied, style
    /// tags in markup expanded. Borrowed when there's nothing to resolve.
    /// Call on a validated scene; see [`Scene::try_resolved`].
    pub fn resolved(&self) -> std::borrow::Cow<'_, Scene> {
        match self.try_resolved() {
            Ok(Some(s)) => std::borrow::Cow::Owned(s),
            Ok(None) | Err(_) => std::borrow::Cow::Borrowed(self),
        }
    }

    /// Like [`Scene::resolved`], reporting what can't be resolved; `None`
    /// when the scene is already as drawn.
    ///
    /// # Errors
    /// An unknown component or style, or a layer that doesn't parse once
    /// resolved.
    pub fn try_resolved(&self) -> Result<Option<Scene>, String> {
        fn go(layers: &mut [Layer], scene: &Scene) -> Result<(), String> {
            for l in layers {
                if let Some(children) = l.kind.children_mut() {
                    go(children, scene)?;
                }
                if let Some(s) = styled(l, scene).map_err(|e| format!("{}: {e}", l.id))? {
                    *l = s;
                }
                // Style-name tags in markup become spans with the style's
                // fields (their tokens already in place).
                if let Kind::Text { text, .. } = &mut l.kind
                    && text.contains('<')
                    && !scene.styles.is_empty()
                {
                    *text = crate::text::markup::expand_styles(text, &scene.styles);
                }
            }
            Ok(())
        }
        let mut any = false;
        self.walk(&mut |l| {
            any |= l.style.is_some() || matches!(l.kind, Kind::Use { .. }) || l.time.shot.is_some();
        });
        if !any && self.styles.is_empty() && self.tokens.is_empty() {
            return Ok(None);
        }
        // Styles with their tokens in place, for layers and markup tags alike.
        let mut scene = self.clone();
        for (name, style) in &mut scene.styles {
            let mut v = serde_json::Value::Object(std::mem::take(style));
            crate::reuse::tokens::substitute(&mut v, &self.tokens)
                .map_err(|e| format!("style {name}: {e}"))?;
            if let serde_json::Value::Object(o) = v {
                *style = o;
            }
        }
        let mut layers = self.layers.clone();
        crate::reuse::components::expand(&scene, &mut layers)?;
        go(&mut layers, &scene)?;
        // At rest (stills, checks), a scene of shots shows its first shot.
        for l in layers.iter_mut().filter(|l| l.time.shot.is_some()).skip(1) {
            l.hidden = true;
        }
        scene.layers = layers;
        Ok(Some(scene))
    }
}

#[cfg(test)]
mod tests {
    use crate::scene::{Kind, Scene};

    #[test]
    fn a_sizes_own_text_expands_style_tags() {
        let s: Scene = serde_json::from_value(serde_json::json!({"width": 100, "height": 100,
            "sizes": [{"id": "a", "width": 100, "height": 100}, {"id": "b", "width": 50, "height": 100}],
            "styles": {"accent": {"color": "#D0202E"}},
            "layers": [{"id": "t", "type": "text", "text": "Big <accent>news</accent>",
                "media": {"b": {"text": "Small <accent>news</accent>"}}}]}))
        .unwrap();
        s.validate().unwrap();
        let resolved = s.resolved();
        let b = resolved.for_size(&s.sizes[1]);
        let Kind::Text { text, .. } = &b.layers[0].kind else {
            unreachable!()
        };
        assert!(!text.contains("<accent>"), "{text}");
        assert!(text.contains("#D0202E"), "{text}");
    }

    #[test]
    fn at_changes_a_layer_for_one_size_only() {
        let mut s: Scene = serde_json::from_value(serde_json::json!({"width": 100,
            "height": 100,
            "sizes": [
                {"id": "a", "width": 100, "height": 100},
                {"id": "b", "width": 50, "height": 100}],
            "layers": [
                {"id": "t", "type": "text", "text": "Hi", "fontSize": 10, "media": {"b": {"fontSize": 20, "color": "#FF0000"}}}]}))
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
        assert!(
            s.validate()
                .unwrap_err()
                .contains("media.b can't change type")
        );
        let restyle = serde_json::json!({"style": "big"});
        s.layers[0]
            .at
            .insert("b".into(), restyle.as_object().unwrap().clone());
        assert!(
            s.validate()
                .unwrap_err()
                .contains("media.b can't change style")
        );
        let typo = serde_json::json!({"fontsize": 20});
        s.layers[0]
            .at
            .insert("b".into(), typo.as_object().unwrap().clone());
        assert!(s.validate().unwrap_err().contains("fontsize → fontSize"));
    }

    #[test]
    fn styles_fill_in_what_a_text_layer_leaves_out() {
        let mut s: Scene = serde_json::from_value(serde_json::json!({"width": 100,
            "height": 100,
            "sizes": [{"id": "a", "width": 100, "height": 100}],
            "styles": {"label": {"fontSize": 30, "fontWeight": 700, "color": "#FF0000"}},
            "layers": [
                {"id": "a", "type": "text", "text": "A", "style": "label"},
                {"id": "b", "type": "text", "text": "B", "style": "label", "color": "#0000FF"}]}))
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
        let s: Scene = serde_json::from_value(serde_json::json!({"width": 100,
            "height": 100,
            "sizes": [
                {"id": "sky", "width": 100, "height": 400},
                {"id": "sq", "width": 100, "height": 100}],
            "layers": [
                {"id": "t", "type": "text", "text": "Hi", "fontSize": 10, "media": {"portrait": {"fontSize": 20, "fontWeight": 700}, "tall": {"fontSize": 30}, "sky": {"fontWeight": 800}}}]}))
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
