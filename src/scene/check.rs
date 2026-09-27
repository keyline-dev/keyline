//! Checks for paint, effects, masks and shapes: what serde can't express.

use super::{Gradient, Kind, Layer, MaskSource, Paint, Scene, Stroke};

/// A gradient needs 2+ stops, each at 0–1.
pub(super) fn gradient(g: &Gradient) -> Result<(), String> {
    if g.stops.len() < 2 || g.stops.iter().any(|s| !(0.0..=1.0).contains(&s.at)) {
        return Err("gradient needs at least 2 stops, each at 0–1".into());
    }
    if g.radius.iter().any(|r| *r <= 0.0) {
        return Err("gradient radius must be > 0".into());
    }
    Ok(())
}

/// A stroke needs a positive width; its dashes can't be negative.
pub(super) fn stroke(s: &Stroke) -> Result<(), String> {
    let widths = match s.width {
        super::StrokeWidth::All(w) => vec![w],
        super::StrokeWidth::Sides(w) => w.to_vec(),
    };
    if s.width.max() <= 0.0 || widths.iter().any(|w| *w < 0.0) {
        return Err("stroke width must be > 0".into());
    }
    if s.dash.iter().any(|d| *d < 0.0) {
        return Err("stroke dash lengths must be >= 0".into());
    }
    s.gradient.as_ref().map_or(Ok(()), gradient)
}

/// Fills, strokes, shadows, blur and transforms.
pub(super) fn look(scene: &Scene, l: &Layer) -> Result<(), String> {
    let look = &l.look;
    for f in look.fills.iter().flat_map(super::OneOrMany::as_slice) {
        paint(scene, f)?;
    }
    for s in look.strokes.iter().flat_map(super::OneOrMany::as_slice) {
        stroke(s)?;
    }
    if look
        .shadows
        .iter()
        .flat_map(super::OneOrMany::as_slice)
        .any(|s| s.blur < 0.0)
    {
        return Err("shadow blur must be >= 0".into());
    }
    if look.blur < 0.0 || look.backdrop_blur < 0.0 {
        return Err("blur and backdropBlur must be >= 0".into());
    }
    if look.scale <= 0.0 {
        return Err("scale must be > 0".into());
    }
    Ok(())
}

fn paint(scene: &Scene, p: &Paint) -> Result<(), String> {
    let c = p.common();
    if !(0.0..=1.0).contains(&c.opacity) {
        return Err("fill opacity must be 0–1".into());
    }
    match p {
        Paint::Gradient(g) => gradient(&g.gradient),
        Paint::Image(i) if !scene.assets.contains_key(&i.image) => {
            Err(format!("unknown asset {} in fills", i.image))
        }
        Paint::Image(i) if i.tile_scale <= 0.0 => Err("fill tileScale must be > 0".into()),
        Paint::Pattern(pt) if pt.size <= 0.0 => Err("pattern size must be > 0".into()),
        Paint::Noise(n) if !(0.0..=1.0).contains(&n.noise) => Err("noise must be 0–1".into()),
        _ => Ok(()),
    }
}

/// A mask's gradient, shape, path, layer or image must exist and parse.
pub(super) fn mask(scene: &Scene, l: &Layer) -> Result<(), String> {
    let Some(m) = &l.mask else { return Ok(()) };
    match &m.source {
        MaskSource::Gradient(g) => {
            gradient(g).map_err(|_| "mask needs at least 2 stops, each at 0–1".to_owned())
        }
        MaskSource::Shape(name) => match name.as_str() {
            "rect" | "ellipse" | "circle" => Ok(()),
            n if crate::shapes::path(n).is_some() => Ok(()),
            n => Err(format!(
                "unknown mask shape {n}; shapes: rect, ellipse, {}",
                crate::shapes::names().join(", ")
            )),
        },
        MaskSource::Path(d) => svg_path(d),
        MaskSource::Image(id) if !scene.assets.contains_key(id) => {
            Err(format!("unknown asset {id} in mask"))
        }
        MaskSource::Layer(id) => {
            let mut found = false;
            scene.walk(&mut |x| found |= x.id == *id);
            if !found {
                return Err(format!("mask layer {id} not found"));
            }
            if *id == l.id {
                return Err("a layer can't mask itself".into());
            }
            Ok(())
        }
        MaskSource::Image(_) => Ok(()),
    }
}

/// Shape types' own fields.
pub(super) fn shape(l: &Layer) -> Result<(), String> {
    match &l.kind {
        Kind::Polygon {
            sides,
            inner_radius,
            ..
        } => {
            if *sides < 3 {
                return Err("polygon sides must be >= 3".into());
            }
            if inner_radius.is_some_and(|r| !(0.0..=1.0).contains(&r)) {
                return Err("innerRadius must be 0–1".into());
            }
            Ok(())
        }
        Kind::Path { d, shape, .. } => match (d, shape) {
            (Some(d), None) => svg_path(d),
            (None, Some(s)) if crate::shapes::path(s).is_some() => Ok(()),
            (None, Some(s)) => Err(format!(
                "unknown shape {s}; shapes: {}",
                crate::shapes::names().join(", ")
            )),
            _ => Err("a path needs d or shape (not both)".into()),
        },
        Kind::Ellipse { arc: Some(a), .. } if !(0.0..1.0).contains(&a.inner) => {
            Err("arc inner must be 0–1".into())
        }
        _ => Ok(()),
    }
}

fn svg_path(d: &str) -> Result<(), String> {
    match skia_safe::Path::from_svg(d) {
        Some(p) if !p.bounds().is_empty() => Ok(()),
        _ => Err("bad SVG path d".into()),
    }
}
