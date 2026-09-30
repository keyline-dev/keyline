//! Tabulating the runs: `results.tsv` (one row per run) and the medians
//! table for the README.

use std::fmt::Write as _;
use std::path::PathBuf;

use serde_json::Value;

use crate::bench_root;
use crate::prompts::{Arm, Task};

/// One saved run: its `summary.json` and, once judged, its `judge.json`.
pub struct Run {
    pub dir: PathBuf,
    pub summary: Value,
    pub judge: Option<Value>,
}

impl Run {
    fn s(&self, k: &str) -> &Value {
        &self.summary[k]
    }

    fn f(&self, k: &str) -> f64 {
        self.summary[k].as_f64().unwrap_or_default()
    }

    /// Correct by the judge; a run with wrong or missing files never is.
    pub fn correct(&self) -> Option<bool> {
        if self.summary["filesOk"] != true {
            return Some(false);
        }
        self.judge.as_ref().map(|j| j["correct"] == true)
    }

    /// Counted runs: not pilots, on the frozen prompt version.
    pub fn counted(&self) -> bool {
        self.s("promptVersion") == crate::prompts::VERSION
            && !self
                .s("label")
                .as_str()
                .unwrap_or_default()
                .starts_with("pilot")
    }
}

/// Every saved run, in task, arm and label order.
pub fn runs() -> Vec<Run> {
    let mut out = Vec::new();
    for task in ["reference-ad", "speaker-card"] {
        let Ok(dirs) = std::fs::read_dir(bench_root().join(task)) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = dirs.filter_map(Result::ok).map(|e| e.path()).collect();
        dirs.sort();
        for dir in dirs {
            let read = |f: &str| {
                std::fs::read_to_string(dir.join(f))
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            };
            if let Some(summary) = read("summary.json") {
                let judge = read("judge.json");
                out.push(Run {
                    dir,
                    summary,
                    judge,
                });
            }
        }
    }
    out
}

/// Rewrites `results.tsv` from the saved runs.
pub fn write_tsv(runs: &[Run]) {
    let mut tsv = String::from(
        "task\tarm\tlabel\tcommit\tclaude_version\tmodel\tcorrect\ttotal_tokens\tinput\tcache_write\t\
cache_read\toutput\tthinking\tfixed_overhead\tpeak_context\tcost\tturns\tcalls\timages_seen\t\
js_measure\tsecs\tlikeness_k\tlikeness_c\tdefects_judge\tcold_cost\toutcome\tprompt\n",
    );
    for r in runs {
        let j = r.judge.as_ref();
        let judged = |k: &str| {
            j.and_then(|j| j[k].as_f64())
                .map_or(String::new(), |v| format!("{v:.1}"))
        };
        let cols: Vec<String> = vec![
            r.s("task").as_str().unwrap_or_default().to_owned(),
            r.s("arm").as_str().unwrap_or_default().to_owned(),
            r.s("label").as_str().unwrap_or_default().to_owned(),
            r.s("commit").as_str().unwrap_or_default().to_owned(),
            r.s("claudeVersion").as_str().unwrap_or_default().to_owned(),
            r.s("model").as_str().unwrap_or_default().to_owned(),
            r.correct()
                .map_or(String::new(), |c| u8::from(c).to_string()),
            r.s("totalTokens").to_string(),
            r.s("input").to_string(),
            r.s("cacheWrite").to_string(),
            r.s("cacheRead").to_string(),
            r.s("output").to_string(),
            r.s("thinking").to_string(),
            r.s("fixedOverhead").to_string(),
            r.s("peakContext").to_string(),
            format!("{:.3}", r.f("costUsd")),
            r.s("turns").to_string(),
            r.s("calls").to_string(),
            r.s("imagesSeen").to_string(),
            r.s("jsMeasure").to_string(),
            format!("{:.0}", r.f("secs")),
            judged("likenessKeyline"),
            judged("likenessChrome"),
            j.map_or(String::new(), |j| j["defects"].to_string()),
            format!("{:.3}", r.f("coldCostUsd")),
            r.s("outcome").as_str().unwrap_or_default().to_owned(),
            r.s("promptVersion").as_str().unwrap_or_default().to_owned(),
        ];
        tsv.push_str(&cols.join("\t"));
        tsv.push('\n');
    }
    std::fs::write(bench_root().join("results.tsv"), tsv).unwrap();
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    match v.len() {
        0 => f64::NAN,
        n if n % 2 == 1 => v[n / 2],
        n => f64::midpoint(v[n / 2 - 1], v[n / 2]),
    }
}

