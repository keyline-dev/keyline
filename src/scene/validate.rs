//! Scene invariants serde can't express, checked after every mutation.

use super::resolve::{ASPECT_CLASSES, UNSTYLABLE, patched};
use super::{Kind, Layer, Length, MAX_TRACKS, Resize, Scene};

impl Scene {
    /// Checks invariants serde can't express. Called after every mutation.
    pub fn validate(&self) -> Result<(), String> {
        for (name, style) in &self.styles {
            if let Some(k) = style.keys().find(|k| UNSTYLABLE.contains(&k.as_str())) {
                return Err(format!("style {name}: {k} can't be styled"));
            }
        }
        // Components expand and styles apply, or say why not.
        let valid = |v: f32, lo: f32, hi: f32| (lo..=hi).contains(&v);
        if self.duration.is_some_and(|d| !valid(d, 0.001, 86_400.0)) || !valid(self.fps, 1.0, 120.0)
        {
            return Err("duration must be > 0 and fps between 1 and 120".into());
        }
        let top: Vec<&str> = self
            .layers
            .iter()
            .filter(|l| l.time.shot.is_some())
            .map(|l| l.id.as_str())
            .collect();
        let mut shot_error = None;
        let mut prev: Option<f32> = None;
        for l in self.layers.iter().filter(|l| l.time.shot.is_some()) {
            let shot = l.time.shot.as_ref().map_or(0.0, |s| s.duration);
            let into = l
                .time
                .shot
                .as_ref()
                .and_then(|s| s.transition)
                .map_or(0.0, crate::anim::shots::Transition::overlap);
            if !matches!(l.kind, Kind::Frame { .. }) {
                shot_error = Some(format!("{}: a shot is a frame", l.id));
            } else if shot <= 0.0 {
                shot_error = Some(format!("{}: a shot needs a duration > 0", l.id));
            } else if into < 0.0 {
                shot_error = Some(format!("{}: a transition lasts 0 s or more", l.id));
            } else if prev.is_some_and(|p| into > p.min(shot)) {
                shot_error = Some(format!(
                    "{}: its transition is longer than a shot it joins",
                    l.id
                ));
            }
            prev = Some(shot);
        }
        self.walk(&mut |l| {
            if l.time.shot.is_some() && !top.contains(&l.id.as_str()) {
                shot_error = Some(format!("{}: shots are top-level layers", l.id));
            }
            if let Kind::Video {
                start,
                delay,
                speed,
                ..
            } = l.kind
                && (start < 0.0 || delay < 0.0 || !(0.01..=100.0).contains(&speed))
            {
                shot_error = Some(format!(
                    "{}: trimStart and delay are 0 s or more, playbackRate 0.01 to 100",
                    l.id
                ));
            }
        });
        if let Some(e) = shot_error {
            return Err(e);
        }
        self.check_units()?;
        let resolved = self.try_resolved()?;
        let resolved = resolved.as_ref().unwrap_or(self);
        let mut result = Ok(());
        // The resolved scene, so `at` inside components is checked too.
        resolved.walk(&mut |l| {
            for size in l.at.keys() {
                if result.is_err() {
                    return;
                }
                result = if self.sizes.iter().any(|s| s.id == *size)
                    || ASPECT_CLASSES.contains(&size.as_str())
                {
                    patched(l, size).map(drop)
                } else {
                    Err(format!(
                        "no size or aspect class {size}; classes: {}",
                        ASPECT_CLASSES.join(", ")
                    ))
                }
                .map_err(|e| format!("{}: {e}", l.id));
            }
        });
        result?;
        resolved.validate_resolved()?;
        for size in &self.sizes {
            resolved.for_size(size).validate_resolved()?;
        }
        Ok(())
    }

