//! Shots: top-level frames that play one after another instead of stacking,
//! each joined to the one before by a transition. Times inside a shot count
//! from its own start. Layers outside the shots stay on for the whole video.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::ease::{Dir, Ease, Family, short};
use crate::scene::{Layer, Scene};

/// A shot's timing: how long it lasts and how it enters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotTime {
    /// Seconds.
    pub duration: f32,
    /// How it enters from the shot before (a cut when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<Transition>,
}

/// How one shot gives way to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// No transition.
    Cut,
    /// The new shot fades in over the old.
    Fade,
    /// The new shot slides in over the old, from the right.
    SlideLeft,
    /// From the left.
    SlideRight,
    /// From below.
    SlideUp,
    /// From above.
    SlideDown,
    /// The new shot pushes the old one out to the left.
    PushLeft,
    /// To the right.
    PushRight,
    /// Upwards.
    PushUp,
    /// Downwards.
    PushDown,
    /// A moving edge reveals the new shot, right to left.
    WipeLeft,
    /// Left to right.
    WipeRight,
    /// Bottom to top.
    WipeUp,
    /// Top to bottom.
    WipeDown,
    /// The old shot grows and fades as the new one appears.
    Zoom,
}

/// A transition: `"push-left"` or `{"type": "fade", "duration": 0.8, "ease": "power2.inOut"}`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    /// Which one.
    pub kind: Kind,
    /// Seconds, overlapping the two shots (0.5).
    pub duration: f32,
    /// `power2.inOut` unless given.
    pub ease: Ease,
}

const LENGTH: f32 = 0.5;

impl Transition {
    /// How long the two shots overlap, seconds: none for a cut.
    pub fn overlap(self) -> f32 {
        if self.kind == Kind::Cut {
            0.0
        } else {
            self.duration
        }
    }
}
const EASE: Ease = Ease::Curve(Family::Power(2), Dir::InOut);

impl<'de> Deserialize<'de> for Transition {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let v = Value::deserialize(d)?;
        let kind = |v: &Value| {
            serde_json::from_value::<Kind>(v.clone()).map_err(|_| {
                D::Error::custom(format!(
                    "unknown transition {v}; use cut, fade, slide-*, push-*, wipe-* (left, right, up, down) or zoom"
                ))
            })
        };
        match &v {
            Value::String(_) => Ok(Transition {
                kind: kind(&v)?,
                duration: LENGTH,
                ease: EASE,
            }),
            Value::Object(o) => {
                if let Some(k) = o
                    .keys()
                    .find(|k| !["type", "duration", "ease"].contains(&k.as_str()))
                {
                    return Err(D::Error::custom(format!(
                        "unknown field {k}; use type, duration, ease"
                    )));
                }
                Ok(Transition {
                    kind: kind(
                        o.get("type")
                            .ok_or_else(|| D::Error::custom("needs a type"))?,
                    )?,
                    duration: o
                        .get("duration")
                        .map(|d| {
                            d.as_f64()
                                .ok_or_else(|| D::Error::custom("duration is seconds"))
                        })
                        .transpose()?
                        .map_or(LENGTH, |d| d as f32),
                    ease: o
                        .get("ease")
                        .map(|e| serde_json::from_value(e.clone()))
                        .transpose()
                        .map_err(D::Error::custom)?
                        .unwrap_or(EASE),
                })
            }
            _ => Err(D::Error::custom(
                "a transition name or {type, duration, ease}",
            )),
        }
    }
}

impl Serialize for Transition {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.duration == LENGTH && self.ease == EASE {
            return self.kind.serialize(s);
        }
        let mut o = serde_json::Map::new();
        o.insert(
            "type".into(),
            serde_json::to_value(self.kind).map_err(serde::ser::Error::custom)?,
        );
        if self.duration != LENGTH {
            o.insert("duration".into(), short(self.duration).into());
        }
        if self.ease != EASE {
            o.insert(
                "ease".into(),
                serde_json::to_value(self.ease).map_err(serde::ser::Error::custom)?,
            );
        }
        o.serialize(s)
    }
}

/// Each top-level shot's `(index in layers, start, duration, transition in)`.
pub fn timeline(scene: &Scene) -> Vec<(usize, f32, f32, Option<Transition>)> {
    let mut out = Vec::new();
    let mut end = 0.0_f32;
    for (i, l) in scene.layers.iter().enumerate() {
        if let Some(shot) = &l.time.shot {
            let overlap = if out.is_empty() {
                0.0
            } else {
                shot.transition.map_or(0.0, Transition::overlap)
            };
            let start = (end - overlap).max(0.0);
            out.push((i, start, shot.duration, shot.transition));
            end = start + shot.duration;
        }
    }
    out
}

/// How long the scene plays: its `duration`, else where its last shot ends.
pub fn length(scene: &Scene) -> Option<f32> {
    scene
        .duration
        .or_else(|| timeline(scene).last().map(|(_, start, dur, _)| start + dur))
}

