//! Benchmark with a real model: keyline against a headless-browser agent.
//! Claude Code (headless, on your Claude subscription) makes the same images
//! three ways, and each run is kept in `bench/versus-browser/`:
//!
//! - `keyline`: only the keyline MCP server;
//! - `browser-cli`: Bash, Write, Edit, Read and Playwright's screenshot CLI;
//! - `browser-mcp`: Playwright MCP plus Write, Edit, Read.
//!
//! ```sh
//! KEYLINE_BENCH_ARM=browser-cli KEYLINE_BENCH_TASK=reference-ad KEYLINE_MCP_BENCH=1 \
//!   cargo test --release --test versus_browser claude_makes_the_images -- --ignored --nocapture
//! cargo test --release --test versus_browser judge_runs -- --ignored --nocapture
//! ```
//!
//! `CLAUDE_BIN` and `KEYLINE_MCP_TEST_MODEL` work as in `llm_e2e`. The
//! browser arms need `npm ci` and `npx playwright install
//! chromium-headless-shell` in `bench/versus-browser/tooling/` first (see
//! `bench/versus-browser/README.md`).

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "../common/mod.rs"]
mod common;
mod judge;
mod likeness;
mod metrics;
mod prompts;
mod review;
mod speaker;
mod table;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use common::{CHECK_SVG, MAIL_SVG, Mcp, b64, photo_png};
use prompts::{Arm, SYSTEM, Task};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;

const MAX_TURNS: &str = "60";
const MAX_BUDGET_USD: &str = "8";
const TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Where runs, prompts and tooling live.
pub fn bench_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/versus-browser")
}

/// The `claude` CLI: `CLAUDE_BIN`, or `claude` on PATH.
pub fn claude_bin() -> String {
    std::env::var("CLAUDE_BIN").unwrap_or_else(|_| "claude".into())
}

/// 16 random hex digits.
pub fn random_hex() -> String {
    use std::hash::{BuildHasher, RandomState};
    let seed = std::time::SystemTime::now();
    format!("{:016x}", RandomState::new().hash_one(seed))
}

fn tooling_bin() -> PathBuf {
    let bin = bench_root().join("tooling/node_modules/.bin");
    assert!(
        bin.join("playwright").exists(),
        "run `npm ci` in bench/versus-browser/tooling first"
    );
    bin
}

/// An asset's bytes, the same for every arm.
fn asset(id: &str) -> Vec<u8> {
    match id {
        "photo" => photo_png(),
        "mail" => MAIL_SVG.as_bytes().to_vec(),
        "check" => CHECK_SVG.as_bytes().to_vec(),
        "logo" => speaker::LOGO_SVG.as_bytes().to_vec(),
        "portrait" => speaker::portrait_png(),
        _ => unreachable!("unknown asset {id}"),
    }
}

/// Writes the task's assets and Inter into `dir`, as the browser arms get them.
fn write_assets(task: Task, dir: &Path) {
    for (id, file) in task.assets() {
        std::fs::write(dir.join(file), asset(id)).unwrap();
    }
    let inter = Path::new(env!("CARGO_MANIFEST_DIR")).join("fonts/InterVariable.ttf");
    std::fs::copy(inter, dir.join("Inter.ttf")).unwrap();
}

/// A PNG's width and height, from its header.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let be = |r: std::ops::Range<usize>| Some(u32::from_be_bytes(bytes.get(r)?.try_into().ok()?));
    bytes
        .starts_with(b"\x89PNG\r\n\x1a\n")
        .then(|| be(16..20).zip(be(20..24)))
        .flatten()
}

