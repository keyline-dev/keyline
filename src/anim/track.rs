//! Keyframe tracks, GSAP-style: values for some properties over time.
//!
//! ```json
//! {"scale": [1, 1.06, 1], "duration": 1.6, "repeat": -1, "ease": "sine.inOut"}
//! {"rotate": {"to": 8}, "delay": 2, "duration": 0.4, "ease": "back.out"}
//! {"rotate": {"from": "random(-90, 90)"}, "translate": {"from": ["random(-300, 300)", 0]}}
//! ```
//!
//! `random(lo, hi)` is GSAP's: each target (a layer, or each piece of split
//! text) gets its own value, the same on every render.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::ease::{Dir, Ease, Family, short};
use crate::scene::Color;

/// A value a property takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Val {
    /// A number: opacity, scale, rotation, blur.
    Num(f32),
    /// A pair: offset, skew.
    Pair([f32; 2]),
    /// A color.
    Color(Color),
}

/// The properties that animate, and the kind of value each takes. Only
/// ones that don't change layout: the layout runs once, time only redraws.
/// `draw` is the share of a stroke drawn (0–1); `count` is the number a
/// text's `{{n}}` shows.
pub const PROPS: &[(&str, Kind)] = &[
    ("opacity", Kind::Num),
    ("draw", Kind::Num),
    ("count", Kind::Num),
    ("scale", Kind::Num),
    ("rotate", Kind::Num),
    ("blur", Kind::Num),
    ("translate", Kind::Pair),
    ("skew", Kind::Pair),
    ("color", Kind::Color),
];

/// The kind of value a property takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A number.
    Num,
    /// `[x, y]`.
    Pair,
    /// A color string.
    Color,
}

/// A number as written: fixed, or `random(lo, hi)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum N {
    /// This number.
    Fix(f32),
    /// A number in `lo..hi`, chosen per target.
    Rand(f32, f32),
}

/// A value as written, before `random()` is resolved for a target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Spec {
    /// A number.
    Num(N),
    /// A pair.
    Pair([N; 2]),
    /// A color.
    Color(Color),
}

impl N {
    fn resolve(self, seed: u32, salt: u32) -> f32 {
        match self {
            N::Fix(v) => v,
            N::Rand(lo, hi) => lo + (hi - lo) * unit(seed, salt),
        }
    }
}

impl Spec {
    /// The value for the target `seed`; `salt` tells the random numbers of
    /// one target apart.
    fn resolve(self, seed: u32, salt: u32) -> Val {
        match self {
            Spec::Num(n) => Val::Num(n.resolve(seed, salt)),
            Spec::Pair([x, y]) => {
                Val::Pair([x.resolve(seed, salt), y.resolve(seed, salt ^ 0x9E37)])
            }
            Spec::Color(c) => Val::Color(c),
        }
    }
}

/// A number in 0–1 from a seed and a salt: the same inputs, the same number,
/// on every platform (a small integer hash, not the standard library's).
fn unit(seed: u32, salt: u32) -> f32 {
    let mut h = seed.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// A target's seed from its name (a layer id, with a piece number).
pub fn seed(name: &str) -> u32 {
    name.bytes().fold(0x811C_9DC5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    })
}

/// One property's keys: values spread over the track, or `from`/`to`
/// (a missing end is the layer's own value).
#[derive(Debug, Clone, PartialEq)]
pub enum Keys {
    /// Values, evenly spaced unless the track has `times`.
    List(Vec<Spec>),
    /// From one value to another.
    FromTo(Option<Spec>, Option<Spec>),
}

/// A set of properties animated together.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// The animated properties, by name.
    pub props: BTreeMap<String, Keys>,
    /// Where each value falls, 0–1 of `duration` (even when empty).
    pub times: Vec<f32>,
    /// Start, seconds.
    pub at: f32,
    /// Length of one play, seconds (1).
    pub duration: f32,
    /// Easing between each pair of values (`power1.inOut`).
    pub ease: Ease,
    /// Extra plays; −1 repeats forever (0).
    pub repeat: i32,
    /// Every other play runs backwards (false).
    pub yoyo: bool,
    /// With `count`: digits after the decimal point (0).
    pub decimals: u8,
    /// With `count`: put between thousands, e.g. `","` (none).
    pub separator: String,
}

impl Track {
    /// Where the track is at `t` seconds: 0–1 through one play, holding
    /// the first value before `at` and the last after the final play.
    fn progress(&self, t: f32) -> f32 {
        let local = t - self.at;
        if local <= 0.0 || self.duration <= 0.0 {
            return if local > 0.0 { 1.0 } else { 0.0 };
        }
        let plays = if self.repeat < 0 {
            f32::INFINITY
        } else {
            (self.repeat + 1) as f32
        };
        let mut play = (local / self.duration).floor();
        let mut frac = local / self.duration - play;
        if play >= plays {
            play = plays - 1.0;
            frac = 1.0;
        }
        if self.yoyo && play % 2.0 >= 1.0 {
            1.0 - frac
        } else {
            frac
        }
    }