/// Moves each shot to where it is at `t`, for an output `travel` px wide
/// and `rise` px tall (master px): hidden outside its time, and moved by
/// the transitions into it and into the shot after it. Returns each
/// visible shot's `(index, start, duration)`, for its layers' own clocks.
pub fn place(scene: &mut Scene, t: f32, (travel, rise): (f32, f32)) -> Vec<(usize, f32, f32)> {
    let shots = timeline(scene);
    let mut visible = Vec::new();
    for (n, &(i, start, dur, into)) in shots.iter().enumerate() {
        let l = &mut scene.layers[i];
        if t < start || t >= start + dur {
            l.hidden = true;
            continue;
        }
        l.hidden = false;
        visible.push((i, start, dur));
        // Entering: the transition into this shot.
        if let Some(tr) = into.filter(|tr| n > 0 && t < start + tr.overlap()) {
            enter(
                l,
                tr,
                tr.ease.at((t - start) / tr.duration.max(1e-3)),
                travel,
                rise,
            );
        }
        // Leaving: the transition into the next shot has begun.
        if let Some(&(_, next, _, Some(tr))) = shots.get(n + 1)
            && t >= next
        {
            leave(
                l,
                tr,
                tr.ease.at((t - next) / tr.duration.max(1e-3)),
                travel,
                rise,
            );
        }
    }
    visible
}

/// The incoming shot, `p` of the way through its transition.
fn enter(l: &mut Layer, tr: Transition, p: f32, w: f32, h: f32) {
    let q = 1.0 - p;
    match tr.kind {
        Kind::Cut => {}
        Kind::Fade | Kind::Zoom => l.opacity *= p.clamp(0.0, 1.0),
        Kind::SlideLeft | Kind::PushLeft => l.look.offset[0] += w * q,
        Kind::SlideRight | Kind::PushRight => l.look.offset[0] -= w * q,
        Kind::SlideUp | Kind::PushUp => l.look.offset[1] += h * q,
        Kind::SlideDown | Kind::PushDown => l.look.offset[1] -= h * q,
        Kind::WipeLeft => l.mask = wipe(90.0, 1.0 - p, false),
        Kind::WipeRight => l.mask = wipe(90.0, p, true),
        Kind::WipeUp => l.mask = wipe(180.0, 1.0 - p, false),
        Kind::WipeDown => l.mask = wipe(180.0, p, true),
    }
}

/// The outgoing shot, while the next one's transition is `p` through.
fn leave(l: &mut Layer, tr: Transition, p: f32, w: f32, h: f32) {
    match tr.kind {
        Kind::PushLeft => l.look.offset[0] -= w * p,
        Kind::PushRight => l.look.offset[0] += w * p,
        Kind::PushUp => l.look.offset[1] -= h * p,
        Kind::PushDown => l.look.offset[1] += h * p,
        // The new shot fades in on top, so the old one needn't fade.
        Kind::Zoom => l.look.scale *= 1.0 + 0.25 * p,
        _ => {}
    }
}

/// A mask with a hard edge at `edge` (0–1) along `angle`: shown before the
/// edge when `before`, else after it.
fn wipe(angle: f32, edge: f32, before: bool) -> Option<crate::scene::Mask> {
    let e = edge.clamp(0.0, 1.0);
    let (a, b) = if before {
        ("#000000", "#00000000")
    } else {
        ("#00000000", "#000000")
    };
    serde_json::from_value(json!({"angle": angle, "stops": [
        {"offset": 0, "color": a}, {"offset": e, "color": a}, {"offset": e, "color": b}, {"offset": 1, "color": b}]}))
    .ok()
}

#[cfg(test)]
mod tests {
    use super::{Transition, length, place, timeline};
    use crate::scene::Scene;
    use serde_json::json;

    fn scene() -> Scene {
        serde_json::from_value(json!({"width": 100, "height": 50,
            "sizes": [{"id": "a", "width": 100, "height": 50}], "layers": [
            {"id": "one", "type": "frame", "shot": {"duration": 1}},
            {"id": "two", "type": "frame", "shot": {"duration": 1, "transition": {"type": "push-left", "duration": 0.4, "ease": "none"}}},
            {"id": "three", "type": "frame", "shot": {"duration": 1, "transition": "fade"}},
            {"id": "logo", "type": "rect"}]}))
        .unwrap()
    }

    #[test]
    fn shots_follow_each_other_overlapping_by_their_transitions() {
        let s = scene();
        let starts: Vec<f32> = timeline(&s).iter().map(|t| t.1).collect();
        assert_eq!(starts, [0.0, 0.6, 1.1]);
        assert_eq!(length(&s), Some(2.1));
    }

    #[test]
    fn a_push_moves_both_shots_and_others_hide() {
        let mut s = scene();
        let visible = place(&mut s, 0.8, (100.0, 50.0));
        assert_eq!(visible.iter().map(|v| v.0).collect::<Vec<_>>(), [0, 1]);
        // Halfway through the 0.4 s push from 0.6 s.
        assert!((s.layers[1].look.offset[0] - 50.0).abs() < 1e-3);
        assert!((s.layers[0].look.offset[0] + 50.0).abs() < 1e-3);
        assert!(
            s.layers[2].hidden && !s.layers[3].hidden,
            "a later shot waits; the logo stays"
        );
    }

    #[test]
    fn a_cut_named_or_not_joins_shots_end_to_end() {
        let mut s = scene();
        s.layers[1].time.shot.as_mut().unwrap().transition =
            Some(serde_json::from_value(json!("cut")).unwrap());
        assert_eq!(timeline(&s)[1].1, 1.0);
    }

    #[test]
    fn transitions_read_short_and_full_and_refuse_unknown_names() {
        let t: Transition = serde_json::from_value(json!("wipe-right")).unwrap();
        assert_eq!(serde_json::to_value(t).unwrap(), json!("wipe-right"));
        let e = serde_json::from_value::<Transition>(json!("spin")).unwrap_err();
        assert!(e.to_string().starts_with("unknown transition"), "{e}");
        let e = serde_json::from_value::<Transition>(json!({"type": "fade", "duration": "1s"}))
            .unwrap_err();
        assert!(e.to_string().contains("seconds"), "{e}");
    }
}
