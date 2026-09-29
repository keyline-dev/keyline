//! Named enter and exit effects: `"enter": "fade-up"`,
//! `"exit": {"effect": "fade", "delay": 7}`. An effect moves a layer between an
//! "away" state (transparent, shifted, shrunk or blurred) and where it rests.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ease::{Dir, Ease, Family, short};

/// A named effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Effect {
    /// Opacity only.
    Fade,
    /// Fades while rising `distance` into place.
    FadeUp,
    /// Fades while dropping into place.
    FadeDown,
    /// Fades while moving left into place.
    FadeLeft,
    /// Fades while moving right into place.
    FadeRight,
    /// Grows from 0.6 with a slight overshoot, fading in.
    Pop,
    /// Grows from 0.85, fading in.
    ZoomIn,
    /// Shrinks from 1.15, fading in.
    ZoomOut,
    /// Sharpens from a 12 px blur, fading in.
    BlurIn,
}

/// How far from rest a layer is drawn: what an effect changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Away {
    /// Multiplies the layer's opacity.
    pub opacity: f32,
    /// Added to its offset, master px.
    pub offset: [f32; 2],
    /// Multiplies its scale.
    pub scale: f32,
    /// Added to its blur, px.
    pub blur: f32,
}

impl Away {
    /// At rest: no change.
    pub const REST: Away = Away {
        opacity: 1.0,
        offset: [0.0, 0.0],
        scale: 1.0,
        blur: 0.0,
    };

    /// Both changes at once.
    pub fn then(self, o: Away) -> Away {
        Away {
            opacity: self.opacity * o.opacity,
            offset: [self.offset[0] + o.offset[0], self.offset[1] + o.offset[1]],
            scale: self.scale * o.scale,
            blur: self.blur + o.blur,
        }
    }
}

impl Effect {
    /// The state `p` of the way from away (0) to rest (1); an overshooting
    /// ease passes 1 and comes back.
    fn state(self, p: f32, distance: f32) -> Away {
        // Snap the ends, so "away" is fully away and "rest" exactly rest.
        let p = if p.abs() < 1e-4 { 0.0 } else { p };
        let q = 1.0 - p;
        let fade = Away {
            opacity: p.clamp(0.0, 1.0),
            ..Away::REST
        };
        let shift = |dx: f32, dy: f32| Away {
            offset: [dx * distance * q, dy * distance * q],
            ..fade
        };
        let grow = |from: f32| Away {
            scale: from + (1.0 - from) * p,
            ..fade
        };
        match self {
            Effect::Fade => fade,
            Effect::FadeUp => shift(0.0, 1.0),
            Effect::FadeDown => shift(0.0, -1.0),
            Effect::FadeLeft => shift(1.0, 0.0),
            Effect::FadeRight => shift(-1.0, 0.0),
            Effect::Pop => grow(0.6),
            Effect::ZoomIn => grow(0.85),
            Effect::ZoomOut => grow(1.15),
            Effect::BlurIn => Away {
                blur: 12.0 * q.max(0.0),
                ..fade
            },
        }
    }
}

/// An effect with its timing: `"fade-up"`, or
/// `{"effect": "pop", "delay": 1.2, "duration": 0.5, "ease": "back.out", "distance": 60}`.
#[derive(Debug, Clone, PartialEq)]
pub struct Motion {
    /// Which effect.
    pub effect: Effect,
    /// Start, seconds: `in` defaults to 0, `out` to ending with the scene.
    pub at: Option<f32>,
    /// Seconds (0.6).
    pub duration: f32,
    /// The ease: `power2.out` entering (`back.out` for `pop`), `power2.in`
    /// leaving.
    pub ease: Option<Ease>,
    /// How far a directional effect travels, master px (40).
    pub distance: f32,
}

const DURATION: f32 = 0.6;
const DISTANCE: f32 = 40.0;

impl Motion {
    /// Entering: away before `at`, arriving over `duration`, then at rest.
    pub fn enter(&self, t: f32) -> Away {
        let ease = self.ease.unwrap_or(if self.effect == Effect::Pop {
            Ease::Curve(Family::Back, Dir::Out)
        } else {
            Ease::Curve(Family::Power(2), Dir::Out)
        });
        let p = ease.at(self.progress(self.at.unwrap_or(0.0), t));
        self.effect.state(p, self.distance)
    }

    /// Leaving: at rest until `at` (by default so it ends at `end`), then
    /// away.
    pub fn leave(&self, t: f32, end: f32) -> Away {
        let ease = self.ease.unwrap_or(Ease::Curve(Family::Power(2), Dir::In));
        let at = self.at.unwrap_or(end - self.duration);
        let p = ease.at(self.progress(at, t));
        self.effect.state(1.0 - p, self.distance)
    }

    /// 0–1 through the effect at `t`.
    fn progress(&self, at: f32, t: f32) -> f32 {
        if self.duration <= 0.0 {
            return if t >= at { 1.0 } else { 0.0 };
        }
        ((t - at) / self.duration).clamp(0.0, 1.0)
    }

    /// When the effect starts, when it has an explicit `at`.
    pub fn start(&self) -> Option<f32> {
        self.at
    }

    /// The same effect starting `delay` seconds later (for `stagger`).
    pub fn delayed(&self, delay: f32) -> Motion {
        Motion {
            at: Some(self.at.unwrap_or(0.0) + delay),
            ..self.clone()
        }
    }
}

