//! Resolves every layer's box for one target size, following the usual
//! design-tool rules: the Scale tool first (geometry and font sizes × `scale`), then a frame resize
//! from the scaled master to the target, applying each layer's constraints
//! against its parent, recursively.

use skia_safe::textlayout::Paragraph;

use crate::scene::{Dir, Justify, Kind, Layer, Pin, Resize, Scene, Size, Stack, StackAlign};
use crate::text::{Fit, Text};

/// Frames and rects without a size default to 100 × 100.
const DEFAULT_BOX: f32 = 100.0;

/// An axis-aligned box in canvas pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Rect {
    /// Right edge.
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    /// Bottom edge.
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

/// A layer with its final box in canvas coordinates.
pub struct Placed<'a> {
    /// The scene layer.
    pub layer: &'a Layer,
    /// Its unrotated box, canvas coordinates.
    pub rect: Rect,
    /// Scale-tool factor, for font sizes and corner radii.
    pub k: f32,
    /// The laid-out paragraph and how it fits, for text layers.
    pub text: Option<(Paragraph, Fit)>,
    /// Placed children, for frames.
    pub children: Vec<Placed<'a>>,
}

impl Placed<'_> {
    /// Top of the drawn text. Like `UILabel`, text in a box of its own size
    /// (fit, fixed, truncate) is centered vertically in it, even when taller.
    pub fn text_top(&self) -> f32 {
        let fixed_box = matches!(
            self.layer.text_resize(),
            Some(Resize::Fit | Resize::Fixed | Resize::Truncate)
        );
        match &self.text {
            Some((para, _)) if fixed_box => self.rect.y + (self.rect.h - para.height()) / 2.0,
            _ => self.rect.y,
        }
    }
}

/// Places every layer of `scene` for one target `size`.
pub fn layout<'a>(scene: &'a Scene, size: &Size) -> Vec<Placed<'a>> {
    let k = size.scale;
    let old = (scene.width * k, scene.height * k);
    let new = (size.width, size.height);
    place(scene, &scene.layers, old, new, (0.0, 0.0), k)
}

fn place<'a>(
    scene: &'a Scene,
    layers: &'a [Layer],
    old: (f32, f32),
    new: (f32, f32),
    origin: (f32, f32),
    k: f32,
) -> Vec<Placed<'a>> {
    layers
        .iter()
        .map(|layer| {
            let text = Text::of(layer, k);
            let natural = natural_size(scene, layer, k, text.as_ref());
            let (x, w) = axis(
                layer.constraints.h.into(),
                layer.x * k,
                natural.0,
                old.0,
                new.0,
            );
            let (y, h) = axis(
                layer.constraints.v.into(),
                layer.y * k,
                natural.1,
                old.1,
                new.1,
            );
            let rect = Rect {
                x: origin.0 + x,
                y: origin.1 + y,
                w,
                h,
            };
            finish(scene, layer, text, natural, rect, k)
        })
        .collect()
}

/// Lays out a layer's text and children once its box is known. `natural`
/// is its box before any resize, which its children's constraints follow.
fn finish<'a>(
    scene: &'a Scene,
    layer: &'a Layer,
    text: Option<Text<'_>>,
    natural: (f32, f32),
    mut rect: Rect,
    k: f32,
) -> Placed<'a> {
    let (w, h) = natural;
    let text = text.map(|t| {
        match t.resize() {
            // Auto-width text keeps its measured width wherever it's pinned.
            Resize::AutoWidth => {
                if matches!(Pin::from(layer.constraints.h), Pin::Stretch | Pin::Scale) {
                    rect.x += (rect.w - w) / 2.0;
                }
                rect.w = w;
                rect.h = h;
            }
            // Auto-height text rewraps at its new width and grows down.
            Resize::AutoHeight => rect.h = t.natural_size(rect.w, 0.0).1,
            Resize::Fit | Resize::Fixed | Resize::Truncate => {}
        }
        t.layout(rect.w, rect.h)
    });
    let children = match &layer.kind {
        Kind::Frame {
            children,
            stack: Some(stack),
            ..
        } => place_stack(scene, children, stack, rect, k),
        Kind::Frame { children, .. } => place(
            scene,
            children,
            (w, h),
            (rect.w, rect.h),
            (rect.x, rect.y),
            k,
        ),
        _ => Vec::new(),
    };
    Placed {
        layer,
        rect,
        k,
        text,
        children,
    }
}

