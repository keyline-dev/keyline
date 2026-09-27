//! The server's verification, so the agent can check a design without
//! looking at pixels. Three kinds of output:
//!
//! - defects, `!overflow` etc.: objectively broken, always fix;
//! - advisories, `warn contrast`: a standard says it's weak; the agent decides;
//! - facts ([`facts`]): smallest text and image upscaling per size, with no
//!   threshold, because what's too small or too soft depends on the medium.
//!
//! `scene_describe` returns only warning lines by default (or `ok`); `full`
//! lists every layer in every size:
//!
//! ```text
//! portrait 1080×1350
//!  headline text 60,30 960×136 56px 2L
//!  photo image 0,180 1080×560 fill crop 17%w
//!  cta frame 0,900 1080×90
//!   label text 330,18 380×58 48px !clipped by cta: bottom 4px
//! ```

mod contrast;
mod facts;
mod overlap;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

use skia_safe::Pixmap;

use crate::layout::{Placed, Rect, layout};
use crate::render::{image_crop, image_scale, render_image};
use crate::scene::{Fit, Kind, Scene};

use contrast::contrast;
use overlap::{ink, overlaps};

pub use facts::{facts, text_report};

/// `assets` enables the contrast check, which renders each size without its
/// text and samples what lies behind every text. `None` skips it.
pub fn describe(
    scene: &Scene,
    size_id: Option<&str>,
    full: bool,
    assets: Option<&Path>,
) -> Result<String, String> {
    let sizes: Vec<_> = match size_id {
        Some(id) => vec![
            scene
                .sizes
                .iter()
                .find(|s| s.id == id)
                .ok_or_else(|| format!("no size {id}"))?,
        ],
        None => scene.sizes.iter().collect(),
    };
    let mut out = String::new();
    // The intrinsic sizes, so the agent can place images without cropping.
    if full && !scene.assets.is_empty() {
        let list: Vec<_> = scene
            .assets
            .iter()
            .map(|(id, a)| format!("{id} {}×{}", n(a.width), n(a.height)))
            .collect();
        let _ = writeln!(out, "assets {}", list.join(", "));
    }
    for size in sizes {
        let scene = &*scene.for_size(&size.id);
        let placed = layout(scene, size);
        // A backdrop that fails to render (e.g. an asset missing from the
        // store) only skips the contrast check; `render` reports the error.
        let backdrop = assets.and_then(|dir| render_image(scene, size, 1.0, dir, true).ok());
        let checks = Checks {
            scene,
            overlaps: overlaps(&placed),
            backdrop: backdrop.as_ref().and_then(skia_safe::Image::peek_pixels),
        };
        let clip = Clip {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: size.width,
                h: size.height,
            },
            by: "canvas",
        };
        let mut lines = Vec::new();
        for p in &placed {
            line(&mut lines, &checks, p, clip, 1, 1.0);
        }
        if full {
            let _ = writeln!(out, "{} {}×{}", size.id, n(size.width), n(size.height));
            for l in lines {
                let _ = writeln!(out, "{l}");
            }
        } else {
            for l in lines
                .iter()
                .filter(|l| l.contains(" !") || l.contains(" warn "))
            {
                let _ = writeln!(out, "{} {}", size.id, l.trim_start());
            }
        }
    }
    if out.is_empty() {
        out.push_str("ok");
    }
    Ok(out)
}

/// Defect and advisory lines for every size, or `None` when the design is clean.
/// Edit tools append these so the agent needn't call `scene_describe`.
pub fn warnings(scene: &Scene, assets: Option<&Path>) -> Option<String> {
    describe(scene, None, false, assets)
        .ok()
        .filter(|w| w != "ok")
}

/// Per-size context the warnings need beyond a single layer.
struct Checks<'s, 'i> {
    scene: &'s Scene,
    /// Text ids whose ink overlaps other texts' ink.
    overlaps: HashMap<&'s str, Vec<&'s str>>,
    /// The size rendered without text: what each text is read against.
    backdrop: Option<Pixmap<'i>>,
}

/// The visible area: the canvas intersected with every clipping ancestor,
/// and the innermost of those, named in `!clipped` warnings.
#[derive(Clone, Copy)]
struct Clip<'a> {
    rect: Rect,
    by: &'a str,
}

