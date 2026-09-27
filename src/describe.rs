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

use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

use skia_safe::Pixmap;

use crate::layout::{Placed, Rect, layout};
use crate::render::{image_crop, image_scale, render_image};
use crate::scene::{Color, Fit, Kind, Scene, Size};
use crate::text::Text;

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

/// Facts for the agent to judge against its medium, one line:
/// `smallest text: portrait 30px, sky 8.4px (footer); upscaled: wide photo 1.3x`.
/// No thresholds: 8 px may be fine print on a banner and unreadable on a
/// billboard.
pub fn facts(scene: &Scene) -> String {
    fn walk<'a>(placed: &'a [Placed<'a>], f: &mut impl FnMut(&'a Placed<'a>)) {
        for p in placed {
            f(p);
            walk(&p.children, f);
        }
    }
    let mut smallest = Vec::new();
    let mut upscaled = Vec::new();
    for size in &scene.sizes {
        let sized = scene.for_size(&size.id);
        let placed = layout(&sized, size);
        let mut min: Option<(f32, &str)> = None;
        walk(&placed, &mut |p| match (&p.text, &p.layer.kind) {
            (Some((_, fit)), _) if min.is_none_or(|(px, _)| fit.font_size < px) => {
                min = Some((fit.font_size, &p.layer.id));
            }
            (
                _,
                Kind::Image {
                    asset,
                    fit,
                    crop,
                    tile_scale,
                    ..
                },
            ) => {
                if let Some(a) = scene.assets.get(asset).filter(|a| !a.svg) {
                    let up = image_scale(
                        p.rect,
                        a.width,
                        a.height,
                        *fit,
                        crop.as_ref(),
                        tile_scale * p.k,
                    );
                    if up > 1.005 {
                        upscaled.push(format!("{} {} {up:.1}x", size.id, p.layer.id));
                    }
                }
            }
            _ => {}
        });
        if let Some((px, id)) = min {
            smallest.push(format!("{} {}px ({id})", size.id, px_label(px)));
        }
    }
    let mut out = String::new();
    if !smallest.is_empty() {
        let _ = write!(out, "smallest text: {}", smallest.join(", "));
    }
    if !upscaled.is_empty() {
        let sep = if out.is_empty() { "" } else { "; " };
        let _ = write!(out, "{sep}upscaled: {}", upscaled.join(", "));
    }
    out
}

/// Whole pixels, or one decimal below 12 px where a fraction matters.
fn px_label(px: f32) -> String {
    if px < 12.0 {
        format!("{px:.1}").trim_end_matches(".0").to_owned()
    } else {
        n(px).to_string()
    }
}

/// For `render`: every text that wrapped, shrank or was cut, as actually
/// drawn, so the agent can check wording and breaks without an image.
/// Texts drawn on one line at their requested size are left out.
///
/// ```text
///  headline 68px: "Proven RESULTS for" / "WILLOWMERE Families"
///  cta 44px (max 56): "VOTE BY MAIL"
/// ```
pub fn text_report(scene: &Scene, size: &Size) -> String {
    fn go(out: &mut String, placed: &[Placed]) {
        for p in placed {
            if let (Some((para, fit)), Some(t), Kind::Text { font_size, .. }) =
                (&p.text, Text::of(p.layer, p.k), &p.layer.kind)
            {
                let max = font_size * p.k;
                let shrunk = n(max) != n(fit.font_size);
                if fit.lines > 1 || shrunk || fit.truncated {
                    let _ = write!(out, " {} {}px", p.layer.id, n(fit.font_size));
                    if shrunk {
                        let _ = write!(out, " (max {})", n(max));
                    }
                    let lines: Vec<String> = t
                        .drawn_lines(para, fit)
                        .iter()
                        .map(|l| format!("{l:?}"))
                        .collect();
                    let _ = writeln!(out, ": {}", lines.join(" / "));
                }
            }
            go(out, &p.children);
        }
    }
    let mut out = String::new();
    go(&mut out, &layout(&scene.for_size(&size.id), size));
    out
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

/// The box the glyphs actually cover, canvas coordinates: usually smaller
/// than the layer's box, and than its lines' ascent-to-descent height.
fn ink(p: &Placed) -> Option<Rect> {
    let (_, fit) = p.text.as_ref()?;
    // Lay the text out again to read its glyph outlines: the real ink, not
    // the line box, whose ascent and descent space would make tightly set
    // lines look like they collide.
    let text = Text::of(p.layer, p.k)?;
    let mut para = text.repaint(fit, p.rect.w, &skia_safe::Paint::default(), false);
    let bounds = (0..para.line_number())
        .map(|line| *para.get_path_at(line).1.bounds())
        .filter(|b| !b.is_empty())
        .reduce(skia_safe::Rect::join2)?;
    Some(Rect {
        x: p.rect.x + bounds.left,
        y: p.text_top() + bounds.top,
        w: bounds.width(),
        h: bounds.height(),
    })
}

/// Pairs of texts whose ink overlaps by more than a pixel each way.
fn overlaps<'s>(placed: &[Placed<'s>]) -> HashMap<&'s str, Vec<&'s str>> {
    fn collect<'s>(inks: &mut Vec<(&'s str, Rect)>, placed: &[Placed<'s>]) {
        for p in placed {
            if let Some(r) = ink(p) {
                inks.push((&p.layer.id, r));
            }
            collect(inks, &p.children);
        }
    }
    let mut inks = Vec::new();
    collect(&mut inks, placed);
    // ponytail: O(n²) over texts; a sweep line if scenes grow to hundreds.
    let mut out: HashMap<&str, Vec<&str>> = HashMap::new();
    for (i, (a, ra)) in inks.iter().enumerate() {
        for (b, rb) in &inks[i + 1..] {
            if intersect(*ra, *rb).is_some_and(|x| x.w > 1.0 && x.h > 1.0) {
                out.entry(a).or_default().push(b);
                out.entry(b).or_default().push(a);
            }
        }
    }
    out
}