/// Places a stack's children one after another inside `frame`.
fn place_stack<'a>(
    scene: &'a Scene,
    children: &'a [Layer],
    stack: &Stack,
    frame: Rect,
    k: f32,
) -> Vec<Placed<'a>> {
    let row = stack.dir == Dir::Row;
    let (pad, mut gap) = (stack.padding * k, stack.gap * k);
    // (main, cross) of the frame's inside.
    let inner = if row {
        (frame.w - 2.0 * pad, frame.h - 2.0 * pad)
    } else {
        (frame.h - 2.0 * pad, frame.w - 2.0 * pad)
    };
    let sized: Vec<_> = children
        .iter()
        .map(|c| {
            let text = Text::of(c, k);
            let size = natural_size(scene, c, k, text.as_ref());
            (c, text, size)
        })
        .collect();
    let main_of = |(w, h): (f32, f32)| if row { w } else { h };
    let cross_of = |(w, h): (f32, f32)| if row { h } else { w };
    let n = sized.len() as f32;
    let used = sized.iter().map(|s| main_of(s.2)).sum::<f32>() + gap * (n - 1.0).max(0.0);
    let free = inner.0 - used;
    let mut at = match stack.justify {
        Justify::Start | Justify::Between => 0.0,
        Justify::Center => free / 2.0,
        Justify::End => free,
        Justify::Evenly => free / (n + 1.0),
    };
    match stack.justify {
        Justify::Between if n > 1.0 => gap += free / (n - 1.0),
        Justify::Evenly => gap += free / (n + 1.0),
        Justify::Start | Justify::Center | Justify::End | Justify::Between => {}
    }
    sized
        .into_iter()
        .map(|(c, text, size)| {
            let cross = match stack.align {
                StackAlign::Start => 0.0,
                StackAlign::Center => (inner.1 - cross_of(size)) / 2.0,
                StackAlign::End => inner.1 - cross_of(size),
            };
            let (dx, dy) = if row { (at, cross) } else { (cross, at) };
            at += main_of(size) + gap;
            let rect = Rect {
                x: frame.x + pad + dx,
                y: frame.y + pad + dy,
                w: size.0,
                h: size.1,
            };
            finish(scene, c, text, size, rect, k)
        })
        .collect()
}

// ponytail: hug and place_stack each measure the children, so nested
// hugging stacks re-measure per level; cache sizes if scenes get deep.
/// A stack's size when it hugs its children: `(main, cross)` summed and
/// maxed, plus gaps and padding, in scaled px.
fn hug(scene: &Scene, children: &[Layer], stack: &Stack, k: f32) -> (f32, f32) {
    let sizes: Vec<_> = children
        .iter()
        .map(|c| natural_size(scene, c, k, Text::of(c, k).as_ref()))
        .collect();
    let row = stack.dir == Dir::Row;
    let (main, cross) = sizes.iter().fold((0.0_f32, 0.0_f32), |(m, c), &(w, h)| {
        if row {
            (m + w, c.max(h))
        } else {
            (m + h, c.max(w))
        }
    });
    let gaps = stack.gap * k * (sizes.len() as f32 - 1.0).max(0.0);
    let pad = 2.0 * stack.padding * k;
    let (main, cross) = (main + gaps + pad, cross + pad);
    if row { (main, cross) } else { (cross, main) }
}

/// The layer's box at the scaled master size, before constraints.
fn natural_size(scene: &Scene, layer: &Layer, k: f32, text: Option<&Text>) -> (f32, f32) {
    if let Kind::Frame {
        children,
        stack: Some(stack),
        ..
    } = &layer.kind
        && (layer.width.is_none() || layer.height.is_none())
    {
        let (hw, hh) = hug(scene, children, stack, k);
        return (
            layer.width.map_or(hw, |w| w * k),
            layer.height.map_or(hh, |h| h * k),
        );
    }
    let (w, h) = match (&layer.kind, layer.width, layer.height) {
        (_, Some(w), Some(h)) => (w, h),
        (Kind::Image { asset, .. }, w, h) => {
            let (iw, ih) = scene
                .assets
                .get(asset)
                .map_or((DEFAULT_BOX, DEFAULT_BOX), |a| (a.width, a.height));
            match (w, h) {
                // One side given: keep the image's aspect ratio.
                (Some(w), None) => (w, w * ih / iw),
                (None, Some(h)) => (h * iw / ih, h),
                _ => (iw, ih),
            }
        }
        (Kind::Icon { name, set, .. }, w, h) => {
            let a = crate::icons::aspect(*set, name).unwrap_or(1.0);
            let d = crate::icons::DEFAULT_SIZE;
            match (w, h) {
                (Some(w), None) => (w, w / a),
                (None, Some(h)) => (h * a, h),
                _ => (d * a, d),
            }
        }
        // A line's box is its run: a missing side is 0, not a default box.
        (Kind::Line { .. }, w, h) => (w.unwrap_or(0.0), h.unwrap_or(0.0)),
        (_, w, h) => (w.unwrap_or(DEFAULT_BOX), h.unwrap_or(DEFAULT_BOX)),
    };
    match text {
        Some(t) => t.natural_size(w * k, h * k),
        None => (w * k, h * k),
    }
}

