//! The server's verification, so the agent can check a design without
//! looking at pixels. Three kinds of output:
//!
//! - defects, `!truncated` etc.: objectively broken, always fix;
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
use crate::scene::{Fit, Kind, Position, Scene};

use contrast::contrast;
use overlap::{ink, overlaps};

pub use facts::{facts, scale_hints, text_report};

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
            .map(|(id, a)| {
                if let (Some(c), 0.0) = (a.clip, a.width) {
                    return format!("{id} sound {}s", (c.duration * 10.0).round() / 10.0);
                }
                let clip = a.clip.map_or_else(String::new, |c| {
                    let sound = if c.audio { " sound" } else { "" };
                    format!(" {}s{sound}", (c.duration * 10.0).round() / 10.0)
                });
                format!("{id} {}×{}{clip}", n(a.width), n(a.height))
            })
            .collect();
        let _ = writeln!(out, "assets {}", list.join(", "));
    }
    for size in sizes {
        let sized = &*scene.for_size(size);
        for (view, t, first) in views(sized) {
            let scene = &view;
            let placed = layout(scene, size);
            // A backdrop that fails to render (e.g. an asset missing from the
            // store) only skips the contrast check; `render` reports the error.
            let backdrop = assets.and_then(|dir| {
                // Text is read against the video under it, not an empty canvas;
                // without ffmpeg there is no backdrop and no contrast check.
                if crate::video::frame::has_video(scene) {
                    let (at, frames) = crate::video::frame::still(scene, size, t, dir).ok()?;
                    crate::render::render_image_with(&at, size, 1.0, dir, true, frames).ok()
                } else {
                    render_image(scene, size, 1.0, dir, true).ok()
                }
            });
            // What knockout letters show: the scene without the frames they
            // cut through.
            let knocked = knockout_frames(&placed);
            let through = if knocked.is_empty() || crate::video::frame::has_video(scene) {
                None
            } else {
                assets.and_then(|dir| {
                    let mut open = scene.clone();
                    hide(&mut open.layers, &knocked);
                    render_image(&open, size, 1.0, dir, true).ok()
                })
            };
            let [st, sr, sb, sl] = size.safe;
            let checks = Checks {
                scene,
                safe: (size.safe != [0.0; 4]).then_some(Rect {
                    x: sl,
                    y: st,
                    w: size.width - sl - sr,
                    h: size.height - st - sb,
                }),
                overlaps: overlaps(&placed),
                backdrop: backdrop.as_ref().and_then(skia_safe::Image::peek_pixels),
                through: through.as_ref().and_then(skia_safe::Image::peek_pixels),
            };
            let clip = Clip {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: size.width,
                    h: size.height,
                },
                by: "canvas",
                quiet: false,
            };
            let mut lines = Vec::new();
            // After the first shot, only the shot is new: the layers around it
            // were listed with the first.
            for p in placed
                .iter()
                .filter(|p| first || p.layer.time.shot.is_some())
            {
                line(
                    &mut lines,
                    &checks,
                    p,
                    clip,
                    1,
                    (1.0, &skia_safe::Matrix::new_identity()),
                );
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
    }
    if out.is_empty() {
        out.push_str("ok");
    }
    Ok(out)
}

/// The scene as checked: whole, or once per shot with only that shot
/// shown, each with a moment it's fully on screen (its transition in done)
/// and whether it's the first view.
fn views(scene: &Scene) -> Vec<(Scene, f32, bool)> {
    let shots = crate::anim::shots::timeline(scene);
    if shots.is_empty() {
        return vec![(scene.clone(), 0.0, true)];
    }
    shots
        .iter()
        .enumerate()
        .map(|(n, &(i, start, _, into))| {
            let mut view = scene.clone();
            for &(j, ..) in &shots {
                view.layers[j].hidden = j != i;
            }
            let settled = if n == 0 {
                0.0
            } else {
                start + into.map_or(0.0, crate::anim::shots::Transition::overlap)
            };
            (view, settled, n == 0)
        })
        .collect()
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
    /// The part of the canvas the platform doesn't cover, when it covers any.
    safe: Option<Rect>,
    /// Text ids whose ink overlaps other texts' ink.
    overlaps: HashMap<&'s str, Vec<&'s str>>,
    /// The size rendered without text: what each text is read against.
    backdrop: Option<Pixmap<'i>>,
    /// The size without text or the frames knockout text cuts through:
    /// what shows in its letters. `None` when there's no knockout text.
    through: Option<Pixmap<'i>>,
}