/// WCAG contrast of the text's colors, as drawn (their alpha and `opacity`
/// blended over the backdrop), against the average luminance behind its
/// ink. Returns `(worst ratio, required)` when below the requirement: 3:1
/// for large text (≥ 24 px, or ≥ 18.66 px bold), else 4.5:1.
// ponytail: averages the backdrop; text over a busy photo can pass on
// average yet fail in places. Check per-glyph region if that bites.
fn contrast(p: &Placed, ink: Rect, backdrop: &Pixmap, opacity: f32) -> Option<(f32, f32)> {
    let (_, fit) = p.text.as_ref()?;
    let Kind::Text {
        color,
        ranges,
        weight,
        fill,
        gradient,
        ..
    } = &p.layer.kind
    else {
        return None;
    };
    if fill.is_some() || gradient.is_some() {
        return None; // letters painted with an image or gradient: no single color to judge
    }
    const GRID: i32 = 16;
    let colors: Vec<Color> = std::iter::once(*color)
        .chain(ranges.iter().map(|r| r.color))
        .collect();
    let (bw, bh) = (backdrop.width() - 1, backdrop.height() - 1);
    let mut bg = 0.0;
    let mut fg = vec![0.0; colors.len()];
    for gx in 0..GRID {
        for gy in 0..GRID {
            let x = (ink.x + ink.w * (gx as f32 + 0.5) / GRID as f32) as i32;
            let y = (ink.y + ink.h * (gy as f32 + 0.5) / GRID as f32) as i32;
            let c = backdrop.get_color((x.clamp(0, bw), y.clamp(0, bh)));
            let under = [c.r(), c.g(), c.b()];
            bg += luminance(under);
            for (sum, text) in fg.iter_mut().zip(&colors) {
                *sum += luminance(over(*text, opacity, under));
            }
        }
    }
    let samples = (GRID * GRID) as f32;
    let worst = fg
        .iter()
        .map(|f| ratio(f / samples, bg / samples))
        .fold(f32::MAX, f32::min);
    let large = fit.font_size >= 24.0 || (fit.font_size >= 18.66 && *weight >= 700);
    let min = if large { 3.0 } else { 4.5 };
    (worst < min).then_some((worst, min))
}

/// `text` drawn at its alpha × `opacity` over `under`, as sRGB.
fn over(text: Color, opacity: f32, under: [u8; 3]) -> [u8; 3] {
    let [a, r, g, b] = text.0.to_be_bytes();
    let a = f32::from(a) / 255.0 * opacity;
    let mix = |t: u8, u: u8| (f32::from(t) * a + f32::from(u) * (1.0 - a)).round() as u8;
    [mix(r, under[0]), mix(g, under[1]), mix(b, under[2])]
}

