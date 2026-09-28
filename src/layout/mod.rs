//! Resolves every layer's box for one target size, following the usual
//! design-tool rules: the Scale tool first (geometry and font sizes ×
//! `scale`), then a frame resize from the scaled master to the target,
//! applying each layer's constraints against its parent, recursively.
//! Stacks lay out their children like CSS flexbox; `firstFit` picks one.

mod first_fit;
mod flex;
mod grid;
mod measure;
mod stack;
#[cfg(test)]
mod tests;

use skia_safe::textlayout::Paragraph;

use crate::scene::{
    Align, Dirs, Inset, Kind, Layer, Length, Pin, Place, Position, Resize, Scene, Size, Spot,
    VAlign,
};
use crate::text::{Fit, Text};

use measure::{clamp, measure, offset};

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
    /// Placed children, for frames (and the chosen child of a `firstFit`).
    pub children: Vec<Placed<'a>>,
    /// What an adaptive layout chose at this size: a stack's direction
    /// when it had several, or the id of a `firstFit`'s child.
    pub chosen: Option<String>,
}

impl Placed<'_> {
    /// Top of the drawn text; see [`Placed::text_origin`].
    pub fn text_top(&self) -> f32 {
        self.text_origin().1
    }

    /// Where the paragraph is drawn: inside the padding, placed by
    /// `verticalAlign` (like `UILabel`, text in a box of its own size — fit,
    /// fixed, truncate — is centered, even when taller), shifted up by a cap
    /// trim, and across by alignment when balanced lines wrap narrower.
    pub fn text_origin(&self) -> (f32, f32) {
        let [pt, pr, pb, pl] = measure::text_padding(self.layer, self.k);
        let (x, y, w, h) = (
            self.rect.x + pl,
            self.rect.y + pt,
            self.rect.w - pl - pr,
            self.rect.h - pt - pb,
        );
        let Some((para, fit)) = &self.text else {
            return (x, y);
        };
        let (top, bottom) = if measure::text_trims(self.layer) {
            Text::of(self.layer, self.k).map_or((0.0, 0.0), |t| t.cap_trim(para, fit.font_size))
        } else {
            (0.0, 0.0)
        };
        let visible = para.height() - top - bottom;
        let fixed_box = matches!(
            self.layer.text_resize(),
            Some(Resize::Fit | Resize::Fixed | Resize::Truncate)
        );
        let valign = match &self.layer.kind {
            Kind::Text { more, .. } => more.vertical_align,
            _ => None,
        };
        let y0 = match valign.unwrap_or(if fixed_box {
            VAlign::Center
        } else {
            VAlign::Top
        }) {
            VAlign::Top => y,
            VAlign::Center => y + (h - visible) / 2.0,
            VAlign::Bottom => y + h - visible,
        };
        let spare = (w - fit.wrap_width).max(0.0);
        let dx = match self.layer.kind {
            Kind::Text { align, .. } => match align {
                Align::Center => spare / 2.0,
                Align::Right => spare,
                Align::Left | Align::Justify => 0.0,
            },
            _ => 0.0,
        };
        (x + dx, y0 - top)
    }
}

/// Places every layer of `scene` for one target `size`.
pub fn layout<'a>(scene: &'a Scene, size: &Size) -> Vec<Placed<'a>> {
    let k = size.scale;
    let old = (scene.width * k, scene.height * k);
    let new = (size.width, size.height);
    place_free(scene, &scene.layers, old, new, (0.0, 0.0), k)
}

/// Places children by their own position, size and constraints, as the
/// parent goes from `old` (scaled master) to `new`.
fn place_free<'a>(
    scene: &'a Scene,
    layers: &'a [Layer],
    old: (f32, f32),
    new: (f32, f32),
    origin: (f32, f32),
    k: f32,
) -> Vec<Placed<'a>> {
    layers
        .iter()
        .filter(|l| !l.hidden)
        .map(|layer| {
            let natural = measure(scene, layer, k, old, (None, None), false);
            let (hs, vs) = layer.place.map(Place::spots).unzip();
            let inset = layer.inset.map_or((0.0, 0.0), Inset::xy);
            let (x, w) = free_axis(
                FreeAxis {
                    pos: layer.x,
                    len: layer.width,
                    pin: layer.constraints.h.into(),
                    spot: hs,
                    inset: inset.0 * k,
                },
                natural.0,
                old.0,
                new.0,
                k,
            );
            let (y, h) = free_axis(
                FreeAxis {
                    pos: layer.y,
                    len: layer.height,
                    pin: layer.constraints.v.into(),
                    spot: vs,
                    inset: inset.1 * k,
                },
                natural.1,
                old.1,
                new.1,
                k,
            );
            let (w, h) = clamp(layer, k, (w, h));
            let rect = Rect {
                x: origin.0 + x,
                y: origin.1 + y,
                w,
                h,
            };
            finish(scene, layer, natural, rect, k, false)
        })
        .collect()
}

/// One axis of a free child: its position and size fields and how it's pinned.
#[derive(Clone, Copy)]
struct FreeAxis {
    pos: Length,
    len: Option<Length>,
    pin: Pin,
    spot: Option<Spot>,
    inset: f32,
}