/// `opacity` is the product of the ancestors' opacities.
fn line(
    lines: &mut Vec<String>,
    checks: &Checks,
    p: &Placed,
    clip: Clip,
    depth: usize,
    opacity: f32,
) {
    let visible = clip.rect;
    let r = p.rect;
    let l = p.layer;
    let opacity = opacity * l.opacity;
    let mut out = String::new();
    let _ = write!(
        out,
        "{:depth$}{} {} {},{} {}×{}",
        "",
        l.id,
        l.kind.name(),
        n(r.x),
        n(r.y),
        n(r.w),
        n(r.h)
    );
    // ponytail: checks below use the unrotated box; fine for the small tilts
    // designs use, wrong for large angles.
    if l.rotation != 0.0 {
        let _ = write!(out, " rot {}°", n(l.rotation));
    }
    match &l.kind {
        Kind::Text { font_size, .. } => {
            let max = font_size * p.k;
            if let Some((_, fit)) = &p.text {
                let _ = write!(out, " {}px", n(fit.font_size));
                if n(max) != n(fit.font_size) {
                    let _ = write!(out, " (max {})", n(max));
                }
                if fit.lines > 1 {
                    let _ = write!(out, " {}L", fit.lines);
                }
                if fit.overflow {
                    let _ = write!(
                        out,
                        " !overflow needs {}×{} (one line: {} wide)",
                        n(r.w),
                        n(fit.need_height),
                        n(fit.one_line_width)
                    );
                }
                if fit.truncated {
                    out.push_str(" !truncated");
                }
            }
            if let Some(others) = checks.overlaps.get(l.id.as_str()) {
                let _ = write!(out, " !overlaps {}", others.join(","));
            }
            if let (Some(backdrop), Some(ink)) = (&checks.backdrop, ink(p))
                && let Some((ratio, min)) = contrast(p, ink, backdrop, opacity)
            {
                let _ = write!(out, " warn contrast {ratio:.1}:1 (WCAG {min})");
            }
        }
        Kind::Image {
            asset,
            fit,
            crop,
            tile_scale,
            ..
        } => {
            if let Some(a) = checks.scene.assets.get(asset) {
                let (cw, ch) = image_crop(r, a.width, a.height, *fit, crop.as_ref());
                out.push_str(match fit {
                    Fit::Fill => " fill",
                    Fit::Fit => " fit",
                    Fit::Tile => " tile",
                });
                if cw >= 0.005 {
                    let _ = write!(out, " crop {}%w", (cw * 100.0).round());
                }
                if ch >= 0.005 {
                    let _ = write!(out, " crop {}%h", (ch * 100.0).round());
                }
                let up = image_scale(r, a.width, a.height, *fit, crop.as_ref(), tile_scale * p.k);
                if !a.svg && up > 1.005 {
                    let _ = write!(out, " upscaled {up:.1}x");
                }
            }
        }
        _ => {}
    }
    // A line's box has no area; its stroke is what shows.
    let shown = match &l.kind {
        Kind::Line { stroke_width, .. } => {
            let t = stroke_width * p.k / 2.0;
            Rect {
                x: r.x - t,
                y: r.y - t,
                w: r.w + 2.0 * t,
                h: r.h + 2.0 * t,
            }
        }
        _ => r,
    };
    if intersect(shown, visible).is_none() {
        out.push_str(" !hidden");
    } else if matches!(l.kind, Kind::Text { .. }) && !contains(visible, r) {
        let _ = write!(out, " !clipped by {}:", clip.by);
        let cut = [
            ("left", visible.x - r.x),
            ("top", visible.y - r.y),
            ("right", r.right() - visible.right()),
            ("bottom", r.bottom() - visible.bottom()),
        ];
        for (side, px) in cut.iter().filter(|(_, px)| *px > 0.5) {
            let _ = write!(out, " {side} {}px", px.ceil());
        }
    }
    lines.push(out);
    let inner = match &l.kind {
        Kind::Frame { clip: true, .. } => Clip {
            rect: intersect(r, visible).unwrap_or(Rect {
                w: 0.0,
                h: 0.0,
                ..r
            }),
            by: &l.id,
        },
        _ => clip,
    };
    for c in &p.children {
        line(lines, checks, c, inner, depth + 1, opacity);
    }
}

fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    (right > x && bottom > y).then_some(Rect {
        x,
        y,
        w: right - x,
        h: bottom - y,
    })
}

/// Whether `inner` lies inside `outer`, with half a pixel of slack so
/// subpixel rounding isn't reported.
fn contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x - 0.5
        && inner.y >= outer.y - 0.5
        && inner.right() <= outer.right() + 0.5
        && inner.bottom() <= outer.bottom() + 0.5
}

/// Rounds to whole pixels for display.
fn n(v: f32) -> i64 {
    v.round() as i64
}
