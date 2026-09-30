//! Split text: each letter or word moves on its own (GSAP's SplitText). The
//! text is laid out once as usual; each piece is drawn clipped to its own
//! box, with its own entrance (delayed by `stagger`), keyframe values and
//! `random()` seed.

use anyhow::Result;
use skia_safe::textlayout::{RectHeightStyle, RectWidthStyle};
use skia_safe::{Paint, canvas::SaveLayerRec};

use super::Ctx;
use crate::anim::motion::Away;
use crate::anim::track::{self, Val};
use crate::anim::{LayerTime, Split};
use crate::layout::Placed;
use crate::scene::OneOrMany;
use crate::text::Text;

/// The pieces of `display`, as UTF-16 ranges (what the paragraph measures in).
fn pieces(display: &str, split: Split) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut unit = 0;
    let mut word: Option<usize> = None;
    for c in display.chars() {
        let len = c.len_utf16();
        let space = c.is_whitespace();
        match split {
            Split::Chars if !space => out.push(unit..unit + len),
            Split::Words if space => {
                if let Some(start) = word.take() {
                    out.push(start..unit);
                }
            }
            Split::Words => {
                word.get_or_insert(unit);
            }
            Split::Chars => {}
        }
        unit += len;
    }
    if let Some(start) = word {
        out.push(start..unit);
    }
    out
}

/// Where piece `i` is at `t`: its entrance and exit, then its tracks.
fn motion(time: &LayerTime, i: usize, seed: u32, t: f32, end: f32) -> (Away, f32, [f32; 2]) {
    let delay = i as f32 * time.stagger.unwrap_or(0.0);
    let mut away = Away::REST;
    if let Some(m) = &time.enter {
        away = away.then(m.delayed(delay).enter(t));
    }
    if let Some(m) = &time.out {
        away = away.then(m.leave(t, end));
    }
    let (mut rotation, mut skew) = (0.0, [0.0, 0.0]);
    for tr in time.animate.iter().flat_map(OneOrMany::as_slice) {
        let mut tr = tr.clone();
        tr.at += delay;
        let num = |prop: &str, own: f32| match tr.value(prop, Val::Num(own), t, seed) {
            Some(Val::Num(n)) => n,
            _ => own,
        };
        away.opacity *= num("opacity", 1.0);
        away.scale *= num("scale", 1.0);
        away.blur += num("blur", 0.0);
        rotation = num("rotate", rotation);
        if let Some(Val::Pair([x, y])) = tr.value("translate", Val::Pair([0.0, 0.0]), t, seed) {
            away.offset = [away.offset[0] + x, away.offset[1] + y];
        }
        if let Some(Val::Pair(s)) = tr.value("skew", Val::Pair(skew), t, seed) {
            skew = s;
        }
    }
    (away, rotation, skew)
}

impl Ctx<'_> {
    /// Draws split text `p` at `t`, piece by piece.
    pub(super) fn draw_split(
        &mut self,
        canvas: &skia_safe::Canvas,
        p: &Placed,
        r: skia_safe::Rect,
        (t, end): (f32, f32),
    ) -> Result<()> {
        let l = p.layer;
        let (Some((para, fit)), Some(text), Some(split)) =
            (&p.text, Text::of(l, p.k), l.time.split)
        else {
            return self.draw_text(canvas, p, r);
        };
        let (ox, oy) = p.text_origin();
        let bands = line_bands(&text, fit);
        let pieces = pieces(text.display(), split);
        self.split_highlights(canvas, p, &pieces, &bands, (t, end));
        for (i, range) in pieces.into_iter().enumerate() {
            let boxes = para.get_rects_for_range(
                range.clone(),
                RectHeightStyle::Max,
                RectWidthStyle::Tight,
            );
            let Some(bx) = boxes.iter().map(|b| b.rect).reduce(skia_safe::Rect::join2) else {
                continue;
            };
            // Clipped across to the piece's advance, and up and down to its
            // own line's ink: tightly set lines' boxes overlap, and a piece
            // mustn't carry a sliver of the next line along.
            let line = para
                .get_line_number_at_utf16_offset(range.start)
                .unwrap_or(0);
            let (top, bottom) = bands.get(line).copied().unwrap_or((bx.top, bx.bottom));
            let bx = skia_safe::Rect::new(bx.left, top, bx.right, bottom).with_offset((ox, oy));
            let seed = track::seed(&format!("{}#{i}", l.id));
            let (away, rotation, skew) = motion(&l.time, i, seed, t, end);
            if away.opacity <= 0.0 {
                continue;
            }
            // A layer of its own when it fades or blurs, else a plain save.
            if away.opacity < 1.0 || away.blur > 0.0 {
                let mut paint = Paint::default();
                paint.set_alpha_f(away.opacity.clamp(0.0, 1.0));
                if away.blur > 0.0 {
                    let s = super::effects::sigma(away.blur * p.k);
                    paint.set_image_filter(skia_safe::image_filters::blur(
                        (s, s),
                        None,
                        None,
                        None,
                    ));
                }
                canvas.save_layer(&SaveLayerRec::default().paint(&paint));
            } else {
                canvas.save();
            }
            let c = bx.center();
            canvas.translate((c.x + away.offset[0] * p.k, c.y + away.offset[1] * p.k));
            canvas.rotate(rotation, None);
            canvas.scale((away.scale, away.scale));
            canvas.skew((skew[0].to_radians().tan(), skew[1].to_radians().tan()));
            canvas.translate((-c.x, -c.y));
            // ponytail: clipped to the glyphs' advance box; a glyph that
            // overhangs its advance (italic, swashes) loses the overhang.
            // Anti-aliased only while turned or scaled: two soft edges
            // meeting on one pixel leave a faint seam between letters
            // that touch (kerned T and A).
            let turned = rotation != 0.0 || away.scale != 1.0 || skew != [0.0, 0.0];
            canvas.clip_rect(bx, None, turned);
            self.draw_text_with(canvas, p, r, false)?;
            canvas.restore();
        }
        Ok(())
    }
}

