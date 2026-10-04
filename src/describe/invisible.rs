//! The `warn invisible` advisory: a shape, line or icon that paints
//! nothing, every fill and stroke fully transparent or missing (`rgba(…, 0)`
//! meant as a fade, `fill: []` with no stroke).

use std::collections::HashSet;

use crate::scene::{
    Color, Gradient, Kind, Layer, Mask, MaskSource, Paint, Scene, Stroke, StrokeWidth,
};

/// Whether `l` draws nothing. Frames and spacers are left out (empty on
/// purpose), and so are a layer with a backdrop blur, which shows through
/// its shape, and one whose color animates.
pub(super) fn paints_nothing(l: &Layer) -> bool {
    let drawn = matches!(
        l.kind,
        Kind::Rect { .. }
            | Kind::Ellipse { .. }
            | Kind::Polygon { .. }
            | Kind::Path { .. }
            | Kind::Line { .. }
            | Kind::Icon { .. }
    );
    if !drawn || l.look.backdrop_blur > 0.0 || animates_color(l) {
        return false;
    }
    let fills = match (&l.kind, &l.look.fills) {
        (Kind::Icon { color, .. }, None) => vec![Paint::color(*color)],
        _ => crate::render::fills_of(l),
    };
    fills.iter().all(clear) && crate::render::strokes_of(l).iter().all(|s| unstroked(s))
}

/// The ids of layers another layer uses as its mask: they aren't drawn on
/// their own, so what they paint is only their alpha.
pub(super) fn masks(scene: &Scene) -> HashSet<String> {
    let mut ids = HashSet::new();
    scene.walk(&mut |l| {
        if let Some(Mask {
            source: MaskSource::Layer(id),
            ..
        }) = &l.mask
        {
            ids.insert(id.clone());
        }
    });
    ids
}

fn animates_color(l: &Layer) -> bool {
    l.time
        .animate
        .iter()
        .flat_map(crate::scene::OneOrMany::as_slice)
        .any(|t| t.props.contains_key("color"))
}

/// A paint that adds nothing: transparent or at opacity 0. An image,
/// pattern or noise always shows.
fn clear(p: &Paint) -> bool {
    match p {
        Paint::Solid(s) => s.opacity <= 0.0 || transparent(s.color),
        Paint::Gradient(g) => g.opacity <= 0.0 || all_transparent(&g.gradient),
        Paint::Image(_) | Paint::Pattern(_) | Paint::Noise(_) => false,
    }
}

/// A stroke with no width or a transparent paint (none is black).
fn unstroked(s: &Stroke) -> bool {
    let thin = match s.width {
        StrokeWidth::All(w) => w <= 0.0,
        StrokeWidth::Sides(sides) => sides.iter().all(|w| *w <= 0.0),
    };
    thin || s
        .gradient
        .as_ref()
        .map_or_else(|| s.color.is_some_and(transparent), all_transparent)
}

fn all_transparent(g: &Gradient) -> bool {
    g.stops.iter().all(|s| transparent(s.color))
}

fn transparent(c: Color) -> bool {
    c.0 >> 24 == 0
}

#[cfg(test)]
mod tests {
    use super::super::{describe, tests::scene};
    use serde_json::json;

    /// The problem lines for one layer, alone in the scene.
    fn checked(layer: &serde_json::Value) -> String {
        describe(&scene(json!([layer])), None, false, None).unwrap()
    }

    #[test]
    fn a_shape_that_paints_nothing_is_flagged_once_for_every_size() {
        // The divider an agent drew: its side lines at alpha 0, meant as a fade.
        let d = checked(
            &json!({"id": "rule", "type": "rect", "width": 100, "height": 2, "fill": "rgba(216,161,91,0)"}),
        );
        assert_eq!(
            d, "all rule warn invisible (no visible fill or stroke)\n",
            "{d}"
        );
        for layer in [
            json!({"id": "box", "type": "rect", "width": 100, "height": 50, "fill": []}),
            json!({"id": "dot", "type": "ellipse", "width": 20, "height": 20, "fill": {"color": "#FF0000", "opacity": 0}}),
            json!({"id": "fade", "type": "rect", "width": 100, "height": 50, "fill": "linear-gradient(90deg, #FFFFFF00, #00000000)"}),
            json!({"id": "hair", "type": "line", "width": 100, "height": 0, "stroke": "#00000000"}),
            json!({"id": "cup", "type": "icon", "name": "coffee", "width": 24, "height": 24, "color": "#D8A15B00"}),
        ] {
            let d = checked(&layer);
            assert!(d.contains(" warn invisible"), "{layer}: {d}");
        }
    }

    #[test]
    fn what_shows_or_is_empty_on_purpose_is_not_flagged() {
        for layer in [
            json!({"id": "card", "type": "frame", "width": 100, "height": 50, "children": []}),
            json!({"id": "gap", "type": "spacer", "width": 10, "height": 10}),
            json!({"id": "outline", "type": "rect", "width": 100, "height": 50, "fill": [], "stroke": "#000000"}),
            json!({"id": "glass", "type": "rect", "width": 100, "height": 50, "fill": "#FFFFFF00", "backdropBlur": 12}),
            json!({"id": "fadein", "type": "rect", "width": 100, "height": 50, "fill": "#FF000000", "animate": {"color": {"to": "#FF0000"}, "duration": 1}}),
            json!({"id": "photo", "type": "rect", "width": 100, "height": 50, "fill": {"image": "img"}}),
            json!({"id": "edge", "type": "rect", "width": 100, "height": 50, "fill": ["#FFFFFF00", "#000000"]}),
            json!({"id": "black", "type": "line", "width": 100, "height": 0}),
        ] {
            assert_eq!(checked(&layer), "ok", "{layer}");
        }
    }

    #[test]
    fn a_layer_used_as_a_mask_is_not_flagged() {
        // Not drawn on its own: only its alpha shapes the photo.
        let s = scene(json!([
            {"id": "blob", "type": "path", "shape": "blob-1", "width": 100, "height": 100},
            {"id": "photo", "type": "image", "asset": "img", "width": 100, "height": 100, "mask": {"layer": "blob"}}]));
        assert_eq!(describe(&s, None, false, None).unwrap(), "ok");
    }
}