    /// `prop`'s value at `t` for the target `seed`, given its own value
    /// `own`; `None` when this track doesn't animate `prop`.
    pub fn value(&self, prop: &str, own: Val, t: f32, seed: u32) -> Option<Val> {
        let p = self.progress(t);
        let salt = |i: usize| self::seed(prop) ^ (i as u32).wrapping_mul(0x2545_F491);
        Some(match self.props.get(prop)? {
            Keys::FromTo(from, to) => {
                let from = from.map_or(own, |f| f.resolve(seed, salt(0)));
                let to = to.map_or(own, |v| v.resolve(seed, salt(1)));
                lerp(from, to, self.ease.at(p))
            }
            Keys::List(specs) => match specs.as_slice() {
                [] => own,
                [only] => only.resolve(seed, salt(0)),
                _ => {
                    let vals: Vec<Val> = specs
                        .iter()
                        .enumerate()
                        .map(|(i, v)| v.resolve(seed, salt(i)))
                        .collect();
                    let n = vals.len() - 1;
                    let time = |i: usize| self.times.get(i).copied().unwrap_or(i as f32 / n as f32);
                    let k = (0..n).rfind(|&i| time(i) <= p).unwrap_or(0);
                    let span = (time(k + 1) - time(k)).max(1e-6);
                    let u = ((p - time(k)) / span).clamp(0.0, 1.0);
                    lerp(vals[k], vals[k + 1], self.ease.at(u))
                }
            },
        })
    }
}

/// `a` to `b` by `u` (which may pass 1 for overshooting eases).
fn lerp(a: Val, b: Val, u: f32) -> Val {
    let mix = |x: f32, y: f32| x + (y - x) * u;
    match (a, b) {
        (Val::Num(x), Val::Num(y)) => Val::Num(mix(x, y)),
        (Val::Pair([x0, y0]), Val::Pair([x1, y1])) => Val::Pair([mix(x0, x1), mix(y0, y1)]),
        (Val::Color(c0), Val::Color(c1)) => {
            let ch = |c: Color, s: u32| ((c.0 >> s) & 0xFF) as f32;
            let out = [24, 16, 8, 0].iter().fold(0u32, |acc, &s| {
                acc | ((mix(ch(c0, s), ch(c1, s)).round().clamp(0.0, 255.0) as u32) << s)
            });
            Val::Color(Color(out))
        }
        // Kinds are checked when the track is read; hold `a` otherwise.
        _ => a,
    }
}

/// A number, or `"random(lo, hi)"`.
fn n(v: &Value) -> Option<N> {
    if let Some(x) = v.as_f64() {
        return Some(N::Fix(x as f32));
    }
    let inner = v
        .as_str()?
        .trim()
        .strip_prefix("random(")?
        .strip_suffix(')')?;
    let (lo, hi) = inner.split_once(',')?;
    Some(N::Rand(lo.trim().parse().ok()?, hi.trim().parse().ok()?))
}

/// A value of `kind` from JSON.
fn val(kind: Kind, v: &Value) -> Option<Spec> {
    match kind {
        Kind::Num => n(v).map(Spec::Num),
        Kind::Pair => match v.as_array()?.as_slice() {
            [x, y] => Some(Spec::Pair([n(x)?, n(y)?])),
            _ => None,
        },
        Kind::Color => v.as_str().and_then(Color::parse).map(Spec::Color),
    }
}

fn keys(prop: &str, kind: Kind, v: &Value) -> Result<Keys, String> {
    let want = match kind {
        Kind::Num => "numbers",
        Kind::Pair => "[x, y] pairs",
        Kind::Color => "colors",
    };
    let bad = || format!("{prop}: a list of {want}, or {{from, to}}");
    let one = |v: &Value| val(kind, v).ok_or_else(bad);
    match v {
        Value::Object(o) => {
            if let Some(k) = o.keys().find(|k| !["from", "to"].contains(&k.as_str())) {
                return Err(format!("{prop}: unknown key {k}; use from and to"));
            }
            Ok(Keys::FromTo(
                o.get("from").map(one).transpose()?,
                o.get("to").map(one).transpose()?,
            ))
        }
        // A pair property's single value is itself an array: `[0, -20]`.
        Value::Array(a) if kind == Kind::Pair && val(kind, v).is_some() => {
            Ok(Keys::List(vec![one(&Value::Array(a.clone()))?]))
        }
        Value::Array(a) => a.iter().map(one).collect::<Result<_, _>>().map(Keys::List),
        other => one(other).map(|x| Keys::List(vec![x])),
    }
}

