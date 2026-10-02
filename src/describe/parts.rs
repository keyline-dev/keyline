//! Parts of a layer's line: its motion, a text cut by what clips it, a
//! cover crop past half, a clipped drop shadow.

use std::fmt::Write;

use super::overlap::ink;
use super::{contains, n};
use crate::layout::{Placed, Rect};
use crate::scene::Kind;

/// A layer's motion for `scene_describe full`, in its own clock (a shot's
/// layers count from the shot's start): `shot 2–3.5s`, `enter fade-up
/// 0.4–1s`, `count,draw 1.4–3s` (`×3` repeats, `×∞` forever), `exit fade
/// 7–7.6s` (or `at the end`).
pub(super) fn motion(out: &mut String, l: &crate::scene::Layer, shot: Option<(f32, f32)>) {
    let s = |v: f32| (v * 10.0).round() / 10.0;
    let effect = |m: &crate::anim::motion::Motion| {
        serde_json::to_value(m.effect)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    };
    if let Some((a, b)) = shot {
        let _ = write!(out, " shot {}–{}s", s(a), s(b));
    }
    let t = &l.time;
    if let Some(m) = &t.enter {
        let a = m.at.unwrap_or(0.0);
        let _ = write!(out, " enter {} {}–{}s", effect(m), s(a), s(a + m.duration));
    }
    for tr in t.animate.iter().flat_map(crate::scene::OneOrMany::as_slice) {
        let props: Vec<&str> = tr.props.keys().map(String::as_str).collect();
        let _ = write!(
            out,
            " {} {}–{}s",
            props.join(","),
            s(tr.at),
            s(tr.at + tr.duration)
        );
        match tr.repeat {
            0 => {}
            r if r < 0 => out.push_str(" ×∞"),
            r => {
                let _ = write!(out, " ×{}", r + 1);
            }
        }
    }
    if let Some(m) = &t.out {
        match m.at {
            Some(a) => {
                let _ = write!(out, " exit {} {}–{}s", effect(m), s(a), s(a + m.duration));
            }
            None => {
                let _ = write!(out, " exit {} at the end", effect(m));
            }
        }
    }
}

/// What of text `p` falls outside `visible`: its box, or its letters
/// where they reach past the box (tall caps at a tight `lineHeight` in a
/// frame that clips). `None` when all of it shows. Ink gets a pixel of
/// slack for antialiasing.
pub(super) fn text_cut(p: &Placed, m: &skia_safe::Matrix, visible: Rect) -> Option<Rect> {
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
    let shown = super::overlap::mapped(m, shown);
    (!contains(visible, shown)).then_some(shown)
}

