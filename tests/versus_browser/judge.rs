//! The blind checklist judge: a separate `claude -p` call that sees only a
//! run's PNGs, under shuffled neutral names, and fills in a fixed checklist.

use std::path::Path;
use std::process::Stdio;

use serde_json::{Value, json};

use crate::prompts::Task;
use crate::{bench_root, claude_bin, random_hex};

const SYSTEM: &str = "You check images against a checklist. Answer with JSON only.";

/// The judge's model; fixed for every run, whatever the arms ran on.
pub fn model() -> String {
    std::env::var("KEYLINE_BENCH_JUDGE_MODEL").unwrap_or_else(|_| "claude-opus-5".into())
}

/// Task-specific checklist lines and their fields in the answer's shape.
fn checks(task: Task) -> (&'static str, &'static str) {
    match task {
        Task::ReferenceAd => (
            "- `photoDistorted`: whether the photo looks stretched or squashed instead of cropped.\n\
- `bandsFullWidth`: whether the photo band and the red call-to-action bar both span the full width.\n\
- `columnsEven`: whether the three candidate columns are evenly spaced.",
            "\"photoDistorted\": false, \"bandsFullWidth\": true, \"columnsEven\": true",
        ),
        Task::SpeakerCard => (
            "- `portraitCircle`: whether the portrait is cropped to a true circle (not an ellipse, \
a square or a stretched photo).",
            "\"portraitCircle\": true",
        ),
    }
}

/// Judges one run whose three PNGs are in `run`; returns `judge.json`.
pub async fn judge(run: &Path, task: Task) -> Value {
    let dir = std::env::temp_dir().join(format!("judge-{}", random_hex()));
    std::fs::create_dir_all(&dir).unwrap();
    // Neutral names in a random order, so neither name nor order says which
    // arm made them; the size is given, since it isn't a secret.
    let mut names: Vec<(String, &str, u32, u32)> = task
        .sizes()
        .iter()
        .map(|&(size, w, h, _)| (format!("{}.png", &random_hex()[..6]), size, w, h))
        .collect();
    names.sort();
    let mut files = String::new();
    for (name, size, w, h) in &names {
        std::fs::copy(run.join(format!("{size}.png")), dir.join(name)).unwrap();
        files.push_str(&format!("- `{name}`: {w}×{h}\n"));
    }
    let (lines, extra) = checks(task);
    let template = std::fs::read_to_string(bench_root().join("judge-prompt.md")).unwrap();
    let prompt = template
        .replace("{files}", files.trim_end())
        .replace("{content}", task.content())
        .replace("{checks}", lines)
        .replace("{extra}", extra);

    let out = tokio::process::Command::new(claude_bin())
        .arg("-p")
        .arg(&prompt)
        .args(["--output-format", "json", "--model", &model()])
        .args(["--tools", "Read", "--allowedTools", "Read"])
        .args(["--system-prompt", SYSTEM, "--max-turns", "15"])
        .args([
            "--mcp-config",
            r#"{"mcpServers":{}}"#,
            "--strict-mcp-config",
        ])
        .args(["--setting-sources", "", "--no-session-persistence"])
        .current_dir(&dir)
        .stdin(Stdio::null())
        .output()
        .await
        .expect("run claude for the judge");
    let _ = std::fs::remove_dir_all(&dir);
    let result: Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|_| panic!("judge: {}", String::from_utf8_lossy(&out.stderr)));
    let text = result["result"].as_str().unwrap_or_default();
    let answer: Value = text
        .find('{')
        .zip(text.rfind('}'))
        .and_then(|(a, b)| serde_json::from_str(&text[a..=b]).ok())
        .unwrap_or_else(|| panic!("judge gave no JSON: {text}"));

    let mut sizes = serde_json::Map::new();
    for (name, size, ..) in &names {
        let c = answer[name].clone();
        assert!(c.is_object(), "judge skipped {name}: {text}");
        sizes.insert((*size).to_owned(), c);
    }
    let correct = sizes.values().all(size_correct);
    let defects: usize = sizes.values().map(defects).sum();
    json!({
        "model": model(), "costUsd": result["total_cost_usd"],
        "correct": correct, "defects": defects, "sizes": sizes,
    })
}

/// Correct at one size: every item present, none cut off or overflowing.
pub fn size_correct(c: &Value) -> bool {
    let items = c["items"].as_object();
    items.is_some_and(|i| i.len() == 6 && i.values().all(|v| v["present"] == true))
        && c["cutOrOverflowing"].as_array().is_some_and(Vec::is_empty)
}

/// Every problem the judge marked at one size.
pub fn defects(c: &Value) -> usize {
    let count = |k: &str| c[k].as_array().map_or(0, Vec::len);
    let items = c["items"].as_object().map_or(0, |i| {
        i.values()
            .map(|v| usize::from(v["present"] != true) + usize::from(v["readable"] != true))
            .sum()
    });
    let flags = [
        ("smallestTextLegible", true),
        ("photoDistorted", false),
        ("bandsFullWidth", true),
        ("columnsEven", true),
        ("portraitCircle", true),
    ]
    .iter()
    .filter(|(k, good)| c[*k].is_boolean() && c[*k] != *good)
    .count();
    items + count("cutOrOverflowing") + count("overlapping") + flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdicts() {
        let item = json!({"present": true, "readable": true});
        let items: serde_json::Map<String, Value> =
            (1..=6).map(|i| (i.to_string(), item.clone())).collect();
        let mut c = json!({"items": items, "cutOrOverflowing": [], "overlapping": [2],
            "smallestTextLegible": true, "photoDistorted": false});
        assert!(size_correct(&c));
        assert_eq!(defects(&c), 1);
        c["cutOrOverflowing"] = json!([5]);
        c["items"]["3"]["present"] = json!(false);
        assert!(!size_correct(&c));
        assert_eq!(defects(&c), 3);
    }
}
