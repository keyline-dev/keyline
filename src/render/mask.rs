//! Masks: after a layer is drawn, keep only what its mask shows. The mask
//! (a gradient, a shape, a path, another layer or an image) is drawn over
//! it with `DstIn` (`DstOut` when inverted); luminance masks turn
//! brightness into alpha first.

use anyhow::{Result, anyhow};
use skia_safe::{BlendMode, ColorFilter, Matrix, Path, canvas::SaveLayerRec, matrix::ScaleToFit};

use super::Ctx;
use super::fills::gradient_shader;
use super::paint::sk_rect;
use crate::layout::Placed;
use crate::scene::{Color, Fit, Mask, MaskMode, MaskSource};

impl Ctx<'_> {
    /// Applies layer `p`'s mask to what's been drawn in its group; `root` is
    /// the whole placed tree, where a mask layer is looked up.
    pub(super) fn apply_mask(
        &mut self,
        canvas: &skia_safe::Canvas,
        p: &Placed,
        m: &Mask,
        root: &[Placed],
    ) -> Result<()> {
        let r = sk_rect(p.rect);
        let mut mask = skia_safe::Paint::default();
        mask.set_anti_alias(true);
        mask.set_blend_mode(if m.invert {
            BlendMode::DstOut
        } else {
            BlendMode::DstIn
        });
        if m.mode == MaskMode::Luminance {
            mask.set_color_filter(ColorFilter::luma());
        }
        match &m.source {
            MaskSource::Gradient(g) => {
                // Alpha mode keeps the stops' alpha only.
                let mut g = g.clone();
                if m.mode == MaskMode::Alpha {
                    for s in &mut g.stops {
                        s.color = Color(s.color.0 & 0xFF00_0000);
                    }
                }
                if let Some(sh) = gradient_shader(&g, r) {
                    mask.set_shader(sh);
                    canvas.draw_rect(r, &mask);
                }
            }
            MaskSource::Shape(name) => {
                let path =
                    named_path(name, r).ok_or_else(|| anyhow!("unknown mask shape {name}"))?;
                coverage(canvas, &mask, |c| c.draw_path(&path, &opaque()));
            }
            MaskSource::Path(d) => {
                let path = fitted_path(d, r).ok_or_else(|| anyhow!("bad mask path"))?;
                coverage(canvas, &mask, |c| c.draw_path(&path, &opaque()));
            }
            MaskSource::Image(id) => {
                let img = self.image_paint(id, p.rect, Fit::Cover, None, 1.0, [0.5, 0.5])?;
                coverage(canvas, &mask, |c| c.draw_rect(r, &img));
            }
            MaskSource::Layer(id) => {
                let source = find(root, id).ok_or_else(|| anyhow!("mask layer {id} not found"))?;
                // The mask layer composites onto this one as a whole.
                canvas.save_layer(&SaveLayerRec::default().paint(&mask));
                let was = std::mem::replace(&mut self.drawing_mask, true);
                let result = self.draw(canvas, source, root);
                self.drawing_mask = was;
                canvas.restore();
                result?;
            }
        }
        Ok(())
    }
}

/// A shape by name, in box `r`: `rect`, `ellipse` (or `circle`), or a named shape.
pub(super) fn named_path(name: &str, r: skia_safe::Rect) -> Option<Path> {
    match name {
        "rect" => Some(Path::rect(r, None)),
        "ellipse" | "circle" => Some(Path::oval(r, None)),
        other => crate::shapes::path(other).and_then(|d| fitted_path(d, r)),
    }
}

/// An SVG path scaled from its own bounds to fit box `r`, centered.
pub(super) fn fitted_path(d: &str, r: skia_safe::Rect) -> Option<Path> {
    let path = Path::from_svg(d)?;
    let m = Matrix::rect_2_rect(path.bounds(), r, ScaleToFit::Center)?;
    Some(path.with_transform(&m))
}

/// The placed layer with `id`, anywhere in the tree.
fn find<'t, 'a>(placed: &'t [Placed<'a>], id: &str) -> Option<&'t Placed<'a>> {
    placed.iter().find_map(|p| {
        if p.layer.id == id {
            Some(p)
        } else {
            find(&p.children, id)
        }
    })
}

/// Composites what `draw` covers onto the layer with the mask paint, over
/// the whole layer: what it leaves transparent is hidden (shown, inverted).
fn coverage(
    canvas: &skia_safe::Canvas,
    mask: &skia_safe::Paint,
    draw: impl FnOnce(&skia_safe::Canvas) -> &skia_safe::Canvas,
) {
    canvas.save_layer(&SaveLayerRec::default().paint(mask));
    draw(canvas);
    canvas.restore();
}

fn opaque() -> skia_safe::Paint {
    let mut p = skia_safe::Paint::default();
    p.set_anti_alias(true);
    p
}
