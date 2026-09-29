//! End-to-end test with a real model: Claude Code, running headless on your
//! Claude subscription, drives the MCP server to build the reference ad.
//! Uses subscription quota, so it only runs on request:
//!
//! ```sh
//! cargo test --test llm_e2e -- --ignored --nocapture
//! ```
//!
//! `CLAUDE_BIN` points at the `claude` CLI (default: `claude` on PATH);
//! `KEYLINE_MCP_TEST_MODEL` picks the model (default: Claude Code's default).
//!
//! Benchmarking: with `KEYLINE_MCP_BENCH=<label>`, the run is kept in
//! `bench/reference-ad/<label>/` (prompt, event log, final layout, renders,
//! summary) and appended to `bench/reference-ad/results.tsv`, so runs on
//! different commits can be compared.

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::process::Stdio;

use common::{CHECK_SVG, MAIL_SVG, Mcp, ad_sizes, b64, photo_png};
use serde_json::{Value, json};

const TOKEN_TARGET: usize = 2000;
const MAX_TURNS: &str = "25";

const SYSTEM: &str =
    "You compose images with the scene MCP tools. Work autonomously; don't ask questions.";

const TASK: &str = "Build a vote-by-mail flyer in scene {scene} and render it.

Assets already in the scene: photo (1600×900 landscape photo), mail (white envelope icon, 56×44 SVG), \
check (red check-circle icon, 40×40 SVG).
Sizes: portrait 1080×1350 (master), wide 1200×1000 at scale 0.85, sky 300×600 at scale 0.28.

Content, top to bottom:
1. Headline \"Proven RESULTS for WILLOWMERE Families\", navy #1B2A5C, with RESULTS and WILLOWMERE in red #D0202E.
2. A full-width photo band.
3. Three candidate columns, evenly spaced: Dana Levi (Mayor), Omar Haddad (Council), Ruth Cohen (Council).
4. A full-width red call-to-action bar with the mail icon and \"VOTE BY MAIL\" in white.
5. Three steps, each with the check icon: \"Request your ballot by October 20\", \"Fill it out at home\", \
\"Mail it back by November 3\".
6. Footer: \"Paid for by Willowmere Forward · willowmereforward.org\".

Every size must look right: use constraints so bands stay full width, columns stay evenly \
spaced, and the photo crops instead of distorting. Fix every layout defect, then render.";