/// WCAG relative luminance of an sRGB color.
fn luminance([r, g, b]: [u8; 3]) -> f32 {
    let lin = |v: u8| {
        let c = f32::from(v) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

fn ratio(a: f32, b: f32) -> f32 {
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scene(layers: serde_json::Value) -> Scene {
        let mut v = json!({
            "width": 400, "height": 200,
            "sizes": [{"id": "wide", "width": 400, "height": 200}, {"id": "small", "width": 200, "height": 200, "scale": 0.5}],
            "assets": {"img": {"sha256": "x", "width": 400, "height": 400}},
        });
        v["layers"] = layers;
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn one_line_per_layer_per_size() {
        let s = scene(json!([
            {"id": "photo", "type": "image", "asset": "img", "width": 400, "height": 200},
            {"id": "bar", "type": "frame", "y": 150, "width": 400, "height": 50, "children": [
                {"id": "r", "type": "rect", "width": 400, "height": 50, "color": "#FF0000"}
            ]}
        ]));
        let d = describe(&s, None, true, None).unwrap();
        assert_eq!(d.lines().count(), 9, "{d}");
        assert!(
            d.starts_with(
                "assets img 400×400\nwide 400×200\n photo image 0,0 400×200 fill crop 50%h\n"
            ),
            "{d}"
        );
        assert!(d.contains("small 200×200\n"), "{d}");
        assert!(d.contains("  r rect 0,150 400×50\n"), "{d}");
        assert_eq!(
            describe(&s, Some("small"), true, None)
                .unwrap()
                .lines()
                .count(),
            5
        );
        assert!(describe(&s, Some("nope"), true, None).is_err());
        // A clean layout costs two characters by default.
        assert_eq!(describe(&s, None, false, None).unwrap(), "ok");
        assert_eq!(warnings(&s, None), None);
    }

    #[test]
    fn warns_on_overflow_clipping_and_hidden_layers() {
        let s = scene(json!([
            {"id": "long", "type": "text", "text": "a very long headline that will not fit", "fontSize": 30, "resize": "fixed", "width": 100, "height": 30},
            {"id": "edge", "type": "text", "text": "edge", "x": 380, "fontSize": 30},
            {"id": "gone", "type": "rect", "x": 500, "width": 10, "height": 10}
        ]));
        let d = describe(&s, Some("wide"), true, None).unwrap();
        assert!(
            d.contains("long text 0,0 100×30 30px") && d.contains("!overflow needs 100×"),
            "{d}"
        );
        assert!(
            d.lines()
                .any(|l| l.starts_with(" edge") && l.contains("!clipped by canvas: right")),
            "{d}"
        );
        assert!(
            d.lines()
                .any(|l| l.starts_with(" gone") && l.ends_with("!hidden")),
            "{d}"
        );
        // By default only the warning lines, each tagged with its size.
        let w = warnings(&s, None).unwrap();
        assert_eq!(w.lines().count(), 6, "{w}");
        assert!(
            w.lines().all(|l| l.contains(" !") || l.contains(" warn ")),
            "{w}"
        );
        assert!(w.contains("small gone rect"), "{w}");
    }

    #[test]
    fn clipped_warning_names_the_frame_and_the_cut() {
        let s = scene(json!([
            {"id": "band", "type": "frame", "y": 10, "width": 400, "height": 40, "children": [
                {"id": "title", "type": "text", "text": "Title", "y": 20, "fontSize": 30}
            ]}
        ]));
        let w = warnings(&s, None).unwrap();
        let line = w.lines().find(|l| l.starts_with("wide title")).unwrap();
        assert!(line.contains("!clipped by band: bottom "), "{line}");
    }

    #[test]
    fn overlapping_text_is_a_defect() {
        let s = scene(json!([
            {"id": "a", "type": "text", "text": "Hello", "fontSize": 30},
            {"id": "b", "type": "text", "text": "World", "x": 40, "y": 10, "fontSize": 30}
        ]));
        let w = warnings(&s, None).unwrap();
        assert!(
            w.contains("wide a text") && w.contains("!overlaps b"),
            "{w}"
        );
        assert!(
            w.contains("wide b text") && w.contains("!overlaps a"),
            "{w}"
        );
    }

    #[test]
    fn tightly_set_lines_only_overlap_if_their_glyphs_do() {
        // "Monica" over "JETHANI": the line boxes overlap by a few pixels of
        // ascent and descent space, the letters don't.
        let s = scene(json!([
            {"id": "first", "type": "text", "text": "Monica", "y": 20, "fontSize": 26, "weight": 800},
            {"id": "last", "type": "text", "text": "JETHANI", "y": 50, "fontSize": 44, "weight": 900}
        ]));
        assert_eq!(warnings(&s, None), None);
    }

    #[test]
    fn small_text_and_upscaling_are_facts_not_warnings() {
        let s = scene(json!([
            // 14 px at master is 7 px in the half-scale size.
            {"id": "fine", "type": "text", "text": "Fine print", "y": 150, "fontSize": 14},
            {"id": "head", "type": "text", "text": "Head", "fontSize": 30},
            // A 400 px image covering 800 px; at half scale it's drawn 1:1.
            {"id": "big", "type": "image", "asset": "img", "y": 100, "width": 800, "height": 100}
        ]));
        assert_eq!(warnings(&s, None), None);
        assert_eq!(
            facts(&s),
            "smallest text: wide 14px (fine), small 7px (fine); upscaled: wide big 2.0x"
        );
        let d = describe(&s, Some("wide"), true, None).unwrap();
        assert!(
            d.contains("big image 0,100 800×100 fill crop 88%h upscaled 2.0x"),
            "{d}"
        );
    }

    #[test]
    fn warns_on_low_contrast_against_what_is_behind() {
        let s: Scene = serde_json::from_value(json!({
            "width": 400, "height": 100, "background": "#FFFFFF",
            "sizes": [{"id": "a", "width": 400, "height": 100}],
            "layers": [
                {"id": "panel", "type": "rect", "width": 200, "height": 100, "color": "#1B2A5C"},
                // Navy on navy: unreadable. White on navy and navy on white: fine.
                {"id": "dim", "type": "text", "text": "Dim", "x": 10, "y": 10, "fontSize": 16, "color": "#22335F"},
                {"id": "lit", "type": "text", "text": "Lit", "x": 10, "y": 50, "fontSize": 16, "color": "#FFFFFF"},
                {"id": "ink", "type": "text", "text": "Ink", "x": 250, "y": 10, "fontSize": 16, "color": "#1B2A5C"}
            ]
        }))
        .unwrap();
        let w = warnings(&s, Some(&std::env::temp_dir())).unwrap();
        assert_eq!(w.lines().count(), 1, "{w}");
        assert!(
            w.starts_with("a dim text")
                && w.contains(" warn contrast 1.")
                && w.contains("(WCAG 4.5)"),
            "{w}"
        );
        // Without an asset store the check is skipped.
        assert_eq!(warnings(&s, None), None);
    }

    #[test]
    fn faint_text_is_judged_as_drawn() {
        let s: Scene = serde_json::from_value(json!({
            "width": 400, "height": 100,
            "sizes": [{"id": "a", "width": 400, "height": 100}],
            "layers": [
                {"id": "panel", "type": "rect", "width": 400, "height": 100, "color": "#1B2A5C"},
                // White, but at 1/8 alpha, or inside a 10% frame: barely visible.
                {"id": "alpha", "type": "text", "text": "Faint", "x": 10, "y": 10, "color": "#FFFFFF20"},
                {"id": "box", "type": "frame", "x": 200, "width": 200, "height": 100, "opacity": 0.1, "children": [
                    {"id": "nested", "type": "text", "text": "Faint", "x": 10, "y": 10, "color": "#FFFFFF"}
                ]},
                {"id": "solid", "type": "text", "text": "Clear", "x": 10, "y": 50, "color": "#FFFFFF"}
            ]
        }))
        .unwrap();
        let w = warnings(&s, Some(&std::env::temp_dir())).unwrap();
        assert_eq!(w.lines().count(), 2, "{w}");
        assert!(
            w.contains("a alpha text") && w.contains("a nested text"),
            "{w}"
        );
    }

    #[test]
    fn text_report_shows_only_wrapped_shrunk_or_cut_text() {
        let s = scene(json!([
            {"id": "plain", "type": "text", "text": "Short", "fontSize": 20},
            {"id": "wrap", "type": "text", "text": "one two three four", "fontSize": 20, "width": 80},
            {"id": "fit", "type": "text", "text": "SHRINK ME", "fontSize": 40, "width": 150, "height": 30, "y": 120}
        ]));
        let r = text_report(&s, &s.sizes[0]);
        assert!(!r.contains("plain"), "{r}");
        assert!(r.contains(" wrap 20px: \"one two\" / "), "{r}");
        assert!(
            r.contains(" fit ") && r.contains("(max 40): \"SHRINK ME\""),
            "{r}"
        );
    }
}
