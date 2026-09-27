//! Deserializing fields that take several shapes (a number or an array, a
//! word or a number), with errors that say what was expected. serde's
//! `untagged` would answer "data did not match any variant", and an agent
//! that can't tell what's wrong re-sends its whole batch to guess.

use serde::de::Error;
use serde_json::Value;

/// `N` numbers from a JSON array of exactly `N` numbers.
pub(super) fn floats<const N: usize>(v: &Value) -> Option<[f32; N]> {
    let a = v.as_array()?;
    if a.len() != N {
        return None;
    }
    let mut out = [0.0; N];
    for (o, x) in out.iter_mut().zip(a) {
        *o = x.as_f64()? as f32;
    }
    Some(out)
}

/// A number as `f32`.
pub(super) fn float(v: &Value) -> Option<f32> {
    v.as_f64().map(|n| n as f32)
}

/// "expected {what}, got {value}" with the value kept short.
pub(super) fn expected<E: Error>(what: &str, got: &Value) -> E {
    let mut s = got.to_string();
    if s.len() > 40 {
        s.truncate(37);
        s.push_str("...");
    }
    E::custom(format!("expected {what}, got {s}"))
}