/// A track's keys that aren't properties: its timing, and `count`'s format.
const TIMING: &[&str] = &[
    "times",
    "delay",
    "duration",
    "ease",
    "repeat",
    "yoyo",
    "decimals",
    "separator",
];

impl<'de> Deserialize<'de> for Track {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let o = Map::<String, Value>::deserialize(d)?;
        let num = |k: &str, dflt: f32| o.get(k).and_then(Value::as_f64).map_or(dflt, |n| n as f32);
        let mut props = BTreeMap::new();
        for (k, v) in &o {
            if TIMING.contains(&k.as_str()) {
                continue;
            }
            let Some((_, kind)) = PROPS.iter().find(|(name, _)| name == k) else {
                let names: Vec<&str> = PROPS.iter().map(|(n, _)| *n).collect();
                return Err(D::Error::custom(format!(
                    "can't animate {k}; animate {} (layout fields stay fixed)",
                    names.join(", ")
                )));
            };
            props.insert(k.clone(), keys(k, *kind, v).map_err(D::Error::custom)?);
        }
        if props.is_empty() {
            return Err(D::Error::custom(
                "animate needs a property, e.g. {\"scale\": [1, 1.1, 1]}",
            ));
        }
        Ok(Track {
            props,
            times: o
                .get("times")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_f64)
                        .map(|t| t as f32)
                        .collect()
                })
                .unwrap_or_default(),
            at: num("delay", 0.0),
            duration: num("duration", 1.0),
            ease: o
                .get("ease")
                .map(|e| serde_json::from_value(e.clone()))
                .transpose()
                .map_err(D::Error::custom)?
                .unwrap_or(Ease::Curve(Family::Power(1), Dir::InOut)),
            repeat: o
                .get("repeat")
                .and_then(Value::as_i64)
                .map_or(0, |r| r.clamp(-1, 10_000) as i32),
            yoyo: o.get("yoyo").and_then(Value::as_bool).unwrap_or(false),
            decimals: o
                .get("decimals")
                .and_then(Value::as_u64)
                .map_or(0, |d| d.min(6) as u8),
            separator: o
                .get("separator")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        })
    }
}

impl Serialize for Track {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let num = |x: &N| match x {
            N::Fix(v) => serde_json::json!(short(*v)),
            N::Rand(lo, hi) => serde_json::json!(format!("random({lo}, {hi})")),
        };
        let v = |x: &Spec| match x {
            Spec::Num(n) => num(n),
            Spec::Pair(p) => serde_json::json!([num(&p[0]), num(&p[1])]),
            Spec::Color(c) => serde_json::json!(c.to_string()),
        };
        let mut o = Map::new();
        for (k, keys) in &self.props {
            o.insert(
                k.clone(),
                match keys {
                    Keys::List(vals) => Value::Array(vals.iter().map(v).collect()),
                    Keys::FromTo(from, to) => {
                        let mut ft = Map::new();
                        if let Some(f) = from {
                            ft.insert("from".into(), v(f));
                        }
                        if let Some(t) = to {
                            ft.insert("to".into(), v(t));
                        }
                        Value::Object(ft)
                    }
                },
            );
        }
        if !self.times.is_empty() {
            let times: Vec<f64> = self.times.iter().copied().map(short).collect();
            o.insert("times".into(), serde_json::json!(times));
        }
        if self.at != 0.0 {
            o.insert("delay".into(), serde_json::json!(short(self.at)));
        }
        if self.duration != 1.0 {
            o.insert("duration".into(), serde_json::json!(short(self.duration)));
        }
        if self.ease != Ease::Curve(Family::Power(1), Dir::InOut) {
            o.insert(
                "ease".into(),
                serde_json::to_value(self.ease).map_err(serde::ser::Error::custom)?,
            );
        }
        if self.repeat != 0 {
            o.insert("repeat".into(), self.repeat.into());
        }
        if self.yoyo {
            o.insert("yoyo".into(), true.into());
        }
        if self.decimals != 0 {
            o.insert("decimals".into(), self.decimals.into());
        }
        if !self.separator.is_empty() {
            o.insert("separator".into(), self.separator.clone().into());
        }
        o.serialize(s)
    }
}

#[cfg(test)]
mod tests {
    use super::{Track, Val};
    use crate::scene::Color;
    use serde_json::json;

    fn track(v: serde_json::Value) -> Track {
        serde_json::from_value(v).unwrap()
    }

    fn num(v: Option<Val>) -> f32 {
        match v {
            Some(Val::Num(n)) => n,
            other => panic!("not a number: {other:?}"),
        }
    }

