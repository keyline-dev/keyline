//! Easing: how a value moves from 0 to 1, by GSAP's names (`power2.out`,
//! `back.out`, `elastic.inOut`, `steps(5)`). The curves are
//! `simple-easing`'s; CSS names and iOS-style words map onto them, since
//! agents reach for those too.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An easing curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ease {
    /// Constant speed (`none`, `linear`).
    Linear,
    /// A GSAP family, `in`, `out` or both.
    Curve(Family, Dir),
    /// Jumps in `n` equal steps (`steps(n)`).
    Steps(u16),
}

/// GSAP's curve families.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// `power1` … `power4`: quad … quint.
    Power(u8),
    /// A quarter sine wave.
    Sine,
    /// Exponential.
    Expo,
    /// A quarter circle.
    Circ,
    /// Pulls back before going, or overshoots before settling.
    Back,
    /// Springs past the end and wobbles back.
    Elastic,
    /// Bounces like a ball.
    Bounce,
}

/// Which end of the curve eases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    /// Slow start.
    In,
    /// Slow end.
    Out,
    /// Slow at both ends.
    InOut,
}

impl Ease {
    /// Progress at `t` (0–1): 0 at the start, 1 at the end; backs and
    /// elastics may pass 1 on the way.
    pub fn at(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Linear => t,
            Ease::Curve(f, d) => curve(f, d)(t),
            Ease::Steps(n) => {
                let n = f32::from(n.max(1));
                (t * n).floor() / n
            }
        }
    }
}

/// The `simple-easing` function for a family and direction. GSAP's
/// `power1`…`power4` are quad…quint.
fn curve(f: Family, d: Dir) -> fn(f32) -> f32 {
    use simple_easing as e;
    let [ease_in, ease_out, in_out]: [fn(f32) -> f32; 3] = match f {
        Family::Power(1) => [e::quad_in, e::quad_out, e::quad_in_out],
        Family::Power(2) => [e::cubic_in, e::cubic_out, e::cubic_in_out],
        Family::Power(3) => [e::quart_in, e::quart_out, e::quart_in_out],
        Family::Power(_) => [e::quint_in, e::quint_out, e::quint_in_out],
        Family::Sine => [e::sine_in, e::sine_out, e::sine_in_out],
        Family::Expo => [e::expo_in, e::expo_out, e::expo_in_out],
        Family::Circ => [e::circ_in, e::circ_out, e::circ_in_out],
        Family::Back => [e::back_in, e::back_out, e::back_in_out],
        Family::Elastic => [e::elastic_in, e::elastic_out, e::elastic_in_out],
        Family::Bounce => [e::bounce_in, e::bounce_out, e::bounce_in_out],
    };
    match d {
        Dir::In => ease_in,
        Dir::Out => ease_out,
        Dir::InOut => in_out,
    }
}

/// Reads a name: GSAP's, or a CSS or iOS word mapped to the nearest one.
fn named(s: &str) -> Option<Ease> {
    let s = s.trim();
    if let Some(n) = s.strip_prefix("steps(").and_then(|r| r.strip_suffix(')')) {
        return n.trim().parse().ok().map(Ease::Steps);
    }
    let alias = match s {
        "none" | "linear" => return Some(Ease::Linear),
        "ease-in" => "power1.in",
        "ease-out" => "power1.out",
        "ease" | "ease-in-out" => "power1.inOut",
        "smooth" => "power3.out",
        "snappy" => "expo.out",
        "bouncy" | "spring" => "back.out",
        other => other,
    };
    // A family alone is `.out`, as in GSAP.
    let (family, dir) = alias.split_once('.').unwrap_or((alias, "out"));
    let family = match family {
        "power0" => return Some(Ease::Linear),
        "power1" | "quad" => Family::Power(1),
        "power2" | "cubic" => Family::Power(2),
        "power3" | "quart" => Family::Power(3),
        "power4" | "quint" | "strong" => Family::Power(4),
        "sine" => Family::Sine,
        "expo" => Family::Expo,
        "circ" => Family::Circ,
        "back" => Family::Back,
        "elastic" => Family::Elastic,
        "bounce" => Family::Bounce,
        _ => return None,
    };
    let dir = match dir {
        "in" => Dir::In,
        "out" => Dir::Out,
        "inOut" | "in-out" | "inout" => Dir::InOut,
        _ => return None,
    };
    Some(Ease::Curve(family, dir))
}