    /// Values whose unit was likely mistaken: a `lineHeight` in px, or a
    /// motion duration in ms.
    fn check_units(&self) -> Result<(), String> {
        let length = crate::anim::shots::length(self);
        let mut err = None;
        self.walk(&mut |l| {
            if let Kind::Text {
                line_height: Some(h),
                ..
            } = l.kind
                && h > 4.0
            {
                err.get_or_insert(format!(
                    "{}: lineHeight is × fontSize (1.2); for {h}px, divide by fontSize",
                    l.id
                ));
            }
            let Some(len) = length else { return };
            let mut long = l.time.enter.iter().chain(&l.time.out).map(|m| m.duration);
            let tracks = l.time.animate.as_ref().map_or(&[][..], |a| a.as_slice());
            // A value in ms: 100 or more, and longer than the scene.
            let ms = |d: &f32| *d >= 100.0 && *d > len;
            if let Some(d) = long
                .find(ms)
                .or_else(|| tracks.iter().map(|t| t.duration).find(ms))
            {
                err.get_or_insert(format!(
                    "{}: a {d} s motion in a {len} s scene; durations are seconds, not ms",
                    l.id
                ));
            }
        });
        err.map_or(Ok(()), Err)
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
        }
        | Kind::Polygon {
            gradient, stroke, ..
        }
        | Kind::Path {
            gradient, stroke, ..
        } = &l.kind
        {
            if let Some(g) = gradient {
                super::check::gradient(g)?;
            }
            if let Some(s) = stroke {
                super::check::stroke(s)?;
            }
        }
        if let Kind::Frame {
            layout: crate::scene::FrameLayout { stack: Some(s), .. },
            ..
        } = &l.kind
        {
            if s.gap.is_negative() || s.padding.sides().iter().any(|p| *p < 0.0) {
                return Err("stack gap and padding must be >= 0".into());
            }
            if s.dir.options().is_empty() {
                return Err("stack dir list needs at least one direction".into());
            }
        }
        if let Kind::Frame {
            layout: crate::scene::FrameLayout { stack, grid },
            children,
            ..
        } = &l.kind
        {
            if stack.is_some() && grid.is_some() {
                return Err("a frame takes stack or grid, not both".into());
            }
            if let Some(g) = grid {
                g.check()?;
                for child in children {
                    if let Some(a) = child.area.as_deref().filter(|a| g.area(a).is_none()) {
                        return Err(format!(
                            "{}: no grid area {a}; areas: {}",
                            child.id,
                            g.areas.join(" / ")
                        ));
                    }
                    let out_of_range = |n: u16| n == 0 || usize::from(n) > MAX_TRACKS;
                    if child.cell().into_iter().flatten().any(out_of_range)
                        || child.span().into_iter().any(out_of_range)
                    {
                        return Err(format!(
                            "{}: gridRow and gridColumn count from 1, up to {MAX_TRACKS}",
                            child.id
                        ));
                    }
                }
            }
        }
        if l.time.split.is_some() && !matches!(l.kind, Kind::Text { .. }) {
            return Err("split works on text layers".into());
        }
        check_lengths(l)?;
        super::check::look(self, l)?;
        super::check::mask(self, l)?;
        super::check::shape(l)?;
        match &l.kind {
            Kind::Image { asset, .. } if !self.assets.contains_key(asset) => {
                Err(format!("unknown asset {asset}; assets: {}", self.assets.keys().cloned().collect::<Vec<_>>().join(", ")))
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
                let set = |len: Option<Length>| len.is_some_and(|v| v != Length::Hug);
                if resize == Some(Resize::AutoHeight) && !set(l.width) {
                    return Err("auto-height text needs a width".into());
                }
                if matches!(resize, Some(Resize::Fit | Resize::Fixed | Resize::Truncate))
                    && (!set(l.width) || !set(l.height))
                {
                    return Err("fit, fixed and truncate text need width and height".into());
                }
                crate::text::markup::check(text)?;
                // Ranges count characters of the text as displayed: markup removed.
                let n = crate::text::markup::parse(text).0.chars().count();
                if let Some(r) = ranges.iter().find(|r| r.weight.is_some_and(|w| !(100..=900).contains(&w) || w % 100 != 0)) {
                    return Err(format!("range {}..{}: weight must be 100, 200, … 900", r.start, r.end));
                }
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

/// Checks positions, sizes and the stack-child fields.
fn check_lengths(l: &Layer) -> Result<(), String> {
    for (name, pos) in [("x", l.x), ("y", l.y)] {
        if matches!(pos, Length::Hug | Length::Fill) {
            return Err(format!("{name} must be px or a percentage"));
        }
    }
    for len in [l.width, l.height].into_iter().flatten() {
        match len {
            Length::Px(v) | Length::Pct(v) if v < 0.0 => {
                return Err("width and height must be >= 0".into());
            }
            _ => {}
        }
    }
    let clamps = [l.min_width, l.max_width, l.min_height, l.max_height];
    if clamps.iter().flatten().any(|v| *v < 0.0) {
        return Err("minWidth, maxWidth, minHeight and maxHeight must be >= 0".into());
    }
    if let (Some(lo), Some(hi)) = (l.min_width, l.max_width)
        && lo > hi
    {
        return Err("minWidth must be <= maxWidth".into());
    }
    if let (Some(lo), Some(hi)) = (l.min_height, l.max_height)
        && lo > hi
    {
        return Err("minHeight must be <= maxHeight".into());
    }
    if l.aspect_ratio.is_some_and(|r| r <= 0.0) {
        return Err("aspectRatio must be > 0".into());
    }
    if l.grow.is_some_and(|g| g < 0.0) {
        return Err("flexGrow must be >= 0".into());
    }
    if l.inset.is_some() && l.place.is_none() {
        return Err("margin needs place; in a stack, space children with gap or padding".into());
    }
    if let Kind::Spacer { min_length } = l.kind
        && min_length < 0.0
    {
        return Err("minLength must be >= 0".into());
    }
    Ok(())
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
    fn clips_and_transitions_refuse_negative_times() {
        let s: Scene = serde_json::from_value(json!({
            "width": 100, "height": 100, "sizes": [{"id": "a", "width": 100, "height": 100}],
            "assets": {"v": {"sha256": "c", "width": 10, "height": 10}},
            "layers": [{"id": "v", "type": "video", "asset": "v", "delay": -1}]
        }))
        .unwrap();
        assert!(s.validate().unwrap_err().contains("delay are 0 s or more"));
        let s: Scene = serde_json::from_value(json!({
            "width": 100, "height": 100, "sizes": [{"id": "a", "width": 100, "height": 100}],
            "layers": [{"id": "a", "type": "frame", "shot": {"duration": 1}},
                       {"id": "b", "type": "frame", "shot": {"duration": 1,
                        "transition": {"type": "fade", "duration": -0.5}}}]
        }))
        .unwrap();
        assert!(s.validate().unwrap_err().contains("0 s or more"));
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