/// `!crop` when a cover crop hides more than half the image top and bottom
/// (a defect: agents fix those, and a `warn` here was left as it was), or
/// `warn crop` when it does at the sides: with the focus at the center (the default), the box then can't
/// hold the image's middle half, so a subject there is cut, e.g. a house
/// in a band too short for it. (The focus lands at its own share of the
/// box, like CSS `object-position`, whatever the crop.) The fix names the
/// field to write, master px: the `height` that shows all of the image and
/// the one that shows half (taller for a crop top and bottom, at most this
/// tall for one at the sides), or the
/// `minHeight` (`maxHeight`) for a layer whose height a stack or its
/// parent gives or squeezes.
pub(super) fn crop_warning(
    out: &mut String,
    (p, siblings): (&Placed, &[Placed]),
    image: (f32, f32),
    focus: [f32; 2],
) {
    let r = p.rect;
    let (iw, ih) = image;
    // Cover scales the image to fill the box; the other axis is cropped.
    let s = (r.w / iw).max(r.h / ih);
    let (drawn_w, drawn_h) = (iw * s, ih * s);
    let vertical = drawn_h - r.h >= drawn_w - r.w;
    let (shown, drawn) = if vertical {
        (r.h, drawn_h)
    } else {
        (r.w, drawn_w)
    };
    if shown / drawn >= 0.495 {
        return;
    }
    let what = if focus == [0.5, 0.5] {
        "the image's middle"
    } else {
        "the area around its focus"
    };
    let l = p.layer;
    // Its height comes from its parent: `fill`, a share, `flexGrow`, or a
    // stretch constraint that drew it at another height than it asks.
    let fixed = l.height.and_then(crate::scene::Length::px).map(|h| h * p.k);
    let given = |len: Option<crate::scene::Length>| {
        matches!(
            len,
            Some(crate::scene::Length::Fill | crate::scene::Length::Pct(_))
        ) || l.grow.is_some_and(|g| g > 0.0)
            || fixed.is_some_and(|h| (h - r.h).abs() > 0.5)
    };
    // Taller shows more of a tall crop; a wide crop's box is usually as
    // wide as it can be, so shorter shows more of it.
    // A fixed height a stretch constraint shrank: setting the height
    // again changes nothing; a minHeight stops the squeeze.
    let squeezed = fixed.filter(|h| *h > r.h + 0.5 && l.min_height.is_none());
    // The value to write, in master px like every field (a size's
    // `scale` multiplies it): taller shows more of a crop top and bottom;
    // a crop at the sides is usually as wide as it can be, so shorter
    // shows more of it.
    let fix = if vertical {
        let all = (drawn / p.k).ceil();
        let min = ((drawn / 2.0) / p.k).ceil();
        // What it takes at this size, in px, and who shares the column:
        // the agent picks what gives way.
        let more = (drawn / 2.0 - r.h).ceil();
        let field = match squeezed {
            Some(h) => format!(
                "squeezed from {} to {}: minHeight {min} shows half, {all} all",
                n(h),
                n(r.h)
            ),
            None if given(l.height) => format!("minHeight {min} shows half, {all} all"),
            None => format!("height {min} shows half, {all} all"),
        };
        format!(
            "needs {}px more here ({field}){}",
            n(more),
            column(p, siblings)
        )
    } else {
        let all = ((r.w * ih / iw) / p.k).floor();
        let max = ((2.0 * r.w * ih / iw) / p.k).floor();
        if given(l.height) {
            format!("maxHeight {all} shows all, {max} half")
        } else {
            format!("height at most {all} shows all, {max} half")
        }
    };
    let _ = write!(
        out,
        " {} cuts {what} (focus {}%,{}%): {fix}",
        // A band too short for its photo cuts the subject (a defect); a
        // crop at the sides, a landscape photo in a tall box, is common.
        if vertical { "!crop" } else { "warn crop" },
        (focus[0] * 100.0).round(),
        (focus[1] * 100.0).round()
    );
}