impl<'de> Deserialize<'de> for Ease {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let v = Value::deserialize(d)?;
        v.as_str().and_then(named).ok_or_else(|| {
            D::Error::custom(format!(
                "ease like power2.out, back.out, elastic.inOut, sine.inOut, steps(5) or none; got {v}"
            ))
        })
    }
}

impl Serialize for Ease {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Ease::Linear => s.serialize_str("none"),
            Ease::Steps(n) => s.serialize_str(&format!("steps({n})")),
            Ease::Curve(f, d) => {
                let family = match f {
                    Family::Power(n) => format!("power{n}"),
                    Family::Sine => "sine".into(),
                    Family::Expo => "expo".into(),
                    Family::Circ => "circ".into(),
                    Family::Back => "back".into(),
                    Family::Elastic => "elastic".into(),
                    Family::Bounce => "bounce".into(),
                };
                let dir = match d {
                    Dir::In => "in",
                    Dir::Out => "out",
                    Dir::InOut => "inOut",
                };
                s.serialize_str(&format!("{family}.{dir}"))
            }
        }
    }
}

/// `v` as written, not as `f32` stores it (0.2, not 0.2000000029).
pub(super) fn short(v: f32) -> f64 {
    (f64::from(v) * 1e4).round() / 1e4
}

#[cfg(test)]
mod tests {
    use super::Ease;
    use serde_json::json;

    fn ease(v: serde_json::Value) -> Ease {
        serde_json::from_value(v).unwrap()
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn gsap_names_pick_the_right_curves() {
        // power2.out = 1 − (1 − t)³; power2 alone is .out, as in GSAP.
        assert!(close(ease(json!("power2.out")).at(0.5), 0.875));
        assert!(close(ease(json!("power2")).at(0.5), 0.875));
        assert!(close(ease(json!("power1.in")).at(0.5), 0.25));
        assert!(close(ease(json!("sine.inOut")).at(0.5), 0.5));
        let back = ease(json!("back.out"));
        assert!(
            (0..=20).map(|i| back.at(i as f32 / 20.0)).any(|v| v > 1.0),
            "overshoots"
        );
        for name in [
            "expo.in",
            "circ.out",
            "elastic.out",
            "bounce.out",
            "back.inOut",
            "power4.in",
        ] {
            let e = ease(json!(name));
            assert!(close(e.at(0.0), 0.0) && close(e.at(1.0), 1.0), "{name}");
        }
        assert!(close(ease(json!("steps(4)")).at(0.6), 0.5));
        assert!(close(ease(json!("none")).at(0.3), 0.3));
    }

    #[test]
    fn css_and_ios_words_map_to_gsap_curves() {
        assert_eq!(ease(json!("ease-in-out")), ease(json!("power1.inOut")));
        assert_eq!(ease(json!("bouncy")), ease(json!("back.out")));
        assert_eq!(ease(json!("smooth")), ease(json!("power3.out")));
    }

    #[test]
    fn names_round_trip_and_bad_ones_list_the_forms() {
        for v in [json!("power2.inOut"), json!("steps(3)"), json!("none")] {
            assert_eq!(serde_json::to_value(ease(v.clone())).unwrap(), v);
        }
        let e = serde_json::from_value::<Ease>(json!("power9.out")).unwrap_err();
        assert!(e.to_string().starts_with("ease like power2.out"), "{e}");
    }
}
