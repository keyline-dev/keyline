//! Scene invariants serde can't express, checked after every mutation.

use super::resolve::{STYLE_KEYS, sized, styled};
use super::{Kind, Layer, Resize, Scene};

impl Scene {
    /// Checks invariants serde can't express. Called after every mutation.
    pub fn validate(&self) -> Result<(), String> {
        for (name, style) in &self.styles {
            if let Some(k) = style.keys().find(|k| !STYLE_KEYS.contains(&k.as_str())) {
                return Err(format!(
                    "style {name}: {k} can't be styled; allowed: {}",
                    STYLE_KEYS.join(", ")
                ));
            }
        }
        let mut result = Ok(());
        self.walk(&mut |l| {
            if result.is_ok()
                && let Err(e) = styled(l, &self.styles)
            {
                result = Err(format!("{}: {e}", l.id));
            }
        });
        result?;
        let mut result = Ok(());
        self.walk(&mut |l| {
            for size in l.at.keys() {
                if result.is_err() {
                    return;
                }
                result = if self.sizes.iter().any(|s| s.id == *size) {
                    sized(l, size).map(drop)
                } else {
                    Err(format!("no size {size}"))
                }
                .map_err(|e| format!("{}: {e}", l.id));
            }
        });
        result?;
        let resolved = self.resolved();
        resolved.validate_resolved()?;
        for size in &self.sizes {
            resolved.for_size(&size.id).validate_resolved()?;
        }
        Ok(())
    }

    fn validate_resolved(&self) -> Result<(), String> {
        if self.width <= 0.0 || self.height <= 0.0 {
            return Err("scene width and height must be > 0".into());
        }
        if self.sizes.is_empty() {
            return Err("scene needs at least one size".into());
        }
        let mut size_ids = std::collections::HashSet::new();
        for s in &self.sizes {
            // Size ids name the rendered files.
            crate::store::check_id(&s.id).map_err(|e| format!("size {e}"))?;
            if s.width < 1.0 || s.height < 1.0 || s.scale <= 0.0 {
                return Err(format!("size {}: width, height >= 1 and scale > 0", s.id));
            }
            if !size_ids.insert(&s.id) {
                return Err(format!("duplicate size id {}", s.id));
            }
        }
        let mut ids = std::collections::HashSet::new();
        let mut result = Ok(());
        self.walk(&mut |l| {
            if result.is_err() {
                return;
            }
            if !ids.insert(l.id.clone()) {
                result = Err(format!("duplicate layer id {}", l.id));
            } else if let Err(e) = self.validate_layer(l) {
                result = Err(format!("{}: {e}", l.id));
            }
        });
        result
    }