/// Whether a frame that clips its content cuts `p`'s drop shadow: where
/// its offset, spread and half its blur reach, so a soft tail fading
/// against the edge doesn't count. Text shadows follow the letters and
/// aren't judged.
pub(super) fn shadow_cut(p: &Placed, visible: Rect) -> bool {
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

/// A cut its own motion makes: the layer drawn at its entrance's and its
/// tracks' extremes (a `pop` overshooting, a scale pulse), `shown` being
/// its box at rest. ` !clipped by info: bottom 13px during enter pop
/// 2.1–2.7s`, at the worst moment; `None` when every moment shows whole.
/// Only cuts by a clipping frame count: at the canvas, entrances fly in
/// from outside on purpose.
pub(super) fn motion_cut(p: &Placed, shown: Rect, clip: &super::Clip) -> Option<String> {
    use crate::anim::track::Val;
    let s = |v: f32| (v * 10.0).round() / 10.0;
    let t = &p.layer.time;
    let seed = crate::anim::track::seed(&p.layer.id);
    // (scale, offset in master px, when)
    let mut samples: Vec<(f32, [f32; 2], String)> = Vec::new();
    if let Some(m) = &t.enter {
        let a = m.at.unwrap_or(0.0);
        let name = serde_json::to_value(m.effect)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        let when = format!("enter {name} {}–{}s", s(a), s(a + m.duration));
        for i in 0..=16 {
            let away = m.enter(a + m.duration * i as f32 / 16.0);
            samples.push((away.scale, away.offset, when.clone()));
        }
    }
    for tr in t.animate.iter().flat_map(crate::scene::OneOrMany::as_slice) {
        if !tr.props.contains_key("scale") && !tr.props.contains_key("translate") {
            continue;
        }
        let props: Vec<&str> = tr.props.keys().map(String::as_str).collect();
        let when = format!(
            "{} {}–{}s",
            props.join(","),
            s(tr.at),
            s(tr.at + tr.duration)
        );
        for i in 0..=16 {
            let at = tr.at + tr.duration * i as f32 / 16.0;
            let scale = match tr.value("scale", Val::Num(1.0), at, seed) {
                Some(Val::Num(v)) => v,
                _ => 1.0,
            };
            let offset = match tr.value("translate", Val::Pair([0.0, 0.0]), at, seed) {
                Some(Val::Pair(o)) => o,
                _ => [0.0, 0.0],
            };
            samples.push((scale, offset, when.clone()));
        }
    }
    let v = clip.rect;
    let mut worst: Option<(f32, String)> = None;
    for (scale, offset, when) in samples {
        let (cx, cy) = (shown.x + shown.w / 2.0, shown.y + shown.h / 2.0);
        let (w, h) = (shown.w * scale, shown.h * scale);
        let b = Rect {
            x: cx - w / 2.0 + offset[0] * p.k,
            y: cy - h / 2.0 + offset[1] * p.k,
            w,
            h,
        };
        // Only past an edge it sits inside at rest: a photo filling its
        // frame (a slow zoom) means to run past it.
        let gaps = [
            shown.x - v.x,
            shown.y - v.y,
            v.right() - shown.right(),
            v.bottom() - shown.bottom(),
        ];
        let past = [
            v.x - b.x,
            v.y - b.y,
            b.right() - v.right(),
            b.bottom() - v.bottom(),
        ];
        // A frame sizing to it is its edge, though: it counts there.
        let counts = |i: usize| gaps[i] > 0.5 || clip.hug[i];
        let px = (0..4)
            .filter(|&i| counts(i))
            .map(|i| past[i])
            .fold(0.0, f32::max);
        // A pixel or two of a moment's overshoot doesn't show.
        if px <= 2.0 || worst.as_ref().is_some_and(|(w, _)| px <= *w) {
            continue;
        }
        // The edges it fills at rest aren't said either.
        let (mut l, mut tp, mut r, mut bt) = (b.x, b.y, b.right(), b.bottom());
        if !counts(0) {
            l = l.max(v.x);
        }
        if !counts(1) {
            tp = tp.max(v.y);
        }
        if !counts(2) {
            r = r.min(v.right());
        }
        if !counts(3) {
            bt = bt.min(v.bottom());
        }
        let b = Rect {
            x: l,
            y: tp,
            w: r - l,
            h: bt - tp,
        };
        let Some(cut) = clip.cut(b) else { continue };
        // Only a frame that clips: at the canvas, entrances fly in from
        // outside, and a growing box overstates a masked or rotated shape.
        if cut.starts_with(" !clipped by canvas") {
            continue;
        }
        worst = Some((px, format!("{cut} during {when}")));
    }
    worst.map(|(_, line)| line)
}

/// The layers above and below `p` in its parent, tallest first, by id and
/// height at this size: `; above and below it: headline 220, steps 202`.
/// They share its column, so one of them gives the room it needs.
fn column(p: &Placed, siblings: &[Placed]) -> String {
    let r = p.rect;
    let mut beside: Vec<&Placed> = siblings
        .iter()
        .filter(|s| !std::ptr::eq(*s, p))
        .filter(|s| {
            let o = s.rect;
            o.x < r.right()
                && o.right() > r.x
                && (o.bottom() <= r.y + 0.5 || o.y >= r.bottom() - 0.5)
        })
        .collect();
    if beside.is_empty() {
        return String::new();
    }
    beside.sort_by(|a, b| b.rect.h.total_cmp(&a.rect.h));
    let list: Vec<String> = beside
        .iter()
        .take(4)
        .map(|s| format!("{} {}", s.layer.id, n(s.rect.h)))
        .collect();
    format!("; above and below it: {}", list.join(", "))
}