impl Ctx<'_> {
    /// Split text's highlights, whole as if unsplit, one line at a time:
    /// each line's arrives with its first piece (that piece's opacity and
    /// offset), so it never shows before its letters.
    fn split_highlights(
        &self,
        canvas: &skia_safe::Canvas,
        p: &Placed,
        pieces: &[std::ops::Range<usize>],
        bands: &[(f32, f32)],
        (t, end): (f32, f32),
    ) {
        let (Some((para, _)), Some(text)) = (&p.text, Text::drawn(p.layer, p.k)) else {
            return;
        };
        if text.highlights().is_empty() {
            return;
        }
        let (ox, oy) = p.text_origin();
        for (line, &(top, bottom)) in bands.iter().enumerate() {
            let Some(first) = pieces
                .iter()
                .position(|r| para.get_line_number_at_utf16_offset(r.start) == Some(line))
            else {
                continue;
            };
            let seed = track::seed(&format!("{}#{first}", p.layer.id));
            let (away, _, _) = motion(&p.layer.time, first, seed, t, end);
            if away.opacity <= 0.0 {
                continue;
            }
            let mut paint = Paint::default();
            paint.set_alpha_f(away.opacity.clamp(0.0, 1.0));
            canvas.save_layer(&SaveLayerRec::default().paint(&paint));
            canvas.translate((away.offset[0] * p.k, away.offset[1] * p.k));
            let band = skia_safe::Rect::new(-1e6, oy + top, 1e6, oy + bottom);
            canvas.clip_rect(band, None, false);
            super::text_extras::highlights(canvas, &text, para, (ox, oy), p.k, p.layer);
            canvas.restore();
        }
    }
}

/// Each line's vertical band, paragraph coordinates: its glyphs' ink,
/// widened halfway to the neighbouring lines' ink (room for shadows and
/// antialiasing without reaching another line's glyphs).
fn line_bands(text: &Text, fit: &crate::text::Fit) -> Vec<(f32, f32)> {
    let mut para = text.repaint(fit, fit.wrap_width, &Paint::default(), false);
    let ink: Vec<(f32, f32)> = crate::text::line_paths(&mut para)
        .iter()
        .map(|line| {
            let b = *line.bounds();
            (b.top, b.bottom)
        })
        .collect();
    let height = para.height();
    (0..ink.len())
        .map(|i| {
            let top = if i == 0 {
                -height
            } else {
                f32::midpoint(ink[i - 1].1, ink[i].0)
            };
            let bottom = ink
                .get(i + 1)
                .map_or(height * 2.0, |next| f32::midpoint(ink[i].1, next.0));
            (top, bottom)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::pieces;
    use crate::anim::Split;

    #[test]
    fn text_splits_into_letters_or_words_in_utf16_units() {
        assert_eq!(pieces("Hi yo", Split::Chars), vec![0..1, 1..2, 3..4, 4..5]);
        assert_eq!(pieces("Hi  yo!", Split::Words), vec![0..2, 4..7]);
        // An emoji is two UTF-16 units.
        assert_eq!(pieces("a😀b", Split::Chars), vec![0..1, 1..3, 3..4]);
    }
}