impl<'de> Deserialize<'de> for Motion {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let v = Value::deserialize(d)?;
        let effect = |v: &Value| -> Result<Effect, D::Error> {
            serde_json::from_value(v.clone()).map_err(|_| {
                D::Error::custom(format!(
                    "unknown effect {v}; use fade, fade-up, fade-down, fade-left, fade-right, pop, zoom-in, zoom-out or blur-in"
                ))
            })
        };
        match &v {
            Value::String(_) => Ok(Motion {
                effect: effect(&v)?,
                at: None,
                duration: DURATION,
                ease: None,
                distance: DISTANCE,
            }),
            Value::Object(o) => {
                if let Some(k) = o.keys().find(|k| {
                    !["effect", "delay", "duration", "ease", "distance"].contains(&k.as_str())
                }) {
                    return Err(D::Error::custom(format!(
                        "unknown field {k}; use effect, delay, duration, ease, distance"
                    )));
                }
                let num = |k: &str| o.get(k).and_then(Value::as_f64).map(|n| n as f32);
                Ok(Motion {
                    effect: effect(
                        o.get("effect")
                            .ok_or_else(|| D::Error::custom("needs an effect"))?,
                    )?,
                    at: num("delay"),
                    duration: num("duration").unwrap_or(DURATION),
                    ease: o
                        .get("ease")
                        .map(|e| serde_json::from_value(e.clone()))
                        .transpose()
                        .map_err(D::Error::custom)?,
                    distance: num("distance").unwrap_or(DISTANCE),
                })
            }
            _ => Err(D::Error::custom(
                "an effect name, or {effect, delay, duration, ease, distance}",
            )),
        }
    }
}

impl Serialize for Motion {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let plain = self.at.is_none()
            && self.duration == DURATION
            && self.ease.is_none()
            && self.distance == DISTANCE;
        if plain {
            return self.effect.serialize(s);
        }
        let mut o = serde_json::Map::new();
        o.insert(
            "effect".into(),
            serde_json::to_value(self.effect).map_err(serde::ser::Error::custom)?,
        );
        if let Some(at) = self.at {
            o.insert("delay".into(), short(at).into());
        }
        if self.duration != DURATION {
            o.insert("duration".into(), short(self.duration).into());
        }
        if let Some(e) = self.ease {
            o.insert(
                "ease".into(),
                serde_json::to_value(e).map_err(serde::ser::Error::custom)?,
            );
        }
        if self.distance != DISTANCE {
            o.insert("distance".into(), short(self.distance).into());
        }
        o.serialize(s)
    }
}

#[cfg(test)]
mod tests {
    use super::{Away, Motion};
    use serde_json::json;

    fn motion(v: serde_json::Value) -> Motion {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn entering_goes_from_away_to_rest() {
        let m = motion(json!({"effect": "fade-up", "delay": 1, "duration": 0.5, "ease": "none"}));
        assert_eq!(m.enter(0.0).opacity, 0.0, "hidden before it starts");
        assert_eq!(m.enter(0.0).offset, [0.0, 40.0], "below its place");
        let mid = m.enter(1.25);
        assert!((mid.opacity - 0.5).abs() < 1e-5 && (mid.offset[1] - 20.0).abs() < 1e-4);
        assert_eq!(m.enter(9.0), Away::REST);
    }

    #[test]
    fn leaving_ends_with_the_scene_unless_timed() {
        let m = motion(json!("fade"));
        assert_eq!(m.leave(0.0, 8.0), Away::REST);
        assert_eq!(m.leave(8.0, 8.0).opacity, 0.0, "gone at the end");
        assert!(m.leave(7.7, 8.0).opacity > 0.0 && m.leave(7.7, 8.0).opacity < 1.0);
        let early = motion(json!({"effect": "fade", "delay": 2, "ease": "none"}));
        assert_eq!(early.leave(3.0, 8.0).opacity, 0.0);
    }

    #[test]
    fn pop_overshoots_and_blur_sharpens() {
        let pop = motion(json!("pop"));
        let scales: Vec<f32> = (0..=12).map(|i| pop.enter(i as f32 * 0.05).scale).collect();
        assert!(
            scales[0] < 0.7 && scales.iter().any(|s| *s > 1.0),
            "{scales:?}"
        );
        let blur = motion(json!({"effect": "blur-in", "ease": "none"}));
        assert_eq!(blur.enter(0.0).blur, 12.0);
        assert_eq!(blur.enter(0.6).blur, 0.0);
    }

    #[test]
    fn stagger_delays_the_start() {
        let m = motion(json!({"effect": "fade", "ease": "smooth"}));
        assert!(m.enter(0.3).opacity < 1.0);
        assert_eq!(m.delayed(0.4).start(), Some(0.4));
    }

    #[test]
    fn effects_read_short_and_full_and_refuse_unknown_names() {
        assert_eq!(
            serde_json::to_value(motion(json!("pop"))).unwrap(),
            json!("pop")
        );
        let full = json!({"effect": "fade-left", "delay": 1.2, "distance": 80});
        let saved = serde_json::to_value(motion(full.clone())).unwrap();
        assert_eq!(motion(saved.clone()), motion(full));
        assert_eq!(saved["delay"], json!(1.2));
        let e = serde_json::from_value::<Motion>(json!("slide-in")).unwrap_err();
        assert!(e.to_string().starts_with("unknown effect"), "{e}");
    }
}