/// Position and length on one axis when the parent goes from `old` to
/// `new`: constraints for px values, a share of the parent for `%`, the
/// rest of the parent for `fill`, and `place` wins over all of them.
fn free_axis(a: FreeAxis, natural: f32, old: f32, new: f32, k: f32) -> (f32, f32) {
    let pin = if matches!(a.pos, Length::Pct(_)) {
        Pin::Scale
    } else {
        a.pin
    };
    let (mut pos, mut len) = axis(pin, offset(a.pos, k, old), natural, old, new);
    match a.len {
        Some(Length::Pct(p)) => len = p * new,
        Some(Length::Fill) => len = (new - pos).max(0.0),
        _ => {}
    }
    if let Some(spot) = a.spot {
        pos = match spot {
            Spot::Start => a.inset,
            Spot::Middle => (new - len) / 2.0,
            Spot::End => new - len - a.inset,
        };
    }
    (pos, len)
}

/// Lays out a layer's text and children once its box is known. `natural`
/// is its box before any resize, which its children's constraints follow.
/// `sized` means a stack already set the box, text included.
fn finish<'a>(
    scene: &'a Scene,
    layer: &'a Layer,
    natural: (f32, f32),
    mut rect: Rect,
    k: f32,
    sized: bool,
) -> Placed<'a> {
    let (w, h) = natural;
    let [pt, pr, pb, pl] = measure::text_padding(layer, k);
    let text = Text::of(layer, k).map(|t| {
        if !sized {
            match t.resize() {
                // Auto-width text keeps its measured width wherever it's pinned.
                Resize::AutoWidth => {
                    if matches!(Pin::from(layer.constraints.h), Pin::Stretch | Pin::Scale)
                        && layer.place.is_none()
                    {
                        rect.x += (rect.w - w) / 2.0;
                    }
                    rect.w = w;
                    rect.h = h;
                }
                // Auto-height text rewraps at its new width and grows down.
                Resize::AutoHeight => {
                    rect.h = measure::measure(
                        scene,
                        layer,
                        k,
                        (rect.w, rect.h),
                        (Some(rect.w), None),
                        true,
                    )
                    .1;
                }
                Resize::Fit | Resize::Fixed | Resize::Truncate => {}
            }
        }
        t.layout((rect.w - pl - pr).max(0.0), (rect.h - pt - pb).max(0.0))
    });
    let (children, chosen) = match &layer.kind {
        Kind::Frame {
            children,
            stack,
            grid,
            ..
        } if stack.is_some() || grid.is_some() => {
            let padding = stack.as_ref().map_or_else(
                || grid.as_ref().map(|g| g.padding).unwrap_or_default(),
                |s| s.padding,
            );
            let [t, r, b, l] = padding.sides().map(|p| p * k);
            let inner = (rect.w - l - r, rect.h - t - b);
            let (items, chosen) = match (stack, grid) {
                (Some(s), _) => {
                    let a = stack::choose(scene, children, s, k, (Some(inner.0), Some(inner.1)));
                    let chosen =
                        matches!(s.dir, Dirs::FirstFit(_)).then(|| a.dir.name().to_owned());
                    (a.items, chosen)
                }
                (None, Some(g)) => (
                    grid::arrange(scene, children, g, k, (Some(inner.0), Some(inner.1))).0,
                    None,
                ),
                (None, None) => (Vec::new(), None),
            };
            let mut placed: Vec<Placed> = items
                .iter()
                .map(|it| {
                    let r = Rect {
                        x: rect.x + l + it.pos.0,
                        y: rect.y + t + it.pos.1,
                        w: it.size.0,
                        h: it.size.1,
                    };
                    let own = measure(scene, it.layer, k, inner, (None, None), true);
                    finish(scene, it.layer, own, r, k, true)
                })
                .collect();
            // Absolute children sit on the frame like free children.
            placed.extend(
                children
                    .iter()
                    .filter(|c| c.position == Position::Absolute)
                    .flat_map(|c| {
                        place_free(
                            scene,
                            std::slice::from_ref(c),
                            (w, h),
                            (rect.w, rect.h),
                            (rect.x, rect.y),
                            k,
                        )
                    }),
            );
            (placed, chosen)
        }
        Kind::Frame { children, .. } => (
            place_free(
                scene,
                children,
                (w, h),
                (rect.w, rect.h),
                (rect.x, rect.y),
                k,
            ),
            None,
        ),
        Kind::FirstFit { children } => {
            let (_, i) = first_fit::content(
                scene,
                children,
                k,
                (rect.w, rect.h),
                (Some(rect.w), Some(rect.h)),
            );
            let chosen = children.get(i).map(|c| c.id.clone());
            let placed = children
                .get(i)
                .map(|c| {
                    place_free(
                        scene,
                        std::slice::from_ref(c),
                        (rect.w, rect.h),
                        (rect.w, rect.h),
                        (rect.x, rect.y),
                        k,
                    )
                })
                .unwrap_or_default();
            (placed, chosen)
        }
        _ => (Vec::new(), None),
    };
    Placed {
        layer,
        rect,
        k,
        text,
        children,
        chosen,
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