#[tokio::test]
#[ignore = "runs Claude Code on your subscription; run with --ignored"]
async fn claude_builds_the_reference_ad() {
    let mcp = Mcp::start("llm").await;

    // Uploads aren't what's being tested, so the harness does them up front.
    // Claude's own server process shares the same data directory.
    let created = mcp
        .ok(
            "scene_create",
            json!({"width": 1080, "height": 1350, "sizes": ad_sizes()}),
        )
        .await;
    let scene = created.split(' ').next().unwrap().to_owned();
    for (id, bytes) in [
        ("photo", photo_png()),
        ("mail", MAIL_SVG.as_bytes().to_vec()),
        ("check", CHECK_SVG.as_bytes().to_vec()),
    ] {
        mcp.ok(
            "asset_add",
            json!({"sceneId": scene, "id": id, "base64": b64(&bytes)}),
        )
        .await;
    }

    let mcp_config = json!({"mcpServers": {"scene": {
        "command": env!("CARGO_BIN_EXE_keyline-mcp"),
        "args": ["--data", mcp.data]
    }}});
    let mut cmd = tokio::process::Command::new(
        std::env::var("CLAUDE_BIN").unwrap_or_else(|_| "claude".into()),
    );
    cmd.arg("-p")
        .arg(TASK.replace("{scene}", &scene))
        .args(["--output-format", "stream-json", "--verbose"])
        .args([
            "--mcp-config",
            &mcp_config.to_string(),
            "--strict-mcp-config",
        ])
        .args(["--tools", "", "--allowedTools", "mcp__scene__*"])
        .args(["--system-prompt", SYSTEM, "--max-turns", MAX_TURNS])
        .args(["--setting-sources", "", "--no-session-persistence"])
        .stdin(Stdio::null());
    if let Ok(model) = std::env::var("KEYLINE_MCP_TEST_MODEL") {
        cmd.args(["--model", &model]);
    }
    let out = cmd
        .output()
        .await
        .expect("run claude; set CLAUDE_BIN if it isn't on PATH");
    let events: Vec<Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let result = events
        .iter()
        .find(|e| e["type"] == "result")
        .unwrap_or_else(|| {
            panic!(
                "no result from claude: {}",
                String::from_utf8_lossy(&out.stderr)
            )
        });
    assert_ne!(result["is_error"], true, "claude failed: {result}");

    // Tool traffic: what the model wrote into tool calls plus what came back.
    let blocks = events
        .iter()
        .filter(|e| e["type"] == "assistant" || e["type"] == "user")
        .flat_map(|e| {
            e["message"]["content"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        });
    let (mut calls, mut traffic, mut images) = (0, 0, 0);
    for b in blocks {
        match b["type"].as_str() {
            Some("tool_use") => {
                calls += 1;
                traffic += b["input"].to_string().len();
            }
            // Images are billed by pixel area, not by their base64 length,
            // so count them apart from text.
            Some("tool_result") => match &b["content"] {
                Value::Array(parts) => {
                    for p in parts {
                        if p["type"] == "image" {
                            images += 1;
                        } else {
                            traffic += p["text"].as_str().map_or(0, str::len);
                        }
                    }
                }
                Value::String(s) => traffic += s.len(),
                _ => {}
            },
            _ => {}
        }
    }
    let u = &result["usage"];
    println!(
        "{calls} tool calls, {} turns, ~{} tokens of text tool traffic (target ~{TOKEN_TARGET}), {images} preview images",
        result["num_turns"],
        traffic / 4
    );
    let log = mcp.data.join("claude-events.jsonl");
    std::fs::write(&log, &out.stdout).expect("save event log");
    println!("event log: {}", log.display());
    println!(
        "usage: {} input + {} cache-write + {} cache-read input, {} output tokens; ${} API-equivalent",
        u["input_tokens"],
        u["cache_creation_input_tokens"],
        u["cache_read_input_tokens"],
        u["output_tokens"],
        result["total_cost_usd"]
    );

    // Defects (`!`) must be fixed; advisories (`warn`) are the model's call.
    let problems = mcp.ok("scene_describe", json!({"sceneId": scene})).await;
    println!("problems: {problems}");
    let described = mcp
        .ok("scene_describe", json!({"sceneId": scene, "full": true}))
        .await;
    println!("{described}");
    let layers = described.lines().filter(|l| l.starts_with(' ')).count() / 3;
    let rendered = mcp.ok("render", json!({"sceneId": scene})).await;
    println!("renders (kept for review):\n{rendered}");

    if let Ok(label) = std::env::var("KEYLINE_MCP_BENCH") {
        let run = Run {
            label: &label,
            task: &TASK.replace("{scene}", &scene),
            result,
            stdout: &out.stdout,
            problems: &problems,
            described: &described,
            rendered: &rendered,
            calls,
            traffic,
            images,
        };
        let dir = run.save();
        println!("benchmark saved to {}", dir.display());
    }
    // Checked after saving, so failed runs are kept for comparison too.
    assert!(!problems.contains(" !"), "defects remain:\n{problems}");
    assert!(layers >= 10, "only {layers} layers per size:\n{described}");
}

/// One benchmark run, saved for comparison with later ones.
struct Run<'a> {
    label: &'a str,
    task: &'a str,
    result: &'a Value,
    stdout: &'a [u8],
    problems: &'a str,
    described: &'a str,
    rendered: &'a str,
    calls: usize,
    traffic: usize,
    images: usize,
}