/// Applies one axis constraint when the parent goes from `old` to `new`.
/// Returns the new position and length.
fn axis(pin: Pin, pos: f32, len: f32, old: f32, new: f32) -> (f32, f32) {
    let d = new - old;
    match pin {
        Pin::Start => (pos, len),
        Pin::End => (pos + d, len),
        Pin::Center => (pos + d / 2.0, len),
        Pin::Stretch => (pos, (len + d).max(0.0)),
        Pin::Scale if old > 0.0 => (pos * new / old, len * new / old),
        Pin::Scale => (pos, len),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scene(layers: serde_json::Value) -> Scene {
        let mut v = json!({
            "width": 1000, "height": 500,
            "sizes": [{"id": "a", "width": 1000, "height": 500}],
            "assets": {"img": {"sha256": "x", "width": 400, "height": 200}},
        });
        v["layers"] = layers;
        serde_json::from_value(v).unwrap()
    }

    fn size(w: f32, h: f32, scale: f32) -> Size {
        Size {
            id: "t".into(),
            width: w,
            height: h,
            scale,
        }
    }

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn axis_follows_constraints() {
        // Parent grows 1000 → 1200 around a 100-wide child at x = 800.
        assert_eq!(
            axis(Pin::Start, 800.0, 100.0, 1000.0, 1200.0),
            (800.0, 100.0)
        );
        assert_eq!(
            axis(Pin::End, 800.0, 100.0, 1000.0, 1200.0),
            (1000.0, 100.0)
        );
        assert_eq!(
            axis(Pin::Center, 800.0, 100.0, 1000.0, 1200.0),
            (900.0, 100.0)
        );
        assert_eq!(
            axis(Pin::Stretch, 800.0, 100.0, 1000.0, 1200.0),
            (800.0, 300.0)
        );
        assert_eq!(
            axis(Pin::Scale, 800.0, 100.0, 1000.0, 1200.0),
            (960.0, 120.0)
        );
        assert_eq!(axis(Pin::Stretch, 0.0, 100.0, 1000.0, 800.0), (0.0, 0.0));
    }

    #[test]
    fn constraints_apply_on_resize() {
        let s = scene(json!([
            {"id": "a", "type": "rect", "x": 900, "y": 400, "width": 100, "height": 100,
             "constraints": {"h": "right", "v": "bottom"}},
            {"id": "b", "type": "rect", "x": 0, "y": 0, "width": 1000, "height": 50,
             "constraints": {"h": "stretch"}}
        ]));
        let p = layout(&s, &size(600.0, 600.0, 1.0));
        assert_eq!(p[0].rect, rect(500.0, 500.0, 100.0, 100.0));
        assert_eq!(p[1].rect, rect(0.0, 0.0, 600.0, 50.0));
    }

    #[test]
    fn scale_applies_before_constraints() {
        let s = scene(json!([
            {"id": "a", "type": "rect", "x": 900, "y": 400, "width": 100, "height": 100,
             "constraints": {"h": "right", "v": "bottom"}}
        ]));
        // Master scaled to 500 × 250, then resized to 300 × 600.
        let p = layout(&s, &size(300.0, 600.0, 0.5));
        assert_eq!(p[0].rect, rect(250.0, 550.0, 50.0, 50.0));
    }

    #[test]
    fn frames_constrain_children_relative_to_themselves() {
        let s = scene(json!([
            {"id": "f", "type": "frame", "x": 0, "y": 400, "width": 1000, "height": 100,
             "constraints": {"h": "stretch", "v": "bottom"},
             "children": [
                {"id": "c", "type": "rect", "x": 450, "y": 25, "width": 100, "height": 50,
                 "constraints": {"h": "center", "v": "center"}}
             ]}
        ]));
        let p = layout(&s, &size(1200.0, 600.0, 1.0));
        assert_eq!(p[0].rect, rect(0.0, 500.0, 1200.0, 100.0));
        assert_eq!(p[0].children[0].rect, rect(550.0, 525.0, 100.0, 50.0));
    }

    #[test]
    fn a_column_stack_hugs_and_centers_its_children() {
        let s = scene(json!([
            {"id": "col", "type": "frame", "x": 100, "y": 50,
             "stack": {"dir": "column", "gap": 10, "padding": 5, "align": "center"},
             "children": [
                {"id": "a", "type": "rect", "x": 999, "y": 999, "width": 200, "height": 40},
                {"id": "b", "type": "rect", "width": 100, "height": 20}
             ]}
        ]));
        let p = layout(&s, &size(1000.0, 500.0, 1.0));
        // Hugs: widest child + padding, heights + gap + padding.
        assert_eq!(p[0].rect, rect(100.0, 50.0, 210.0, 80.0));
        // x and y of children are ignored.
        assert_eq!(p[0].children[0].rect, rect(105.0, 55.0, 200.0, 40.0));
        assert_eq!(p[0].children[1].rect, rect(155.0, 105.0, 100.0, 20.0));
    }

    #[test]
    fn a_row_stack_spreads_children_and_follows_its_frame() {
        let s = scene(json!([
            {"id": "row", "type": "frame", "width": 1000, "height": 100,
             "constraints": {"h": "stretch"},
             "stack": {"dir": "row", "justify": "evenly", "align": "end"},
             "children": [
                {"type": "rect", "width": 100, "height": 50},
                {"type": "rect", "width": 100, "height": 100},
                {"type": "rect", "width": 100, "height": 50}
             ]}
        ]));
        let xs = |p: &[Placed]| {
            p[0].children
                .iter()
                .map(|c| (c.rect.x, c.rect.y))
                .collect::<Vec<_>>()
        };
        // 700 px free, shared by 4 gaps of 175.
        let p = layout(&s, &size(1000.0, 500.0, 1.0));
        assert_eq!(xs(&p), [(175.0, 50.0), (450.0, 0.0), (725.0, 50.0)]);
        // The frame stretches to 1200, so the gaps grow to 225.
        let p = layout(&s, &size(1200.0, 500.0, 1.0));
        assert_eq!(xs(&p), [(225.0, 50.0), (550.0, 0.0), (875.0, 50.0)]);
        let between = scene(json!([
            {"type": "frame", "width": 1000, "height": 100,
             "stack": {"dir": "row", "justify": "between"},
             "children": [{"type": "rect", "width": 100, "height": 10}, {"type": "rect", "width": 100, "height": 10}]}
        ]));
        let p = layout(&between, &size(1000.0, 500.0, 1.0));
        assert_eq!(p[0].children[1].rect.x, 900.0);
    }

    #[test]
    fn stacks_measure_text_and_scale_gaps() {
        let s = scene(json!([
            {"type": "frame", "stack": {"dir": "column", "gap": 20},
             "children": [
                {"id": "t", "type": "text", "text": "Hi", "fontSize": 40},
                {"type": "rect", "width": 10, "height": 10}
             ]}
        ]));
        let p = layout(&s, &size(500.0, 250.0, 0.5));
        let t = p[0].children[0].rect;
        assert!(t.w > 0.0 && t.h > 0.0);
        // The gap scales with the size: 20 × 0.5.
        assert_eq!(p[0].children[1].rect.y, t.h + 10.0);
    }

    #[test]
    fn images_default_to_intrinsic_size_and_keep_aspect() {
        let s = scene(json!([
            {"id": "a", "type": "image", "asset": "img"},
            {"id": "b", "type": "image", "asset": "img", "width": 100}
        ]));
        let p = layout(&s, &size(1000.0, 500.0, 1.0));
        assert_eq!(p[0].rect, rect(0.0, 0.0, 400.0, 200.0));
        assert_eq!(p[1].rect, rect(0.0, 0.0, 100.0, 50.0));
    }

    #[test]
    fn auto_width_text_keeps_width_when_stretched() {
        let s = scene(json!([
            {"id": "t", "type": "text", "text": "Hello", "x": 100, "fontSize": 40,
             "constraints": {"h": "stretch"}}
        ]));
        let a = layout(&s, &size(1000.0, 500.0, 1.0));
        let b = layout(&s, &size(1400.0, 500.0, 1.0));
        assert_eq!(a[0].rect.w, b[0].rect.w);
        assert_eq!(b[0].rect.x, 300.0);
    }

    #[test]
    fn auto_height_text_rewraps_when_narrowed() {
        let s = scene(json!([
            {"id": "t", "type": "text", "text": "one two three four five six seven", "fontSize": 30,
             "resize": "auto-height", "width": 1000, "constraints": {"h": "stretch"}}
        ]));
        let wide = layout(&s, &size(1000.0, 500.0, 1.0));
        let narrow = layout(&s, &size(300.0, 500.0, 1.0));
        assert_eq!(narrow[0].rect.w, 300.0);
        assert!(
            narrow[0].rect.h >= wide[0].rect.h * 2.0,
            "{} vs {}",
            narrow[0].rect.h,
            wide[0].rect.h
        );
    }

    #[test]
    fn boxed_text_is_centered_vertically_like_uilabel() {
        let s = scene(json!([
            {"id": "fit", "type": "text", "text": "Hi", "fontSize": 20, "width": 200, "height": 100},
            {"id": "auto", "type": "text", "text": "Hi", "fontSize": 20, "y": 300}
        ]));
        let p = layout(&s, &size(1000.0, 500.0, 1.0));
        let h = p[0].text.as_ref().unwrap().0.height();
        assert_eq!(p[0].text_top(), (100.0 - h) / 2.0);
        assert_eq!(p[1].text_top(), 300.0);
    }
}
