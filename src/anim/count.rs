//! The `count` track: a number in a text's `{n}` that counts as it plays.
//!
//! ```json
//! {"type": "text", "text": "{n}+ adopted", "animate": {"count": [0, 1250], "separator": ","}}
//! ```
//!
//! Layout measures the text with its widest value, so the box holds still
//! while the number changes; the renderer draws the number of the moment.

use super::track::{Keys, N, Spec, Track};
use crate::scene::{Kind, Layer, OneOrMany};

/// Where the number goes in the text.
pub const SLOT: &str = "{n}";

/// `v` with `decimals` places and `sep` between thousands. With `"."`
/// between thousands the decimal mark is a comma, as in German.
pub fn format(v: f32, decimals: u8, sep: &str) -> String {
    let fixed = format!("{:.*}", usize::from(decimals), v.abs());
    let (int, frac) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let mut out = String::with_capacity(fixed.len() + 4);
    // "-0" reads oddly: a value that rounds to zero has no sign.
    if v < 0.0 && fixed.bytes().any(|b| (b'1'..=b'9').contains(&b)) {
        out.push('-');
    }
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push_str(sep);
        }
        out.push(c);
    }
    if !frac.is_empty() {
        out.push(if sep == "." { ',' } else { '.' });
        out.push_str(frac);
    }
    out
}

/// The layer's count tracks, in order; empty when it doesn't count.
fn tracks(l: &Layer) -> impl Iterator<Item = &Track> {
    l.time
        .animate
        .iter()
        .flat_map(OneOrMany::as_slice)
        .filter(|t| t.props.contains_key("count"))
}

/// The text of `l` with `{n}` filled in: the number of the moment when
/// `now` and one is set, else the widest of its values (what layout
/// measures). `None` when `l` isn't counting text.
pub fn text(l: &Layer, now: bool) -> Option<String> {
    let Kind::Text { text, .. } = &l.kind else {
        return None;
    };
    let first = tracks(l).next()?;
    if !text.contains(SLOT) {
        return None;
    }
    let fmt = |v: f32| format(v, first.decimals, &first.separator);
    let shown = match l.time.count.filter(|_| now) {
        Some(v) => fmt(v),
        None => widest(tracks(l).flat_map(values).map(fmt)),
    };
    Some(text.replace(SLOT, &shown))
}

/// The longest of `shown`, the last on a tie: a count's end, usually.
fn widest(shown: impl Iterator<Item = String>) -> String {
    shown.fold(String::new(), |best, s| {
        if s.chars().count() >= best.chars().count() {
            s
        } else {
            best
        }
    })
}

/// Every number a count track can reach: its keys (a missing end is 0, a
/// random one either bound).
fn values(t: &Track) -> Vec<f32> {
    let n = |s: Option<&Spec>| match s {
        Some(Spec::Num(N::Fix(v))) => vec![*v],
        Some(Spec::Num(N::Rand(lo, hi))) => vec![*lo, *hi],
        _ => vec![0.0],
    };
    match t.props.get("count") {
        Some(Keys::List(specs)) => specs.iter().flat_map(|s| n(Some(s))).collect(),
        Some(Keys::FromTo(from, to)) => [n(from.as_ref()), n(to.as_ref())].concat(),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{format, text};
    use crate::scene::Layer;
    use serde_json::json;

    #[test]
    fn numbers_take_decimals_and_thousands_separators() {
        assert_eq!(format(1250.0, 0, ","), "1,250");
        assert_eq!(format(1_234_567.0, 2, ","), "1,234,567.00");
        assert_eq!(format(12_345.67, 2, ","), "12,345.67");
        assert_eq!(format(999.6, 0, ""), "1000");
        assert_eq!(format(1234.5, 1, "."), "1.234,5", "a dot between thousands");
        assert_eq!(format(-1500.0, 0, " "), "-1 500");
        assert_eq!(format(-0.2, 0, ""), "0", "no sign on zero");
        assert_eq!(format(12.0, 0, ","), "12");
    }

    fn layer(v: serde_json::Value) -> Layer {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn layout_measures_the_widest_value_and_drawing_the_moment() {
        let mut l = layer(json!({"type": "text", "text": "{n}+ adopted",
            "animate": {"count": [0, 1250], "separator": ","}}));
        assert_eq!(text(&l, false).as_deref(), Some("1,250+ adopted"));
        assert_eq!(
            text(&l, true).as_deref(),
            Some("1,250+ adopted"),
            "at rest: its end"
        );
        l.time.count = Some(42.4);
        assert_eq!(text(&l, true).as_deref(), Some("42+ adopted"));
        assert_eq!(text(&l, false).as_deref(), Some("1,250+ adopted"));
        let down =
            layer(json!({"type": "text", "text": "T-{n}", "animate": {"count": {"from": 10}}}));
        assert_eq!(
            text(&down, false).as_deref(),
            Some("T-10"),
            "a countdown measures its start"
        );
        let plain = layer(json!({"type": "text", "text": "{n}", "animate": {"scale": [1, 2]}}));
        assert_eq!(text(&plain, false), None);
    }
}
