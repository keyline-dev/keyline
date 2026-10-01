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
pub(super) fn text_cut(p: &Placed, visible: Rect) -> Option<Rect> {
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
/// axis: with the focus at the center (the default), the box then can't
/// hold the image's middle half, so a subject there is cut, e.g. a house
/// in a band too short for it. (The focus lands at its own share of the
/// box, like CSS `object-position`, whatever the crop.) The fix names the
/// field to write, master px: the `height` that shows half (taller for a
/// crop top and bottom, at most this tall for one at the sides), or the
/// `minHeight` (`maxHeight`) for a layer whose height a stack or its
/// parent gives or squeezes.
pub(super) fn crop_warning(out: &mut String, p: &Placed, image: (f32, f32), focus: [f32; 2]) {
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
        let min = ((drawn / 2.0) / p.k).ceil();
        match squeezed {
            Some(h) => format!(
                "squeezed from {} to {}: minHeight {min} shows half",
                n(h),
                n(r.h)
            ),
            None if given(l.height) => format!("minHeight {min} shows half"),
            None => format!("height {min} shows half"),
        }
    } else {
        let max = ((2.0 * r.w * ih / iw) / p.k).floor();
        if given(l.height) {
            format!("maxHeight {max} shows half")
        } else {
            format!("height at most {max} shows half")
        }
    };
    let _ = write!(
        out,
        " warn crop cuts {what} (focus {}%,{}%): {fix}",
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
