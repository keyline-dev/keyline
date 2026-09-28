//! Time: easing curves, keyframe tracks, and named enter and exit effects,
//! evaluated at a moment to give the values a layer is drawn with. Only
//! fields that don't change layout move, so a scene at any moment lays out
//! exactly as it does at rest.

pub mod ease;
pub mod motion;
pub mod shots;
pub mod track;

use serde::{Deserialize, Serialize};

use crate::scene::{Kind, Layer, OneOrMany, Scene};
use motion::{Away, Motion};
use track::{Track, Val};

/// A layer's timing: how it enters and leaves, and what else moves.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LayerTime {
    /// How it enters: an effect name or `{effect, at, duration, ease,
    /// distance}`. Hidden before it.
    #[serde(rename = "in", default, skip_serializing_if = "Option::is_none")]
    pub enter: Option<Motion>,
    /// How it leaves; gone after it. Ends with the scene unless timed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out: Option<Motion>,
    /// Keyframe tracks: `{"scale": [1, 1.06, 1], "repeat": -1}`, or a list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate: Option<OneOrMany<Track>>,
    /// On a frame or `use`: its children or instances take its `in` one
    /// after another, this many seconds apart, instead of it entering whole.
    /// On split text: its pieces do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stagger: Option<f32>,
    /// On text: animate each letter (`chars`) or word (`words`) on its
    /// own, GSAP's SplitText; the text stays laid out as one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split: Option<Split>,
    /// On a top-level frame: it's a shot, playing after the one before
    /// (`{duration, transition}`) instead of stacking on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shot: Option<shots::ShotTime>,
    /// Set when drawing a moment of split text: the time and the scene's
    /// end, for the renderer to move each piece. Never stored.
    #[serde(skip)]
    pub moment: Option<(f32, f32)>,
}

/// How split text is cut into pieces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Split {
    /// Each letter (spaces are skipped).
    Chars,
    /// Each word.
    Words,
}

impl LayerTime {
    /// True when nothing about the layer moves.
    pub fn is_still(&self) -> bool {
        *self == LayerTime::default()
    }
}

/// `scene` as drawn `t` seconds in at `size` (which sets how far slides and
/// pushes travel). Call on a resolved scene (styles and components applied).
pub fn at_time(scene: &Scene, t: f32, size: &crate::scene::Size) -> Scene {
    let end = shots::length(scene).unwrap_or(0.0);
    let mut out = scene.clone();
    let k = size.scale.max(1e-3);
    let visible = shots::place(&mut out, t, (size.width / k, size.height / k));
    for (i, l) in out.layers.iter_mut().enumerate() {
        match (l.time.shot.is_some(), visible.iter().find(|v| v.0 == i)) {
            // A shot's layers keep its own clock; its clips start with it.
            (true, Some(&(_, start, dur))) => {
                delay_clips(l, start);
                apply(l, t - start, dur, None);
            }
            (true, None) => {}
            (false, _) => apply(l, t, end, None),
        }
    }
    out
}

/// Starts the clips in `l` `by` seconds later: a shot's clips play from
/// the shot's start.
fn delay_clips(l: &mut Layer, by: f32) {
    if let Kind::Video { delay, .. } = &mut l.kind {
        *delay += by;
    }
    for c in l.kind.children_mut().into_iter().flatten() {
        delay_clips(c, by);
    }
}

/// Moves `l` (and its children) to where it is at `t`. `given` is an
/// enter effect handed down by a staggering parent.
fn apply(l: &mut Layer, t: f32, end: f32, given: Option<Motion>) {
    // Split text moves piece by piece, in the renderer.
    if l.time.split.is_some() && matches!(l.kind, Kind::Text { .. }) {
        if l.time.enter.is_none() {
            l.time.enter = given;
        }
        l.time.moment = Some((t, end));
        return;
    }
    let time = l.time.clone();
    let enter = time.enter.or(given);
    // A staggering layer hands its entrance to its children instead.
    let (own_enter, handed) = match (time.stagger, enter) {
        (Some(gap), Some(m)) if l.kind.children().is_some() => (None, Some((m, gap))),
        (_, m) => (m, None),
    };
    let seed = track::seed(&l.id);
    for track in time.animate.iter().flat_map(OneOrMany::as_slice) {
        set_tracks(l, track, t, seed);
    }
    let mut away = Away::REST;
    if let Some(m) = own_enter {
        away = away.then(m.enter(t));
    }
    if let Some(m) = &time.out {
        away = away.then(m.leave(t, end));
    }
    l.opacity *= away.opacity;
    l.look.offset = [
        l.look.offset[0] + away.offset[0],
        l.look.offset[1] + away.offset[1],
    ];
    l.look.scale *= away.scale;
    l.look.blur += away.blur;
    if let Some(children) = l.kind.children_mut() {
        for (i, c) in children.iter_mut().enumerate() {
            let child = handed.as_ref().map(|(m, gap)| m.delayed(i as f32 * gap));
            apply(c, t, end, child);
        }
    }
}

/// Sets every property `track` animates to its value at `t`.
fn set_tracks(l: &mut Layer, track: &Track, t: f32, seed: u32) {
    let num = |v: Option<Val>, own: f32| match v {
        Some(Val::Num(n)) => n,
        _ => own,
    };
    let pair = |v: Option<Val>, own: [f32; 2]| match v {
        Some(Val::Pair(p)) => p,
        _ => own,
    };
    l.opacity = num(
        track.value("opacity", Val::Num(l.opacity), t, seed),
        l.opacity,
    );
    l.rotation = num(
        track.value("rotation", Val::Num(l.rotation), t, seed),
        l.rotation,
    );
    l.look.scale = num(
        track.value("scale", Val::Num(l.look.scale), t, seed),
        l.look.scale,
    );
    l.look.blur = num(
        track.value("blur", Val::Num(l.look.blur), t, seed),
        l.look.blur,
    );
    l.look.offset = pair(
        track.value("offset", Val::Pair(l.look.offset), t, seed),
        l.look.offset,
    );
    l.look.skew = pair(
        track.value("skew", Val::Pair(l.look.skew), t, seed),
        l.look.skew,
    );
    set_color(&mut l.kind, track, t, seed);
}

/// Sets the layer's own color field, when `track` animates `color`.
fn set_color(kind: &mut Kind, track: &Track, t: f32, seed: u32) {
    let black = crate::scene::Color(0xFF00_0000);
    let value = |own: crate::scene::Color| match track.value("color", Val::Color(own), t, seed) {
        Some(Val::Color(c)) => Some(c),
        _ => None,
    };
    match kind {
        Kind::Rect { color, .. }
        | Kind::Ellipse { color, .. }
        | Kind::Polygon { color, .. }
        | Kind::Path { color, .. }
        | Kind::Frame { color, .. } => {
            if let Some(c) = value(color.unwrap_or(black)) {
                *color = Some(c);
            }
        }
        Kind::Text { color, .. } | Kind::Line { color, .. } | Kind::Icon { color, .. } => {
            if let Some(c) = value(*color) {
                *color = c;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
