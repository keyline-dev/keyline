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
mod group;
mod overlap;
mod parts;
#[cfg(test)]
mod reply_tests;
#[cfg(test)]
mod tests;
mod views;

use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

use skia_safe::Pixmap;

use crate::layout::{Placed, Rect, layout};
use crate::render::{image_crop, image_scale, render_image};
use crate::scene::{Fit, Kind, Position, Scene};

use contrast::contrast;
use group::{grouped, named, shown};
use overlap::{covers, ink, overlaps, tight};
use parts::{crop_warning, motion, shadow_cut, text_cut};
use views::{hide, knockout_frames, unplayed, views};

pub use facts::{
    facts, facts_for, fill_hints, fonts_line, halftone_hints, image_dpi, scale_hints, text_report,
};

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
    Ok(check(scene, &sizes, full, assets))
}

/// Problem lines for some of the sizes (a render of only those), or `None`
/// when they're clean; `all` in a grouped line means all of these.
pub fn warnings_for(
    scene: &Scene,
    sizes: &[&crate::scene::Size],
    assets: Option<&Path>,
) -> Option<String> {
    Some(check(scene, sizes, false, assets)).filter(|w| w != "ok")
}

/// [`describe`] for these sizes.
fn check(
    scene: &Scene,
    sizes: &[&crate::scene::Size],
    full: bool,
    assets: Option<&Path>,
) -> String {
    let mut out = String::new();
    let mut problems = Vec::new();
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
    for &size in sizes {
        let sized = &*scene.for_size(size);
        for (view, t, first) in views(sized, &unplayed(scene, size)) {
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
                full,
                shots: crate::anim::shots::timeline(scene)
                    .iter()
                    .map(|&(i, start, dur, _)| (scene.layers[i].id.as_str(), (start, start + dur)))
                    .collect(),
                safe: (size.safe != [0.0; 4]).then_some(Rect {
                    x: sl,
                    y: st,
                    w: size.width - sl - sr,
                    h: size.height - st - sb,
                }),
                overlaps: overlaps(&placed),
                covers: covers(scene, &placed),
                tight: tight(&placed),
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
                problems
                    .extend(shown(&lines).map(|l| (size.id.as_str(), l.trim_start().to_owned())));
            }
        }
    }
    out.push_str(&grouped(&problems, sizes.len()));
    if out.is_empty() {
        out.push_str("ok");
    }
    out
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
    /// Listing every layer (`full`), with its motion.
    full: bool,
    /// Each shot's frame id and when it plays, seconds.
    shots: HashMap<&'s str, (f32, f32)>,
    /// The part of the canvas the platform doesn't cover, when it covers any.
    safe: Option<Rect>,
    /// Text ids whose ink overlaps other texts' ink.
    overlaps: HashMap<&'s str, Vec<&'s str>>,
    /// Text ids whose ink a later layer or highlight covers, and how much.
    covers: HashMap<&'s str, Vec<(String, f32)>>,
    /// Text ids whose ink nearly touches a neighbour's: the gap and who.
    tight: HashMap<&'s str, (f32, &'s str)>,
    /// The size rendered without text: what each text is read against.
    backdrop: Option<Pixmap<'i>>,
    /// The size without text or the frames knockout text cuts through:
    /// what shows in its letters. `None` when there's no knockout text.
    through: Option<Pixmap<'i>>,
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
        if let Some(why) = &p.cause {
            let _ = write!(out, " ({why})");
        }
    }
    if let Some(c) = &p.chosen {
        let _ = write!(out, " → {c}");
    }
    if let Some(why) = &p.skipped {
        let _ = write!(out, " ({why})");
    }
    if checks.full {
        motion(&mut out, l, checks.shots.get(l.id.as_str()).copied());
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
                    let _ = write!(out, " {}", shrunk(max, fit));
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
            // Inside a stack reported `!overflow`, its size is wrong: breaks
            // and overlaps there follow from it, and change once it's fixed.
            if let (Some((para, _)), Some(t), false) =
                (&p.text, crate::text::Text::of(l, p.k), clip.quiet)
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
            if let Some(others) = checks.overlaps.get(l.id.as_str()).filter(|_| !clip.quiet) {
                let _ = write!(out, " !overlaps {}", others.join(","));
            }
            if let Some(by) = checks.covers.get(l.id.as_str()) {
                let by: Vec<String> = by
                    .iter()
                    .map(|(who, share)| format!("{who} {}%", (share * 100.0).round().max(1.0)))
                    .collect();
                let _ = write!(out, " !covered by {}", by.join(", "));
            }
            for (left, right) in crate::render::leader_clashes(p) {
                let short: String = left.chars().take(20).collect();
                let more = if short.len() < left.len() { "…" } else { "" };
                let _ = write!(out, " !leader \"{short}{more}\" meets \"{right}\"");
            }
            if let Some((gap, other)) = checks.tight.get(l.id.as_str()).filter(|_| !clip.quiet) {
                let _ = write!(out, " warn ink {}px from {other}", n(*gap));
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
                // Which side a cover crop takes most from: it hides `focus`
                // of the overflow before the focus and the rest after.
                let focus = kind.focus().unwrap_or([0.5, 0.5]);
                let side = |f: f32, before: &'static str, after: &'static str| match crop {
                    None if fit == Fit::Cover && f <= 1.0 / 3.0 => after,
                    None if fit == Fit::Cover && f >= 2.0 / 3.0 => before,
                    _ => "",
                };
                if cw >= 0.005 {
                    let _ = write!(out, " crop {}%w", (cw * 100.0).round());
                    let s = side(focus[0], "left", "right");
                    if !s.is_empty() {
                        let _ = write!(out, " {s}");
                    }
                }
                if ch >= 0.005 {
                    let _ = write!(out, " crop {}%h", (ch * 100.0).round());
                    let s = side(focus[1], "top", "bottom");
                    if !s.is_empty() {
                        let _ = write!(out, " {s}");
                    }
                }
                let up = image_scale(r, a.width, a.height, fit, crop, tile_scale * p.k);
                if !a.svg && up > 1.005 {
                    let _ = write!(out, " upscaled {up:.1}x");
                }
                if fit == Fit::Cover && crop.is_none() {
                    let focus = kind.focus().unwrap_or([0.5, 0.5]);
                    crop_warning(&mut out, p, (a.width, a.height), focus);
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
    // An unnamed text's words, when there's something to fix or judge.
    if (out.contains(" !") || out.contains(" warn "))
        && let Some(name) = named(l)
    {
        let at = depth + l.id.len();
        out.insert_str(at, &format!(" {name}"));
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

/// A shrunk text's `(max 180, height)`: its size before shrinking, and
/// the side that kept it from being bigger.
fn shrunk(max: f32, fit: &crate::text::Fit) -> String {
    match fit.bound {
        Some(side) => format!("(max {}, {side})", n(max)),
        None => format!("(max {})", n(max)),
    }
}

/// Rounds to whole pixels for display.
fn n(v: f32) -> i64 {
    v.round() as i64
}