/// The commit under test; `+dirty` when `src/` has uncommitted changes.
fn commit() -> String {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
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

/// The keyline arm's scene: the task's sizes and assets, set up before the
/// agent starts (uploading isn't what's tested).
async fn keyline_scene(mcp: &Mcp, task: Task) -> String {
    let sizes: Vec<Value> = task
        .sizes()
        .iter()
        .map(|&(id, w, h, scale)| {
            let mut s = json!({"id": id, "width": w, "height": h});
            if (scale - 1.0).abs() > f32::EPSILON {
                s["scale"] = json!(scale);
            }
            s
        })
        .collect();
    let (_, w, h, _) = task.sizes()[0];
    let created = mcp
        .ok(
            "scene_create",
            json!({"width": w, "height": h, "sizes": sizes}),
        )
        .await;
    let scene = created.split(' ').next().unwrap().to_owned();
    for (id, _) in task.assets() {
        mcp.ok(
            "asset_add",
            json!({"sceneId": scene, "id": id, "base64": b64(&asset(id))}),
        )
        .await;
    }
    scene
}

async fn wait_for_port(port: u16) {
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("static server didn't start on port {port}");
}

#[tokio::test]
#[ignore = "runs Claude Code on your subscription; run with --ignored"]
async fn claude_makes_the_images() {
    let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let arm = Arm::parse(&var("KEYLINE_BENCH_ARM"));
    let task = Task::parse(&var("KEYLINE_BENCH_TASK"));
    let label = var("KEYLINE_MCP_BENCH");
    let run = bench_root()
        .join(task.name())
        .join(format!("{}-{label}", arm.name()));
    assert!(
        !run.exists(),
        "{} exists: every run counts, so pick a new label",
        run.display()
    );
    let bin = tooling_bin();
    let work = std::env::temp_dir().join(format!("vs-{}", random_hex()));
    std::fs::create_dir_all(&work).unwrap();
    let work = work.canonicalize().unwrap();
    write_assets(task, &work);

    let mut servers = serde_json::Map::new();
    let mut keyline = None;
    let mut static_server = None;
    let mut port = 0;
    let (place, tools, allowed) = match arm {
        Arm::Keyline => {
            let mcp = Mcp::start("vs").await;
            let scene = keyline_scene(&mcp, task).await;
            servers.insert(
                "scene".into(),
                json!({"command": env!("CARGO_BIN_EXE_keyline-mcp"), "args": ["--data", mcp.data]}),
            );
            keyline = Some((mcp, scene.clone()));
            (scene, "", "mcp__scene__*")
        }
        Arm::BrowserCli => (
            work.display().to_string(),
            "Bash,Write,Edit,Read",
            "Bash,Write,Edit,Read",
        ),
        Arm::BrowserMcp => {
            port = std::net::TcpListener::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port();
            let server = tokio::process::Command::new("python3")
                .args([
                    "-m",
                    "http.server",
                    &port.to_string(),
                    "--bind",
                    "127.0.0.1",
                ])
                .current_dir(&work)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .expect("start python3 -m http.server");
            static_server = Some(server);
            wait_for_port(port).await;
            servers.insert(
                "playwright".into(),
                json!({"command": bin.join("playwright-mcp"),
                    "args": ["--headless", "--isolated", "--browser", "chromium"]}),
            );
            (
                work.display().to_string(),
                "Write,Edit,Read",
                "mcp__playwright__*,Write,Edit,Read",
            )
        }
    };
    let prompt = task.prompt(&prompts::tools(arm, task, &place, port));
    let mcp_config = json!({ "mcpServers": servers }).to_string();

    let mut cmd = tokio::process::Command::new(claude_bin());
    cmd.arg("-p")
        .arg(&prompt)
        .args(["--output-format", "stream-json", "--verbose"])
        .args(["--mcp-config", &mcp_config, "--strict-mcp-config"])
        .args(["--tools", tools, "--allowedTools", allowed])
        .args(["--system-prompt", SYSTEM, "--max-turns", MAX_TURNS])
        .args(["--max-budget-usd", MAX_BUDGET_USD])
        .args(["--setting-sources", "", "--no-session-persistence"])
        .current_dir(&work)
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Ok(model) = std::env::var("KEYLINE_MCP_TEST_MODEL") {
        cmd.args(["--model", &model]);
    }
    let mut child = cmd
        .spawn()
        .expect("run claude; set CLAUDE_BIN if it isn't on PATH");
    let read_all = |mut r: Box<dyn tokio::io::AsyncRead + Unpin + Send>| {
        tokio::spawn(async move {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf).await;
            buf
        })
    };
    let stdout = read_all(Box::new(child.stdout.take().unwrap()));
    let stderr = read_all(Box::new(child.stderr.take().unwrap()));
    let timed_out = tokio::time::timeout(TIMEOUT, child.wait()).await.is_err();
    if timed_out {
        let _ = child.kill().await;
    }
    let (stdout, stderr) = (stdout.await.unwrap(), stderr.await.unwrap());
    drop(static_server);

    std::fs::create_dir_all(run.join("work")).unwrap();
    std::fs::write(run.join("events.jsonl"), metrics::elide_images(&stdout)).unwrap();
    if !stderr.is_empty() {
        std::fs::write(run.join("stderr.txt"), &stderr).unwrap();
    }
    std::fs::write(
        run.join("prompt.md"),
        format!(
            "# System\n\n{SYSTEM}\n\n# Task\n\n{prompt}\n\n# Setup\n\n- tools: `{tools}`\n- allowed: `{allowed}`\n- MCP servers: `{}`\n",
            servers.keys().cloned().collect::<Vec<_>>().join(", ")
        ),
    )
    .unwrap();

    // The final PNGs: keyline's rendered by the harness from the scene the
    // agent left, the browser arms' taken from the folder.
    if let Some((mcp, scene)) = keyline {
        let problems = mcp
            .call("scene_describe", json!({"sceneId": scene}))
            .await
            .unwrap_or_else(|e| e);
        let described = mcp
            .call("scene_describe", json!({"sceneId": scene, "full": true}))
            .await
            .unwrap_or_else(|e| e);
        std::fs::write(
            run.join("work/layout.txt"),
            format!("{problems}\n\n{described}"),
        )
        .unwrap();
        if let Ok(rendered) = mcp.call("render", json!({"sceneId": scene})).await {
            for line in rendered.lines().filter(|l| !l.starts_with(' ')) {
                if let Some((size, path)) = common::file_of(line) {
                    let _ = std::fs::copy(path, run.join(format!("{size}.png")));
                }
            }
        }
        mcp.stop().await;
    } else {
        for (size, ..) in task.sizes() {
            let _ = std::fs::copy(
                work.join(format!("{size}.png")),
                run.join(format!("{size}.png")),
            );
        }
        // The agent's own files: its HTML, CSS and scripts.
        for entry in std::fs::read_dir(&work).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            if ["html", "htm", "css", "js", "mjs", "cjs", "ts", "py", "sh"].contains(&ext) {
                let _ = std::fs::copy(&path, run.join("work").join(entry.file_name()));
            }
        }
    }
    let files_ok = task.sizes().iter().all(|&(size, w, h, _)| {
        std::fs::read(run.join(format!("{size}.png")))
            .ok()
            .and_then(|b| png_size(&b))
            == Some((w, h))
    });
    let _ = std::fs::remove_dir_all(&work);

    let events = metrics::parse(&stdout);
    let mut summary = metrics::summarize(&events, arm != Arm::Keyline);
    for (k, v) in [
        ("task", json!(task.name())),
        ("arm", json!(arm.name())),
        ("label", json!(label)),
        ("promptVersion", json!(prompts::VERSION)),
        ("commit", json!(commit())),
        ("filesOk", json!(files_ok)),
        ("timedOut", json!(timed_out)),
    ] {
        summary[k] = v;
    }
    std::fs::write(
        run.join("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .unwrap();
    println!("{summary:#}\nsaved to {}", run.display());
    // Checked after saving: a run without a result is an infrastructure
    // failure to look into, not a result.
    assert!(
        timed_out || events.iter().any(|e| e["type"] == "result"),
        "no result from claude: {}",
        String::from_utf8_lossy(&stderr)
    );
}

/// Judges every unjudged run with the blind checklist judge, scores the
/// reference ad's likeness, rewrites `results.tsv` and prints the medians.
#[tokio::test]
#[ignore = "runs Claude Code on your subscription; run with --ignored"]
async fn judge_runs() {
    let refs = likeness::references().await;
    for run in table::runs() {
        if run.judge.is_some() || run.summary["filesOk"] != true {
            continue;
        }
        let task = Task::parse(run.summary["task"].as_str().unwrap());
        let mut verdict = judge::judge(&run.dir, task).await;
        if task == Task::ReferenceAd {
            verdict["likenessKeyline"] =
                json!(likeness::likeness_to(&run.dir, &refs.join("keyline")));
            verdict["likenessChrome"] =
                json!(likeness::likeness_to(&run.dir, &refs.join("chrome")));
        }
        println!("{}: {verdict}", run.dir.display());
        std::fs::write(
            run.dir.join("judge.json"),
            serde_json::to_string_pretty(&verdict).unwrap(),
        )
        .unwrap();
    }
    let runs = table::runs();
    table::write_tsv(&runs);
    println!("## Counted runs\n{}", table::medians(&runs, true));
    println!(
        "## All runs, pilots included\n{}",
        table::medians(&runs, false)
    );
    print!("{}", review::agreement(&runs));
}

#[test]
fn png_sizes_are_read_from_the_header() {
    assert_eq!(png_size(&photo_png()), Some((1600, 900)));
    assert_eq!(png_size(&speaker::portrait_png()), Some((800, 800)));
    assert_eq!(png_size(b"not a png"), None);
}
