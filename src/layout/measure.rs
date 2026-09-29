//! A layer's size before its parent places it: fixed sides first (px,
//! `%`, `fill`, or what a stack imposes), then the content decides the rest,
//! then `aspectRatio`, then min/max.

use crate::scene::{Kind, Layer, Length, Resize, Scene};
use crate::text::Text;

use super::{first_fit, grid, stack};

/// Frames, rects and ellipses without a size are 100 × 100.
pub(super) const DEFAULT_BOX: f32 = 100.0;

/// A side a parent imposes (`Some`) or leaves to the layer (`None`).
pub(super) type Forced = (Option<f32>, Option<f32>);

/// The layer's size, scaled px, inside a parent whose inside is `parent`
/// (scaled px). `forced` sides win over the layer's own `width`/`height`.
/// In a stack's flow (`flow`), the stack resolves `fill` and `%`; a side it
/// leaves open sizes to the content.
pub(super) fn measure(
    scene: &Scene,
    layer: &Layer,
    k: f32,
    parent: (f32, f32),
    forced: Forced,
    flow: bool,
) -> (f32, f32) {
    let pos = (offset(layer.x, k, parent.0), offset(layer.y, k, parent.1));
    let own = |len: Option<Length>| {
        if flow {
            len.and_then(|l| l.px().map(Length::Px))
        } else {
            len
        }
    };
    let mut w = forced
        .0
        .or_else(|| fixed(own(layer.width), k, parent.0, pos.0));
    let mut h = forced
        .1
        .or_else(|| fixed(own(layer.height), k, parent.1, pos.1));
    if let Some(r) = layer.aspect_ratio.filter(|r| *r > 0.0) {
        match (w, h) {
            (Some(w0), None) => h = Some(w0 / r),
            (None, Some(h0)) => w = Some(h0 * r),
            _ => {}
        }
    }
    let size = match (w, h) {
        (Some(w), Some(h)) => (w, h),
        _ => {
            let (cw, ch) = content(scene, layer, k, parent, (w, h));
            (w.unwrap_or(cw), h.unwrap_or(ch))
        }
    };
    clamp(layer, k, size)
}

/// A position in px: `x` or `y`, as a px value or a share of the parent.
pub(super) fn offset(pos: Length, k: f32, parent: f32) -> f32 {
    match pos {
        Length::Pct(p) => p * parent,
        other => other.px().unwrap_or(0.0) * k,
    }
}

/// A side that doesn't depend on the content: px, `%` of the parent, or
/// `fill` (the rest of the parent after `pos`, in free layout).
fn fixed(len: Option<Length>, k: f32, parent: f32, pos: f32) -> Option<f32> {
    match len? {
        Length::Px(v) => Some(v * k),
        Length::Pct(p) => Some(p * parent),
        Length::Fill => Some((parent - pos).max(0.0)),
        Length::Hug => None,
    }
}

/// Applies `minWidth` … `maxHeight` (master px, scaled by `k`).
pub(super) fn clamp(layer: &Layer, k: f32, (w, h): (f32, f32)) -> (f32, f32) {
    let c = |v: f32, lo: Option<f32>, hi: Option<f32>| {
        let v = hi.map_or(v, |hi| v.min(hi * k));
        lo.map_or(v, |lo| v.max(lo * k))
    };
    (
        c(w, layer.min_width, layer.max_width),
        c(h, layer.min_height, layer.max_height),
    )
}

