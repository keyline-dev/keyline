//! Resolves every layer's box for one target size, following the usual
//! design-tool rules: the Scale tool first (geometry and font sizes × `scale`), then a frame resize
//! from the scaled master to the target, applying each layer's constraints
//! against its parent, recursively.

mod stack;
#[cfg(test)]
mod tests;

use skia_safe::textlayout::Paragraph;

use crate::scene::{Kind, Layer, Pin, Resize, Scene, Size};
use crate::text::{Fit, Text};

use stack::{hug, place_stack};

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
