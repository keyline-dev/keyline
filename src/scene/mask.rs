//! Masks: what part of a layer shows. A gradient (whose alpha fades
//! the layer), a shape name, `{path}`, `{layer}` or `{image}`, each
//! with `mode` (alpha or luminance) and `invert`.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::Gradient;

/// A layer's mask.
#[derive(Debug, Clone, PartialEq)]
pub struct Mask {
    /// What the mask is made of.
    pub source: MaskSource,
    /// Use the source's alpha (default) or its brightness.
    pub mode: MaskMode,
    /// Show what the source hides instead.
    pub invert: bool,
}

/// What a mask is made of.
#[derive(Debug, Clone, PartialEq)]
pub enum MaskSource {
    /// A gradient over the box; its alpha is the layer's opacity there.
    Gradient(Gradient),
    /// A shape in the box: `ellipse`, or a named shape (`blob-1`, …).
    Shape(String),
    /// An SVG path in the box.
    Path(String),
    /// Another layer's alpha; that layer isn't drawn on its own.
    Layer(String),
    /// An image asset over the box (torn paper, brush strokes).
    Image(String),
}

/// Which channel of the mask source counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaskMode {
    /// The source's alpha.
    #[default]
    Alpha,
    /// The source's brightness: white shows, black hides.
    Luminance,
}

impl Serialize for Mask {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error;
        let plain = self.mode == MaskMode::Alpha && !self.invert;
        let mut o = match &self.source {
            MaskSource::Shape(name) if plain => return s.serialize_str(name),
            MaskSource::Gradient(g) => match serde_json::to_value(g).map_err(S::Error::custom)? {
                Value::Object(o) => o,
                _ => Map::new(),
            },
            MaskSource::Shape(v) => Map::from_iter([("shape".to_owned(), Value::from(v.as_str()))]),
            MaskSource::Path(v) => Map::from_iter([("path".to_owned(), Value::from(v.as_str()))]),
            MaskSource::Layer(v) => Map::from_iter([("layer".to_owned(), Value::from(v.as_str()))]),
            MaskSource::Image(v) => Map::from_iter([("image".to_owned(), Value::from(v.as_str()))]),
        };
        if self.mode == MaskMode::Luminance {
            o.insert("mode".into(), "luminance".into());
        }
        if self.invert {
            o.insert("invert".into(), true.into());
        }
        Value::Object(o).serialize(s)
    }
}

impl<'de> Deserialize<'de> for Mask {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let err = |e: String| D::Error::custom(format!("mask: {e}"));
        let mut o = match Value::deserialize(d)? {
            Value::String(name) => {
                return Ok(Mask {
                    source: MaskSource::Shape(name),
                    mode: MaskMode::Alpha,
                    invert: false,
                });
            }
            Value::Object(o) => o,
            _ => return Err(err("a shape name or an object".into())),
        };
        let mode = match o.remove("mode") {
            Some(m) => serde_json::from_value(m).map_err(|e| err(e.to_string()))?,
            None => MaskMode::Alpha,
        };
        let invert = match o.remove("invert") {
            Some(v) => v
                .as_bool()
                .ok_or_else(|| err("invert must be true or false".into()))?,
            None => false,
        };
        let text = |o: &mut Map<String, Value>, k: &str| -> Result<String, D::Error> {
            let v = o.remove(k).and_then(|v| v.as_str().map(str::to_owned));
            match (v, o.keys().next()) {
                (Some(v), None) => Ok(v),
                (_, Some(extra)) => Err(err(format!("unknown field {extra}"))),
                (None, None) => Err(err(format!("{k} must be a string"))),
            }
        };
        let source = if o.contains_key("stops") {
            MaskSource::Gradient(
                serde_json::from_value(Value::Object(o)).map_err(|e| err(e.to_string()))?,
            )
        } else if o.contains_key("shape") {
            MaskSource::Shape(text(&mut o, "shape")?)
        } else if o.contains_key("path") {
            MaskSource::Path(text(&mut o, "path")?)
        } else if o.contains_key("layer") {
            MaskSource::Layer(text(&mut o, "layer")?)
        } else if o.contains_key("image") {
            MaskSource::Image(text(&mut o, "image")?)
        } else {
            return Err(err(
                "use a gradient {stops}, a shape name, {path}, {layer} or {image}".into(),
            ));
        };
        Ok(Mask {
            source,
            mode,
            invert,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Mask, MaskMode, MaskSource};
    use serde_json::json;

    #[test]
    fn masks_read_every_form_and_write_them_back() {
        for v in [
            json!({"from": [0.5, 0.0], "to": [0.5, 1.0], "stops": [{"at": 0.0, "color": "#000000"}, {"at": 1.0, "color": "#00000000"}]}),
            json!("ellipse"),
            json!({"path": "M0 0 L1 1", "invert": true}),
            json!({"layer": "logo"}),
            json!({"image": "torn", "mode": "luminance"}),
        ] {
            let m: Mask = serde_json::from_value(v.clone()).unwrap();
            assert_eq!(serde_json::to_value(&m).unwrap(), v);
        }
        let m: Mask =
            serde_json::from_value(json!({"image": "torn", "mode": "luminance"})).unwrap();
        assert_eq!(
            (m.source, m.mode),
            (MaskSource::Image("torn".into()), MaskMode::Luminance)
        );
    }

    #[test]
    fn bad_masks_say_what_is_allowed() {
        let e = serde_json::from_value::<Mask>(json!({"shapes": "x"}))
            .unwrap_err()
            .to_string();
        assert!(e.contains("{path}, {layer} or {image}"), "{e}");
        let e = serde_json::from_value::<Mask>(json!({"layer": "a", "extra": 1}))
            .unwrap_err()
            .to_string();
        assert!(e.contains("unknown field extra"), "{e}");
    }
}