/// Ids of the frames whose knockout text cuts through them.
fn knockout_frames(placed: &[Placed]) -> Vec<String> {
    let mut out = Vec::new();
    for p in placed {
        if p.children
            .iter()
            .any(|c| matches!(&c.layer.kind, Kind::Text { more, .. } if more.knockout))
        {
            out.push(p.layer.id.clone());
        }
        out.extend(knockout_frames(&p.children));
    }
    out
}

/// Hides the layers with these ids, wherever they are.
fn hide(layers: &mut [crate::scene::Layer], ids: &[String]) {
    for l in layers {
        if ids.contains(&l.id) {
            l.hidden = true;
        }
        if let Some(children) = l.kind.children_mut() {
            hide(children, ids);
        }
    }
}

/// The visible area: the canvas intersected with every clipping ancestor,
/// and the innermost of those, named in `!clipped` warnings.
#[derive(Clone, Copy)]
struct Clip<'a> {
    rect: Rect,
    by: &'a str,
    /// Inside a stack already reported `!overflow`: its children falling
    /// outside it are that one problem, not one each.
    quiet: bool,
}

/// `opacity` is the product of the ancestors' opacities, `m` their visual
/// transforms.
fn line(
    lines: &mut Vec<String>,
    checks: &Checks,
    p: &Placed,
    clip: Clip,
    depth: usize,
    (opacity, m): (f32, &skia_safe::Matrix),
) {
    let visible = clip.rect;
    let r = p.rect;
    let l = p.layer;
    let opacity = opacity * l.opacity;
    let m = &overlap::through(m, p);
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
    // What an adaptive layout picked here: a stack's direction, a firstFit's child.
    if let Some((w, h)) = p.overflow {
        let _ = write!(out, " !overflow needs {}×{}", n(w), n(h));
    }
    if let Some(c) = &p.chosen {
        let _ = write!(out, " → {c}");
    }
    match &l.kind {
        Kind::Text {
            font_size,
            max_lines,
            ..
        } => {
            let max = font_size * p.k;
            if let Some((_, fit)) = &p.text {
                let _ = write!(out, " {}px", n(fit.font_size));
                if n(max) != n(fit.font_size) {
                    let _ = write!(out, " (max {})", n(max));
                }
                if fit.lines > 1 {
                    let _ = write!(out, " {}L", fit.lines);
                }
                // Text that doesn't fit its box, cut or not: one marker.
                // Cut at `maxLines`, the fix is more lines or a wider box.
                if fit.truncated && max_lines.is_some_and(|m| fit.lines >= m) {
                    let _ = write!(out, " !truncated at maxLines {}", fit.lines);
                    // With a height too, text shrinks to fit before it's cut.
                    if l.height.is_none() {
                        out.push_str(" (a height lets it shrink instead)");
                    }
                } else if fit.truncated || fit.overflow {
                    let _ = write!(
                        out,
                        " !truncated needs {}×{} (one line: {} wide)",
                        n(r.w),
                        n(fit.need_height),
                        n(fit.one_line_width)
                    );
                }
            }
            if let (Some((para, _)), Some(t)) = (&p.text, crate::text::Text::of(l, p.k))
                && let Some(word) = t.broken_word(para)
            {
                let _ = write!(
                    out,
                    " !breaks \"{word}\" (needs {} wide)",
                    n(para.min_intrinsic_width().ceil())
                );
            }
            if checks.safe.is_some_and(|safe| !contains(safe, r)) {
                out.push_str(" !unsafe");
            }
            if let Some(others) = checks.overlaps.get(l.id.as_str()) {
                let _ = write!(out, " !overlaps {}", others.join(","));
            }
            for (left, right) in crate::render::leader_clashes(p) {
                let short: String = left.chars().take(20).collect();
                let more = if short.len() < left.len() { "…" } else { "" };
                let _ = write!(out, " !leader \"{short}{more}\" meets \"{right}\"");
            }
            if let (Some(backdrop), Some(ink)) = (&checks.backdrop, ink(p))
                && let Some((ratio, min)) =
                    contrast(p, ink, (backdrop, checks.through.as_ref()), (opacity, m))
            {
                let _ = write!(out, " warn contrast {ratio:.1}:1 (WCAG {min})");
            }
        }
        kind if let Some((asset, fit, crop, tile_scale)) = kind.picture() => {
            if let Some(a) = checks.scene.assets.get(asset) {
                let (cw, ch) = image_crop(r, a.width, a.height, fit, crop);
                out.push_str(match fit {
                    Fit::Cover => " cover",
                    Fit::Contain => " contain",
                    Fit::Stretch => " fill",
                    Fit::Tile => " tile",
                });
                if cw >= 0.005 {
                    let _ = write!(out, " crop {}%w", (cw * 100.0).round());
                }
                if ch >= 0.005 {
                    let _ = write!(out, " crop {}%h", (ch * 100.0).round());
                }
                let up = image_scale(r, a.width, a.height, fit, crop, tile_scale * p.k);
                if !a.svg && up > 1.005 {
                    let _ = write!(out, " upscaled {up:.1}x");
                }
                if fit == Fit::Cover && crop.is_none() {
                    crop_warning(&mut out, (cw, ch), kind.focus().unwrap_or([0.5, 0.5]));
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
    // Pushed out of an overflowing stack: reported once, on the stack. A
    // spacer draws nothing, so an empty one hides nothing.
    if clip.quiet {
    } else if intersect(shown, visible).is_none() && !matches!(l.kind, Kind::Spacer { .. }) {
        out.push_str(" !hidden");
    } else if let Some(r) = text_cut(p, visible) {
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
    } else if clip.by != "canvas" && shadow_cut(p, visible) {
        let _ = write!(out, " warn shadow clipped by {}", clip.by);
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
            quiet: clip.quiet,
        },
        _ => clip,
    };
    for c in &p.children {
        let pushed = p.overflow.is_some() && c.layer.position != Position::Absolute;
        let own = Clip {
            quiet: inner.quiet || pushed,
            ..inner
        };
        line(lines, checks, c, own, depth + 1, (opacity, m));
    }
}

/// What of text `p` falls outside `visible`: its box, or its letters
/// where they reach past the box (tall caps at a tight `lineHeight` in a
/// frame that clips). `None` when all of it shows. Ink gets a pixel of
/// slack for antialiasing.
fn text_cut(p: &Placed, visible: Rect) -> Option<Rect> {
    if !matches!(p.layer.kind, Kind::Text { .. }) {
        return None;
    }
    let r = p.rect;
    let ink = ink(p).map(|i| Rect {
        x: i.x + 1.0,
        y: i.y + 1.0,
        w: (i.w - 2.0).max(0.0),
        h: (i.h - 2.0).max(0.0),
    });
    let shown = ink.map_or(r, |i| {
        let (x, y) = (r.x.min(i.x), r.y.min(i.y));
        Rect {
            x,
            y,
            w: r.right().max(i.right()) - x,
            h: r.bottom().max(i.bottom()) - y,
        }
    });
    (!contains(visible, shown)).then_some(shown)
}

/// `warn crop` when a cover crop hides more than half the image on an
/// axis: the window then can't hold the half of the image around the
/// focus (the focus point itself always stays in view), so a subject there
/// is cut, e.g. a house in a band too short for it.
fn crop_warning(out: &mut String, (cw, ch): (f32, f32), focus: [f32; 2]) {
    let (share, bigger) = if ch >= cw {
        (ch, "taller")
    } else {
        (cw, "wider")
    };
    if share <= 0.505 {
        return;
    }
    let what = if focus == [0.5, 0.5] {
        "the image's middle"
    } else {
        "the area around its focus"
    };
    let _ = write!(
        out,
        " warn crop cuts {what} (focus {}%,{}%): a {bigger} box keeps more",
        (focus[0] * 100.0).round(),
        (focus[1] * 100.0).round()
    );
}

/// Whether a frame that clips its content cuts `p`'s drop shadow: where
/// its offset, spread and half its blur reach, so a soft tail fading
/// against the edge doesn't count. Text shadows follow the letters and
/// aren't judged.
fn shadow_cut(p: &Placed, visible: Rect) -> bool {
    if matches!(p.layer.kind, Kind::Text { .. }) {
        return false;
    }
    let r = p.rect;
    p.layer
        .look
        .shadows
        .iter()
        .flat_map(crate::scene::OneOrMany::as_slice)
        .filter(|s| !s.inset && s.color.0 >> 24 != 0)
        .any(|s| {
            let grow = (s.spread + s.blur / 2.0).max(0.0) * p.k;
            let reach = Rect {
                x: r.x + s.x * p.k - grow,
                y: r.y + s.y * p.k - grow,
                w: r.w + 2.0 * grow,
                h: r.h + 2.0 * grow,
            };
            !contains(visible, reach)
        })
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
