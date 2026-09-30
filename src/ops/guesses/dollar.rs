//! Tokens as `"$name"`, the way many template tools write them: a whole
//! value naming a known token reads as `"{{name}}"`; inside text, `$name`
//! may be a price or a currency, so it's only pointed out.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::reuse::tokens::is_name;
use crate::scene::{Kind, Scene};

/// Rewrites every string in `v` that is exactly `"$name"`, for a token
/// that exists, as `"{{name}}"`.
pub(crate) fn whole(v: &mut Value, tokens: &BTreeMap<String, Value>) {
    match v {
        Value::String(s) => {
            if let Some(name) = s.strip_prefix('$')
                && tokens.contains_key(name)
            {
                *s = crate::reuse::tokens::braced(name);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| whole(x, tokens)),
        Value::Object(o) => o.values_mut().for_each(|x| whole(x, tokens)),
        _ => {}
    }
}

/// One line per text in `scene` that writes a token as `$name` outside
/// `{{ }}`: `hint: did you mean {{price}}? (text1 says $price)`.
pub(crate) fn hints(scene: &Scene) -> Vec<String> {
    let mut out = Vec::new();
    scene.walk(&mut |l| {
        if let Kind::Text { text, .. } = &l.kind
            && let Some(name) = named(text, &scene.tokens)
        {
            out.push(format!(
                "hint: did you mean {}? ({} says ${name})",
                crate::reuse::tokens::braced(name),
                l.id
            ));
        }
    });
    out
}

/// The first `$name` in `text` that names a token.
fn named<'a>(text: &'a str, tokens: &BTreeMap<String, Value>) -> Option<&'a str> {
    text.match_indices('$').find_map(|(i, _)| {
        let rest = &text[i + 1..];
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || "_.-".contains(c)))
            .unwrap_or(rest.len());
        // A name's trailing `.` or `-` is punctuation: "Pay $price."
        let name = rest[..end].trim_end_matches(['.', '-']);
        (is_name(name) && tokens.contains_key(name)).then_some(name)
    })
}

#[cfg(test)]
mod tests {
    use super::{named, whole};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn tokens() -> BTreeMap<String, serde_json::Value> {
        serde_json::from_value(json!({"brand": "#D0202E", "price": 29})).unwrap()
    }

    #[test]
    fn a_whole_dollar_value_naming_a_token_reads_as_a_placeholder() {
        let mut v = json!({"fill": "$brand", "stroke": {"color": "$brand"}, "text": "$price", "a": "$other", "b": "pay $brand"});
        whole(&mut v, &tokens());
        assert_eq!(
            v,
            json!({"fill": "{{brand}}", "stroke": {"color": "{{brand}}"}, "text": "{{price}}", "a": "$other", "b": "pay $brand"})
        );
    }

    #[test]
    fn dollar_names_in_text_are_found_only_for_tokens() {
        let t = tokens();
        assert_eq!(named("Only $price.", &t), Some("price"));
        assert_eq!(named("Only $29 or $USD", &t), None);
        assert_eq!(named("{{price}} flat", &t), None);
    }
}