/// `median (min–max)`, with `fmt` for each number.
fn stat(v: &[f64], fmt: fn(f64) -> String) -> String {
    let mut v = v.to_vec();
    if v.is_empty() {
        return "–".into();
    }
    let m = median(&mut v);
    format!("{} ({}–{})", fmt(m), fmt(v[0]), fmt(v[v.len() - 1]))
}

fn k(v: f64) -> String {
    format!("{:.0}k", v / 1000.0)
}

fn usd(v: f64) -> String {
    format!("${v:.2}")
}

fn int(v: f64) -> String {
    format!("{v:.0}")
}

/// The medians table per task (counted runs only), as Markdown.
pub fn medians(runs: &[Run], counted_only: bool) -> String {
    let mut md = String::new();
    for task in [Task::ReferenceAd, Task::SpeakerCard] {
        let _ = writeln!(md, "\n### {}\n", task.name());
        md.push_str("| Arm | Runs | Correct | Total tokens | Cost | Cold cost | Turns | Time (s) | Images seen | Fixed overhead | Peak context | JS measure |\n");
        md.push_str("|---|---|---|---|---|---|---|---|---|---|---|---|\n");
        let mut tokens_by_arm = Vec::new();
        for arm in [Arm::Keyline, Arm::BrowserCli, Arm::BrowserMcp] {
            let rs: Vec<&Run> = runs
                .iter()
                .filter(|r| r.s("task") == task.name() && r.s("arm") == arm.name())
                .filter(|r| !counted_only || r.counted())
                .collect();
            if rs.is_empty() {
                continue;
            }
            let col = |key: &str| rs.iter().map(|r| r.f(key)).collect::<Vec<f64>>();
            let correct = rs.iter().filter(|r| r.correct() == Some(true)).count();
            let judged = rs.iter().filter(|r| r.correct().is_some()).count();
            let js = rs.iter().filter(|r| r.s("jsMeasure") == true).count();
            let _ = writeln!(
                md,
                "| {} | {} | {correct}/{judged} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                arm.name(),
                rs.len(),
                stat(&col("totalTokens"), k),
                stat(&col("costUsd"), usd),
                stat(&col("coldCostUsd"), usd),
                stat(&col("turns"), int),
                stat(&col("secs"), int),
                stat(&col("imagesSeen"), int),
                stat(&col("fixedOverhead"), k),
                stat(&col("peakContext"), k),
                if arm == Arm::Keyline {
                    "–".into()
                } else {
                    format!("{js}/{}", rs.len())
                },
            );
            let ok = |key: &str| {
                rs.iter()
                    .filter(|r| r.correct() == Some(true))
                    .map(|r| r.f(key))
                    .collect::<Vec<f64>>()
            };
            tokens_by_arm.push((
                arm,
                col("totalTokens"),
                col("costUsd"),
                ok("totalTokens"),
                ok("costUsd"),
            ));
        }
        md.push_str("\nCorrect runs only:\n\n| Arm | Total tokens | Cost |\n|---|---|---|\n");
        for (arm, _, _, tok, cost) in &tokens_by_arm {
            let _ = writeln!(
                md,
                "| {} | {} | {} |",
                arm.name(),
                stat(tok, k),
                stat(cost, usd)
            );
        }
        if let Some((_, kt, kc, ..)) = tokens_by_arm.iter().find(|a| a.0 == Arm::Keyline) {
            md.push_str("\nBrowser ÷ keyline (ratio of medians; range from the extremes):\n\n| Arm | Total tokens | Cost |\n|---|---|---|\n");
            for (arm, bt, bc, ..) in tokens_by_arm.iter().filter(|a| a.0 != Arm::Keyline) {
                let _ = writeln!(
                    md,
                    "| {} | {} | {} |",
                    arm.name(),
                    ratio(bt, kt),
                    ratio(bc, kc)
                );
            }
        }
    }
    md
}

/// `b ÷ a` as a ratio of medians, with the lowest and highest it could be.
fn ratio(b: &[f64], a: &[f64]) -> String {
    let (mut b, mut a) = (b.to_vec(), a.to_vec());
    let r = median(&mut b) / median(&mut a);
    let lo = b[0] / a[a.len() - 1];
    let hi = b[b.len() - 1] / a[0];
    format!("{r:.1}× ({lo:.1}–{hi:.1}×)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medians_and_ratios() {
        assert!((median(&mut [3.0, 1.0, 2.0]) - 2.0).abs() < f64::EPSILON);
        assert!((median(&mut [4.0, 1.0, 2.0, 3.0]) - 2.5).abs() < f64::EPSILON);
        assert_eq!(
            ratio(&[20.0, 40.0, 30.0], &[10.0, 20.0, 15.0]),
            "2.0× (1.0–4.0×)"
        );
        assert_eq!(stat(&[2000.0, 1000.0, 3000.0], k), "2k (1k–3k)");
    }
}