    #[test]
    fn values_spread_over_the_duration_and_hold_at_the_ends() {
        let t = track(json!({"scale": [1, 2, 1], "delay": 1, "duration": 2, "ease": "none"}));
        let s = |secs: f32| num(t.value("scale", Val::Num(1.0), secs, 0));
        assert_eq!(s(0.0), 1.0, "holds the first value before it starts");
        assert_eq!(s(1.5), 1.5);
        assert_eq!(s(2.0), 2.0);
        assert_eq!(s(2.5), 1.5);
        assert_eq!(s(9.0), 1.0, "holds the last value after it ends");
        assert!(t.value("rotate", Val::Num(0.0), 1.5, 0).is_none());
    }

    #[test]
    fn repeat_and_yoyo_play_again_and_backwards() {
        let t = track(json!({"opacity": [0, 1], "repeat": -1, "yoyo": true, "ease": "none"}));
        let o = |secs: f32| num(t.value("opacity", Val::Num(1.0), secs, 0));
        assert!((o(0.25) - 0.25).abs() < 1e-5);
        assert!((o(1.25) - 0.75).abs() < 1e-5, "the second play runs back");
        assert!((o(100.25) - 0.25).abs() < 1e-4, "forever");
        let twice = track(json!({"opacity": [0, 1], "repeat": 1, "ease": "none"}));
        assert_eq!(num(twice.value("opacity", Val::Num(1.0), 5.0, 0)), 1.0);
    }

    #[test]
    fn from_to_defaults_to_the_layers_own_value_and_colors_blend() {
        let t = track(
            json!({"rotate": {"from": -40}, "translate": {"to": [0, -20]}, "ease": "none",
            "color": ["#000000", "#FF0000"]}),
        );
        assert_eq!(num(t.value("rotate", Val::Num(10.0), 0.5, 0)), -15.0);
        assert_eq!(
            t.value("translate", Val::Pair([0.0, 0.0]), 0.5, 0),
            Some(Val::Pair([0.0, -10.0]))
        );
        assert_eq!(
            t.value("color", Val::Color(Color(0xFF00_0000)), 0.5, 0),
            Some(Val::Color(Color(0xFF80_0000)))
        );
    }

    #[test]
    fn random_values_differ_per_target_and_repeat_per_render() {
        let t = track(json!({"rotate": {"from": "random(-90, 90)"}, "ease": "none"}));
        let r = |seed: u32| num(t.value("rotate", Val::Num(0.0), 0.0, seed));
        assert_eq!(r(1), r(1), "the same target, the same value");
        assert_ne!(r(1), r(2), "another target, another value");
        assert!((1..50).map(r).all(|v| (-90.0..=90.0).contains(&v)));
        assert_eq!(
            num(t.value("rotate", Val::Num(0.0), 1.0, 7)),
            0.0,
            "ends at its own value"
        );
        let saved = serde_json::to_value(&t).unwrap();
        assert_eq!(saved["rotate"]["from"], json!("random(-90, 90)"));
    }

    #[test]
    fn times_place_values_unevenly() {
        let t = track(json!({"scale": [1, 2, 3], "times": [0, 0.8, 1], "ease": "none"}));
        assert!((num(t.value("scale", Val::Num(1.0), 0.4, 0)) - 1.5).abs() < 1e-5);
        assert!((num(t.value("scale", Val::Num(1.0), 0.9, 0)) - 2.5).abs() < 1e-4);
    }

    #[test]
    fn layout_fields_and_bad_values_are_refused_in_one_line() {
        let e = serde_json::from_value::<Track>(json!({"fontSize": [10, 20]})).unwrap_err();
        assert!(
            e.to_string()
                .starts_with("can't animate fontSize; animate opacity"),
            "{e}"
        );
        let e = serde_json::from_value::<Track>(json!({"translate": [1, 2, 3]})).unwrap_err();
        assert!(e.to_string().contains("[x, y] pairs"), "{e}");
        let e = serde_json::from_value::<Track>(json!({"duration": 2})).unwrap_err();
        assert!(e.to_string().starts_with("animate needs a property"), "{e}");
    }

    #[test]
    fn tracks_round_trip_without_defaults() {
        let v = json!({"scale": [1, 1.5, 1], "duration": 1.6, "repeat": -1, "yoyo": true, "ease": "sine.inOut"});
        let saved = serde_json::to_value(track(v)).unwrap();
        assert_eq!(
            saved["duration"],
            json!(1.6),
            "written as given, not as f32 stores it"
        );
        assert!(saved.get("delay").is_none() && saved.get("times").is_none());
        assert_eq!(
            track(saved.clone()),
            track(serde_json::to_value(track(saved)).unwrap())
        );
    }
}