impl Run<'_> {
    fn save(&self) -> std::path::PathBuf {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/reference-ad");
        let dir = root.join(self.label);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
                .unwrap_or_default()
        };
        let commit = git(&["rev-parse", "--short", "HEAD"]);
        let dirty = !git(&["status", "--porcelain", "--", "src"]).is_empty();
        let commit = if dirty {
            format!("{commit}+dirty")
        } else {
            commit
        };
        std::fs::write(
            dir.join("prompt.md"),
            format!("# System\n\n{SYSTEM}\n\n# Task\n\n{}\n", self.task),
        )
        .unwrap();
        std::fs::write(dir.join("events.jsonl"), self.stdout).unwrap();
        std::fs::write(
            dir.join("layout.txt"),
            format!("{}\n\n{}", self.problems, self.described),
        )
        .unwrap();
        for line in self.rendered.lines().filter(|l| !l.starts_with(' ')) {
            if let Some((size, path)) = line.split_once(' ') {
                std::fs::copy(path, dir.join(format!("{size}.png"))).unwrap();
            }
        }

        let r = self.result;
        let u = &r["usage"];
        let model = r["modelUsage"]
            .as_object()
            .and_then(|m| m.keys().find(|k| !k.contains("haiku")).cloned())
            .unwrap_or_default();
        let smallest = smallest_text(self.described);
        let defects = self.problems.matches(" !").count();
        let summary = json!({
            "label": self.label, "commit": commit, "model": model,
            "costUsd": r["total_cost_usd"], "turns": r["num_turns"], "durationMs": r["duration_ms"],
            "toolCalls": self.calls, "toolTrafficChars": self.traffic, "previewImages": self.images,
            "usage": u, "smallestTextPx": smallest, "defects": defects,
        });
        std::fs::write(
            dir.join("summary.json"),
            serde_json::to_string_pretty(&summary).unwrap(),
        )
        .unwrap();

        let tsv = root.join("results.tsv");
        let mut rows = std::fs::read_to_string(&tsv).unwrap_or_else(|_| {
            "label\tcommit\tmodel\tcost\tturns\tsecs\tcalls\ttool_chars\timages\toutput\tthinking\tcache_write\tcache_read\tsmallest_text\tdefects\n".into()
        });
        let px: Vec<String> = smallest.iter().map(|(s, v)| format!("{s} {v}")).collect();
        rows.push_str(&format!(
            "{}\t{commit}\t{model}\t{:.3}\t{}\t{:.0}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{defects}\n",
            self.label,
            r["total_cost_usd"].as_f64().unwrap_or_default(),
            r["num_turns"],
            r["duration_ms"].as_f64().unwrap_or_default() / 1000.0,
            self.calls,
            self.traffic,
            self.images,
            u["output_tokens"],
            u["output_tokens_details"]["thinking_tokens"],
            u["cache_creation_input_tokens"],
            u["cache_read_input_tokens"],
            px.join(", "),
        ));
        std::fs::write(&tsv, rows).unwrap();
        dir
    }
}

/// The smallest font size per size, from `scene_describe full`: size header
/// lines, then indented layer lines carrying `NNpx` for text.
fn smallest_text(described: &str) -> Vec<(String, f32)> {
    let mut out: Vec<(String, f32)> = Vec::new();
    for line in described.lines() {
        if !line.starts_with(' ') {
            if let Some(size) = line.split(' ').next().filter(|s| *s != "assets") {
                out.push((size.to_owned(), f32::INFINITY));
            }
            continue;
        }
        let px = line
            .split(' ')
            .filter_map(|w| w.strip_suffix("px")?.parse::<f32>().ok())
            .next();
        if let (Some(px), Some((_, min))) = (px, out.last_mut()) {
            *min = min.min(px);
        }
    }
    out.retain(|(_, v)| v.is_finite());
    out
}