/// The size the content wants, given the sides already known.
fn content(scene: &Scene, layer: &Layer, k: f32, parent: (f32, f32), known: Forced) -> (f32, f32) {
    let (w, h) = known;
    let hug = |l: Option<Length>| l == Some(Length::Hug);
    match &layer.kind {
        Kind::Frame {
            children,
            layout: crate::scene::FrameLayout { stack: Some(s), .. },
            ..
        } => stack::hug(scene, children, s, k, known),
        Kind::Frame {
            children,
            layout: crate::scene::FrameLayout { grid: Some(g), .. },
            ..
        } => {
            let [t, r, b, l] = g.padding.sides().map(|p| p * k);
            let inner = (known.0.map(|w| w - l - r), known.1.map(|h| h - t - b));
            let c = grid::arrange(scene, children, g, k, inner).1;
            (
                known.0.unwrap_or(c.0 + l + r),
                known.1.unwrap_or(c.1 + t + b),
            )
        }
        // A frame of free children with no size wraps them, like a Figma
        // group; on an axis they don't size (empty, hidden, all `fill`), it's
        // a plain box.
        Kind::Frame { children, .. }
            if hug(layer.width)
                || hug(layer.height)
                || (!children.is_empty() && (layer.width.is_none() || layer.height.is_none())) =>
        {
            let wraps = |l: Option<Length>| hug(l) || (l.is_none() && !children.is_empty());
            let b = bounds(scene, children, k, (w.unwrap_or(0.0), h.unwrap_or(0.0)));
            (
                if wraps(layer.width) && b.0 > 0.0 {
                    b.0
                } else {
                    DEFAULT_BOX * k
                },
                if wraps(layer.height) && b.1 > 0.0 {
                    b.1
                } else {
                    DEFAULT_BOX * k
                },
            )
        }
        Kind::FirstFit { children } => first_fit::content(scene, children, k, parent, known).0,
        // Spacers take space only from a stack; `use` layers are expanded before layout.
        Kind::Spacer { .. } | Kind::Use { .. } => (0.0, 0.0),
        Kind::Text { .. } => text_content(layer, k, known),
        Kind::Image { asset, .. } | Kind::Video { asset, .. } => {
            let (iw, ih) = scene
                .assets
                .get(asset)
                .map_or((DEFAULT_BOX, DEFAULT_BOX), |a| (a.width, a.height));
            match (w, h) {
                // One side given: keep the image's aspect ratio.
                (Some(w), None) => (w, w * ih / iw),
                (None, Some(h)) => (h * iw / ih, h),
                _ => (iw * k, ih * k),
            }
        }
        Kind::Icon { name, set, .. } => {
            let a = crate::icons::aspect(*set, name).unwrap_or(1.0);
            let d = crate::icons::DEFAULT_SIZE * k;
            match (w, h) {
                (Some(w), None) => (w, w / a),
                (None, Some(h)) => (h * a, h),
                _ => (d * a, d),
            }
        }
        // A line's box is its run: a missing side is 0, not a default box.
        Kind::Line { .. } => (0.0, 0.0),
        Kind::Rect { .. }
        | Kind::Ellipse { .. }
        | Kind::Frame { .. }
        | Kind::Polygon { .. }
        | Kind::Path { .. } => (DEFAULT_BOX * k, DEFAULT_BOX * k),
    }
}

/// Text: auto-width measures one line; a known width wraps at it; a fixed
/// box (fit, fixed, truncate) is whatever its sides are. Padding adds to
/// the box; `trim: "cap"` takes the space above caps and below the baseline.
fn text_content(layer: &Layer, k: f32, (w, h): Forced) -> (f32, f32) {
    let Some(t) = Text::of(layer, k) else {
        return (0.0, 0.0);
    };
    let [pt, pr, pb, pl] = text_padding(layer, k);
    let trimmed = |width: f32, height: f32| {
        if !text_trims(layer) {
            return height;
        }
        let mut p = t.paragraph_at(width);
        p.layout(width);
        let (top, bottom) = t.cap_trim(&p, t.font_size());
        height - top - bottom
    };
    match (t.resize(), w) {
        (Resize::Fit | Resize::Fixed | Resize::Truncate, _) => {
            (w.unwrap_or(DEFAULT_BOX * k), h.unwrap_or(DEFAULT_BOX * k))
        }
        (_, Some(w)) => {
            let inner = (w - pl - pr).max(0.0);
            (w, trimmed(inner, t.height_at(inner)) + pt + pb)
        }
        (_, None) => {
            let (tw, th) = t.natural_size(0.0, 0.0);
            // Curved text also takes the arc's rise.
            let curve = match &layer.kind {
                Kind::Text { more, .. } => more
                    .curve
                    .map_or(0.0, |r| crate::render::curve_sagitta(tw, r * k)),
                _ => 0.0,
            };
            (tw + pl + pr, trimmed(tw, th) + curve + pt + pb)
        }
    }
}

/// A text layer's padding, scaled: `[top, right, bottom, left]`.
pub(super) fn text_padding(layer: &Layer, k: f32) -> [f32; 4] {
    match &layer.kind {
        Kind::Text { more, .. } => more.padding.sides().map(|p| p * k),
        _ => [0.0; 4],
    }
}

/// Whether a text layer trims to cap height.
pub(super) fn text_trims(layer: &Layer) -> bool {
    matches!(&layer.kind, Kind::Text { more, .. } if more.trim.is_some())
}

/// The box around a free frame's children, from its top-left corner.
fn bounds(scene: &Scene, children: &[Layer], k: f32, inside: (f32, f32)) -> (f32, f32) {
    children
        .iter()
        .filter(|c| !c.hidden)
        .fold((0.0_f32, 0.0_f32), |(r, b), c| {
            let (w, h) = measure(scene, c, k, inside, (None, None), false);
            let x = offset(c.x, k, inside.0);
            let y = offset(c.y, k, inside.1);
            (r.max(x + w), b.max(y + h))
        })
}