    fn validate_layer(&self, l: &Layer) -> Result<(), String> {
        if !(0.0..=1.0).contains(&l.opacity) {
            return Err("opacity must be 0–1".into());
        }
        if let Kind::Rect {
            gradient, stroke, ..
        }
        | Kind::Ellipse {
            gradient, stroke, ..
        }
        | Kind::Frame {
            gradient, stroke, ..
        } = &l.kind
        {
            let strokes = stroke.iter().filter_map(|s| s.gradient.as_ref());
            for g in gradient.iter().chain(strokes) {
                if g.stops.len() < 2 || g.stops.iter().any(|s| !(0.0..=1.0).contains(&s.at)) {
                    return Err("gradient needs at least 2 stops, each at 0–1".into());
                }
            }
            if stroke.as_ref().is_some_and(|s| s.width <= 0.0) {
                return Err("stroke width must be > 0".into());
            }
        }
        if let Kind::Frame { stack: Some(s), .. } = &l.kind
            && (s.gap < 0.0 || s.padding < 0.0)
        {
            return Err("stack gap and padding must be >= 0".into());
        }
        if let Some(m) = &l.mask
            && (m.stops.len() < 2 || m.stops.iter().any(|s| !(0.0..=1.0).contains(&s.at)))
        {
            return Err("mask needs at least 2 stops, each at 0–1".into());
        }
        if l.width.is_some_and(|w| w < 0.0) || l.height.is_some_and(|h| h < 0.0) {
            return Err("width and height must be >= 0".into());
        }
        match &l.kind {
            Kind::Image { asset, .. } if !self.assets.contains_key(asset) => {
                Err(format!("unknown asset {asset}"))
            }
            Kind::Image { tile_scale, .. } if *tile_scale <= 0.0 => {
                Err("tileScale must be > 0".into())
            }
            Kind::Image { focus, .. } if focus.iter().any(|f| !(0.0..=1.0).contains(f)) => {
                Err("focus must be [x, y], each 0–1".into())
            }
            Kind::Line { stroke_width, .. } if *stroke_width <= 0.0 => {
                Err("strokeWidth must be > 0".into())
            }
            Kind::Icon { stroke_width: Some(w), .. } if *w <= 0.0 => {
                Err("strokeWidth must be > 0".into())
            }
            Kind::Icon { name, set, .. } => crate::icons::check(*set, name),
            Kind::Image {
                crop: Some(c), ..
            } if c.width <= 0.0 || c.height <= 0.0 || c.x < 0.0 || c.y < 0.0 || c.x + c.width > 1.001 || c.y + c.height > 1.001 => {
                Err("crop must lie within the image: x, y ≥ 0, width, height > 0, x+width and y+height ≤ 1".into())
            }
            Kind::Text {
                text,
                ranges,
                weight,
                font_size,
                min_font_scale,
                font_family,
                fill,
                gradient,
                outline,
                ..
            } => {
                if let Some(f) = fill {
                    if !self.assets.contains_key(&f.asset) {
                        return Err(format!("unknown asset {} in fill", f.asset));
                    }
                    if f.tile_scale <= 0.0 {
                        return Err("fill tileScale must be > 0".into());
                    }
                }
                if gradient
                    .as_ref()
                    .is_some_and(|g| g.stops.len() < 2 || g.stops.iter().any(|s| !(0.0..=1.0).contains(&s.at)))
                {
                    return Err("gradient needs at least 2 stops, each at 0–1".into());
                }
                if outline.as_ref().is_some_and(|o| o.width <= 0.0) {
                    return Err("outline width must be > 0".into());
                }
                if !(100..=900).contains(weight) || weight % 100 != 0 {
                    return Err("weight must be 100, 200, … 900".into());
                }
                if !crate::text::families().contains(font_family) {
                    return Err(format!(
                        "unknown fontFamily {font_family}; available: {}",
                        crate::text::families().join(", ")
                    ));
                }
                if *font_size <= 0.0 {
                    return Err("fontSize must be > 0".into());
                }
                if !(*min_font_scale > 0.0 && *min_font_scale <= 1.0) {
                    return Err("minFontScale must be > 0 and <= 1".into());
                }
                let resize = l.text_resize();
                if resize == Some(Resize::AutoHeight) && l.width.is_none() {
                    return Err("auto-height text needs a width".into());
                }
                if matches!(resize, Some(Resize::Fit | Resize::Fixed | Resize::Truncate))
                    && (l.width.is_none() || l.height.is_none())
                {
                    return Err("fit, fixed and truncate text need width and height".into());
                }
                let n = text.chars().count();
                match ranges.iter().find(|r| r.start >= r.end || r.end > n) {
                    Some(r) => Err(format!(
                        "range {}..{} out of bounds for {n} characters",
                        r.start, r.end
                    )),
                    None => Ok(()),
                }
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::scene::Scene;
    use serde_json::json;

    #[test]
    fn icons_must_exist_in_their_set() {
        let s: Scene = serde_json::from_value(serde_json::json!({
            "width": 100, "height": 100, "sizes": [{"id": "a", "width": 100, "height": 100}],
            "layers": [{"type": "icon", "name": "envelope"}]
        }))
        .unwrap();
        // Font Awesome has envelope; Lucide, the default set, calls it mail.
        assert!(
            s.validate()
                .unwrap_err()
                .contains("no lucide icon envelope")
        );
    }

    #[test]
    fn validate_catches_bad_ranges_and_duplicates() {
        let mut s: Scene = serde_json::from_value(json!({
            "width": 100, "height": 100, "sizes": [{"id": "a", "width": 100, "height": 100}],
            "layers": [{"id": "t", "type": "text", "text": "abc", "ranges": [{"start": 1, "end": 5, "color": "#FF0000"}]}]
        }))
        .unwrap();
        assert!(s.validate().unwrap_err().contains("out of bounds"));
        s.layers[0] = serde_json::from_value(json!({"id": "t", "type": "rect"})).unwrap();
        s.layers.push(s.layers[0].clone());
        assert!(s.validate().unwrap_err().contains("duplicate"));
    }

    #[test]
    fn size_ids_must_work_as_file_names() {
        let s: Scene = serde_json::from_value(json!({
            "width": 10, "height": 10, "sizes": [{"id": "1080x1350 portrait", "width": 10, "height": 10}]
        }))
        .unwrap();
        assert!(s.validate().unwrap_err().contains("bad id"));
    }
}
