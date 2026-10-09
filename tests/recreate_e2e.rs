//! Benchmark with a real model: Claude Code rebuilds an existing design from
//! its image through the MCP server, and the result is scored against it.
//! Uses subscription quota and local files, so it only runs on request:
//!
//! ```sh
//! KEYLINE_MCP_BENCH=<label> cargo test --test recreate_e2e -- --ignored --nocapture
//! ```
//!
//! The design lives in `bench/recreate/local/` (gitignored: references are
//! often real people's material) or `$KEYLINE_MCP_RECREATE_DIR`:
//! `reference.png|jpg`, `assets/` (uploaded before the run, e.g. the
//! original photo) and an optional `task.md` appended to the prompt. Each
//! run is kept in `<dir>/runs/<label>/` and appended to `<dir>/results.tsv`.
//! `CLAUDE_BIN` and `KEYLINE_MCP_TEST_MODEL` work as in `llm_e2e`.

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Stdio;

use common::{Mcp, b64};
use serde_json::{Value, json};
use skia_safe::{Data, Image, surfaces};

const SYSTEM: &str =
    "You compose images with the scene MCP tools. Work autonomously; don't ask questions.";

#[tokio::test]
#[ignore = "runs Claude Code on your subscription with local files; run with --ignored"]
async fn claude_recreates_a_reference_design() {
    let dir = std::env::var_os("KEYLINE_MCP_RECREATE_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/recreate/local"),
        PathBuf::from,
    );
    let Some(reference) = ["reference.png", "reference.jpg", "reference.jpeg"]
        .iter()
        .map(|f| dir.join(f))
        .find(|p| p.exists())
    else {
        eprintln!("skipped: no reference image in {}", dir.display());
        return;
    };
    let label = std::env::var("KEYLINE_MCP_BENCH").unwrap_or_else(|_| "run".into());
    let run = dir.join("runs").join(&label);
    let _ = std::fs::remove_dir_all(&run);
    std::fs::create_dir_all(&run).unwrap();
    let ref_bytes = std::fs::read(&reference).unwrap();
    // Kept with the run, so it can be re-scored against what it was shown.
    std::fs::write(run.join(reference.file_name().unwrap()), &ref_bytes).unwrap();
    let ref_img = decode(&ref_bytes);
    let (w, h) = (ref_img.width(), ref_img.height());

    let mcp = Mcp::start("recreate").await;
    let created = mcp
        .ok(
            "scene_create",
            json!({"width": w, "height": h, "sizes": [{"id": "main", "width": w, "height": h}]}),
        )
        .await;
    let scene = created.split(' ').next().unwrap().to_owned();
    // Uploads aren't what's being tested; the model is told what they are.
    let mut uploaded = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(dir.join("assets"))
        .map(|d| d.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    files.sort();
    for path in files {
        let id = path.file_stem().unwrap().to_string_lossy().into_owned();
        let bytes = std::fs::read(&path).unwrap();
        let reply = mcp
            .ok(
                "asset_add",
                json!({"sceneId": scene, "id": id, "base64": b64(&bytes)}),
            )
            .await;
        uploaded.push(format!("`{reply}` (file {})", path.display()));
    }
    let extra = std::fs::read_to_string(dir.join("task.md")).unwrap_or_default();
    let task = format!(
        "Recreate the design in {} (read it first) as closely as you can, in scene {scene} \
({w}×{h}, one size 'main'). Assets already uploaded (asset_add replies; view a file if you need to): {}. \
Match layout, colors, fonts (any Google Font family works by name), text and icons. \
Fix any defects, then render.\n\n{extra}",
        reference.display(),
        if uploaded.is_empty() {
            "none".into()
        } else {
            uploaded.join(", ")
        },
    );

    let mcp_config = json!({"mcpServers": {"scene": {
        "command": env!("CARGO_BIN_EXE_keyline-mcp"),
        "args": ["--data", mcp.data, "--folder", mcp.workspace()]
    }}});
    let mut cmd = tokio::process::Command::new(
        std::env::var("CLAUDE_BIN").unwrap_or_else(|_| "claude".into()),
    );
    cmd.arg("-p")
        .arg(&task)
        .args(["--output-format", "stream-json", "--verbose"])
        .args([
            "--mcp-config",
            &mcp_config.to_string(),
            "--strict-mcp-config",
        ])
        // Read, so the model can look at the reference and the assets.
        .args(["--tools", "Read", "--allowedTools", "mcp__scene__*", "Read"])
        .args(["--system-prompt", SYSTEM, "--max-turns", "40"])
        .args(["--setting-sources", "", "--no-session-persistence"])
        .stdin(Stdio::null());
    if let Ok(model) = std::env::var("KEYLINE_MCP_TEST_MODEL") {
        cmd.args(["--model", &model]);
    }
    let out = cmd
        .output()
        .await
        .expect("run claude; set CLAUDE_BIN if it isn't on PATH");
    std::fs::write(run.join("events.jsonl"), &out.stdout).unwrap();
    std::fs::write(
        run.join("prompt.md"),
        format!("# System\n\n{SYSTEM}\n\n# Task\n\n{task}\n"),
    )
    .unwrap();
    let events: Vec<Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let result = events
        .iter()
        .find(|e| e["type"] == "result")
        .unwrap_or_else(|| panic!("no result: {}", String::from_utf8_lossy(&out.stderr)));

    let calls: Vec<String> = events
        .iter()
        .filter(|e| e["type"] == "assistant")
        .flat_map(|e| {
            e["message"]["content"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter(|b| b["type"] == "tool_use")
        .map(|b| {
            b["name"]
                .as_str()
                .unwrap_or_default()
                .replace("mcp__scene__", "")
        })
        .collect();

    // The final render, scored against the reference.
    let rendered = mcp.ok("render", json!({"sceneId": scene})).await;
    let path = common::file_of(rendered.lines().next().unwrap()).unwrap().1;
    let png = std::fs::read(path).unwrap();
    std::fs::write(run.join("final.png"), &png).unwrap();
    let score = likeness(&ref_img, &decode(&png));
    let described = mcp
        .ok("scene_describe", json!({"sceneId": scene, "full": true}))
        .await;
    std::fs::write(run.join("layout.txt"), &described).unwrap();
    mcp.stop().await;

    let u = &result["usage"];
    let row = format!(
        "{label}\t{}\t{:.3}\t{}\t{:.0}\t{}\t{}\t{}\t{}\t{}\t{score:.2}\t{}\n",
        commit(),
        result["total_cost_usd"].as_f64().unwrap_or_default(),
        result["num_turns"],
        result["duration_ms"].as_f64().unwrap_or_default() / 1000.0,
        u["output_tokens"],
        u["output_tokens_details"]["thinking_tokens"],
        u["cache_creation_input_tokens"],
        u["cache_read_input_tokens"],
        calls.len(),
        calls.join(" "),
    );
    let tsv = dir.join("results.tsv");
    let mut rows = std::fs::read_to_string(&tsv).unwrap_or_else(|_| {
        "label\tcommit\tcost\tturns\tsecs\toutput\tthinking\tcache_write\tcache_read\tcalls\tscore\ttools\n".into()
    });
    rows.push_str(&row);
    std::fs::write(&tsv, rows).unwrap();
    println!("{row}saved to {}", run.display());
}

fn decode(bytes: &[u8]) -> Image {
    Image::from_encoded(Data::new_copy(bytes)).expect("decodable image")
}

/// How far apart two images look, 0–255, lower is closer. Both are
/// averaged into 8 px blocks at the reference's size, then compared in
/// 64 px tiles, each tile at its best offset within ±64 px: a region that
/// sits a little off (a photo cropped 50 px higher) still matches, while
/// wrong colors, missing text or a misplaced block still count, since a
/// tile moves as one piece.
fn likeness(reference: &Image, img: &Image) -> f32 {
    const BLOCK: i32 = 8;
    const TILE: i32 = 8; // blocks per tile side: 64 px
    const SHIFT: i32 = 8; // blocks either way: 64 px
    let (w, h) = (reference.width(), reference.height());
    let (bw, bh) = (w / BLOCK, h / BLOCK);
    let grid = |i: &Image| {
        let mut s = surfaces::raster_n32_premul((w, h)).unwrap();
        s.canvas().draw_image_rect(
            i,
            None,
            skia_safe::Rect::from_wh(w as f32, h as f32),
            &skia_safe::Paint::default(),
        );
        let snap = s.image_snapshot();
        let px = snap.peek_pixels().unwrap();
        let mut g = vec![[0.0_f32; 3]; (bw * bh) as usize];
        for by in 0..bh {
            for bx in 0..bw {
                let mut sum = [0.0_f32; 3];
                for y in by * BLOCK..(by + 1) * BLOCK {
                    for x in bx * BLOCK..(bx + 1) * BLOCK {
                        let c = px.get_color((x, y));
                        for (s, v) in sum.iter_mut().zip([c.r(), c.g(), c.b()]) {
                            *s += f32::from(v);
                        }
                    }
                }
                g[(by * bw + bx) as usize] = sum.map(|s| s / (BLOCK * BLOCK) as f32);
            }
        }
        g
    };
    let (a, b) = (grid(reference), grid(img));
    let diff =
        |p: [f32; 3], q: [f32; 3]| p.iter().zip(q).map(|(x, y)| (x - y).abs()).sum::<f32>() / 3.0;
    let (mut total, mut blocks) = (0.0, 0);
    for ty in (0..bh).step_by(TILE as usize) {
        for tx in (0..bw).step_by(TILE as usize) {
            let (tw, th) = (TILE.min(bw - tx), TILE.min(bh - ty));
            let tile_cost = |dx: i32, dy: i32| {
                let mut sum = 0.0;
                for y in ty..ty + th {
                    for x in tx..tx + tw {
                        let (sx, sy) = ((x + dx).clamp(0, bw - 1), (y + dy).clamp(0, bh - 1));
                        sum += diff(a[(y * bw + x) as usize], b[(sy * bw + sx) as usize]);
                    }
                }
                sum
            };
            let best = (-SHIFT..=SHIFT)
                .flat_map(|dy| (-SHIFT..=SHIFT).map(move |dx| (dx, dy)))
                .map(|(dx, dy)| tile_cost(dx, dy))
                .fold(f32::INFINITY, f32::min);
            total += best;
            blocks += tw * th;
        }
    }
    total / blocks as f32
}

/// Re-scores every run in `results.tsv` from its saved `final.png`, against
/// the reference kept with it (or the current one), after a scoring change.
#[test]
#[ignore = "rewrites the local bench results; run with --ignored"]
fn rescore_runs() {
    let dir = std::env::var_os("KEYLINE_MCP_RECREATE_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/recreate/local"),
        PathBuf::from,
    );
    let tsv = dir.join("results.tsv");
    let Ok(text) = std::fs::read_to_string(&tsv) else {
        return;
    };
    let names = ["reference.png", "reference.jpg", "reference.jpeg"];
    let current = names
        .iter()
        .map(|f| dir.join(f))
        .find(|p| p.exists())
        .unwrap();
    let mut out = String::new();
    for (i, line) in text.lines().enumerate() {
        let mut cols: Vec<String> = line.split('\t').map(str::to_owned).collect();
        let run = dir.join("runs").join(&cols[0]);
        if i > 0 && run.join("final.png").exists() {
            let reference = names
                .iter()
                .map(|f| run.join(f))
                .find(|p| p.exists())
                .unwrap_or_else(|| current.clone());
            let score = likeness(
                &decode(&std::fs::read(reference).unwrap()),
                &decode(&std::fs::read(run.join("final.png")).unwrap()),
            );
            cols[10] = format!("{score:.2}");
        }
        out.push_str(&cols.join("\t"));
        out.push('\n');
    }
    std::fs::write(&tsv, out).unwrap();
    print!("{}", std::fs::read_to_string(&tsv).unwrap());
}

fn commit() -> String {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_default()
    };
    let commit = git(&["rev-parse", "--short", "HEAD"]);
    if git(&["status", "--porcelain", "--", "src"]).is_empty() {
        commit
    } else {
        format!("{commit}+dirty")
    }
}
