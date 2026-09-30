//! Shared end-to-end helpers: a real server process over stdio, and the
//! reference ad from the spec's "Done when" section.

// Test support: a panic is how a test reports failure.
#![allow(clippy::unwrap_used, clippy::expect_used)]
#![allow(dead_code)] // each test binary uses a different subset

pub mod golden;

use std::path::PathBuf;

use base64::Engine;
use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, CallToolResult, Tool},
    service::RunningService,
    transport::TokioChildProcess,
};
use serde_json::{Value, json};
use skia_safe::{Color, EncodedImageFormat, Paint, Rect, surfaces};

pub struct Mcp {
    client: RunningService<RoleClient, ()>,
    pub data: PathBuf,
    /// Characters sent and received through tool calls, for token budgets.
    pub traffic: std::cell::Cell<usize>,
}

impl Mcp {
    /// Starts the real server binary with a fresh data directory.
    pub async fn start(name: &str) -> Mcp {
        let data =
            std::env::temp_dir().join(format!("keyline-mcp-e2e-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        Self::start_in(data).await
    }

    /// Starts the server on an existing data directory, as after a restart.
    pub async fn start_in(data: PathBuf) -> Mcp {
        Self::start_with(data, &[]).await
    }

    /// Starts the server with a fresh data directory and command-line `args`.
    pub async fn start_args(name: &str, args: &[&str]) -> Mcp {
        let data =
            std::env::temp_dir().join(format!("keyline-mcp-e2e-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        Self::start_with(data, args).await
    }

    async fn start_with(data: PathBuf, args: &[&str]) -> Mcp {
        let mut cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_keyline-mcp"));
        cmd.args(args);
        cmd.arg("--data").arg(&data);
        // Reference images are CPU renders, deterministic on each OS version.
        cmd.args(["--renderer", "cpu"]);
        let client =
            ().serve(TokioChildProcess::new(cmd).expect("spawn server"))
                .await
                .expect("MCP handshake");
        Mcp {
            client,
            data,
            traffic: 0.into(),
        }
    }

    /// The name the server gave in its MCP handshake.
    pub fn server_name(&self) -> String {
        self.client
            .peer_info()
            .expect("handshake done")
            .server_info
            .as_ref()
            .expect("server info")
            .name
            .clone()
    }

    pub async fn tools(&self) -> Vec<Tool> {
        self.client.list_all_tools().await.expect("tools/list")
    }

    /// Calls a tool and returns the full result.
    pub async fn call_raw(&self, tool: &str, args: Value) -> CallToolResult {
        let Value::Object(args) = args else {
            panic!("args must be an object")
        };
        // Uploaded file bytes aren't tokens the agent writes; leave them out.
        let mut counted = args.clone();
        counted.remove("base64");
        let args_len = Value::Object(counted).to_string().len();
        let result = self
            .client
            .call_tool(CallToolRequestParams::new(tool.to_owned()).with_arguments(args))
            .await
            .expect("tools/call");
        self.traffic
            .set(self.traffic.get() + args_len + text(&result).len());
        result
    }

    /// Calls a tool and returns its text, or `Err` with the tool's error text.
    pub async fn call(&self, tool: &str, args: Value) -> Result<String, String> {
        let r = self.call_raw(tool, args).await;
        if r.is_error == Some(true) {
            Err(text(&r))
        } else {
            Ok(text(&r))
        }
    }

    /// Like `call`, but panics on a tool error.
    pub async fn ok(&self, tool: &str, args: Value) -> String {
        self.call(tool, args)
            .await
            .unwrap_or_else(|e| panic!("{tool} failed: {e}"))
    }

    pub async fn stop(self) {
        let data = self.shut_down().await;
        let _ = std::fs::remove_dir_all(&data);
    }

    /// Stops the server but keeps its data directory; returns it.
    pub async fn shut_down(self) -> PathBuf {
        let _ = self.client.cancel().await;
        self.data
    }
}

pub fn text(r: &CallToolResult) -> String {
    r.content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// A 1600×900 stand-in for a photo: sky, sun and hills.
pub fn photo_png() -> Vec<u8> {
    let mut surface = surfaces::raster_n32_premul((1600, 900)).expect("surface");
    let c = surface.canvas();
    c.clear(Color::from_rgb(120, 180, 230));
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_color(Color::from_rgb(255, 210, 80));
    c.draw_circle((1250.0, 220.0), 110.0, &p);
    p.set_color(Color::from_rgb(70, 140, 70));
    c.draw_circle((400.0, 1300.0), 700.0, &p);
    p.set_color(Color::from_rgb(50, 110, 60));
    c.draw_circle((1300.0, 1400.0), 750.0, &p);
    p.set_color(Color::from_rgb(200, 60, 50));
    c.draw_rect(Rect::from_xywh(700.0, 480.0, 200.0, 160.0), &p);
    let png = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .expect("encode");
    png.as_bytes().to_vec()
}

pub const MAIL_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 56 44" width="56" height="44">
<rect x="2" y="2" width="52" height="40" rx="5" fill="none" stroke="#fff" stroke-width="4"/>
<path d="M4 6 L28 26 L52 6" fill="none" stroke="#fff" stroke-width="4" stroke-linejoin="round"/></svg>"##;

pub const CHECK_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40">
<circle cx="20" cy="20" r="20" fill="#D0202E"/>
<path d="M11 21 L17 27 L29 14" fill="none" stroke="#fff" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;

/// Sizes from the spec: portrait master, a wide and a skyscraper.
pub fn ad_sizes() -> Value {
    json!([
        {"id": "portrait", "width": 1080, "height": 1350},
        {"id": "wide", "width": 1200, "height": 1000, "scale": 0.85},
        {"id": "sky", "width": 300, "height": 600, "scale": 0.28}
    ])
}

/// The vote-by-mail reference ad: two-color headline, photo band, three
/// name columns, red call-to-action bar with an icon, three steps with
/// icons, footer. Built the way an agent would, in 5 calls.
pub async fn build_reference_ad(mcp: &Mcp) -> String {
    let created = mcp
        .ok(
            "scene_create",
            json!({"width": 1080, "height": 1350, "sizes": ad_sizes()}),
        )
        .await;
    let id = created.split(' ').next().expect("scene id").to_owned();
    mcp.ok(
        "asset_add",
        json!({"sceneId": id, "id": "photo", "base64": b64(&photo_png())}),
    )
    .await;
    mcp.ok(
        "asset_add",
        json!({"sceneId": id, "id": "mail", "base64": b64(MAIL_SVG.as_bytes())}),
    )
    .await;
    mcp.ok(
        "asset_add",
        json!({"sceneId": id, "id": "check", "base64": b64(CHECK_SVG.as_bytes())}),
    )
    .await;

    let navy = "#1B2A5C";
    let red = "#D0202E";
    let bottom = json!({"horizontal": "stretch", "vertical": "bottom"});
    let mut layers = vec![
        json!({"id": "headline",
            "type": "text",
            "x": 60,
            "y": 40,
            "width": 960,
            "fontSize": 64,
            "fontWeight": 800,
            "textAlign": "center",
            "color": navy,
            "text": "Proven RESULTS for WILLOWMERE Families",
            "ranges": [
                {"start": 7, "end": 14, "color": red},
                {"start": 19, "end": 29, "color": red}],
            "constraints": {"horizontal": "stretch", "vertical": "top"}}),
        json!({"id": "photo",
            "type": "image",
            "asset": "photo",
            "x": 0,
            "y": 220,
            "width": 1080,
            "height": 460,
            "constraints": {"horizontal": "stretch", "vertical": "stretch"}}),
        json!({"id": "names", "type": "frame", "y": 700, "width": 1080, "height": 110, "constraints": bottom}),
        json!({"id": "cta",
            "type": "frame",
            "y": 830,
            "width": 1080,
            "height": 100,
            "constraints": bottom,
            "children": [
                {"id": "mail", "type": "image", "asset": "mail", "x": 318, "y": 28, "width": 56, "height": 44, "constraints": {"horizontal": "center", "vertical": "center"}},
                {"id": "cta-text", "type": "text", "text": "VOTE BY MAIL", "x": 394, "y": 21, "fontSize": 48, "fontWeight": 800, "color": "#FFFFFF", "constraints": {"horizontal": "center", "vertical": "center"}}],
            "fill": red}),
        json!({"id": "steps", "type": "frame", "y": 950, "width": 1080, "height": 290, "constraints": bottom}),
        json!({"id": "footer",
            "type": "text",
            "x": 60,
            "y": 1280,
            "width": 960,
            "fontSize": 30,
            "textAlign": "center",
            "color": "#6B7280",
            "text": "Paid for by Willowmere Forward · willowmereforward.org",
            "constraints": bottom}),
    ];
    let columns = [
        ("Dana Levi", "Mayor"),
        ("Omar Haddad", "Council"),
        ("Ruth Cohen", "Council"),
    ];
    for (i, (name, title)) in columns.iter().enumerate() {
        let x = 60 + i * 330;
        let col = json!({"horizontal": "scale", "vertical": "top"});
        layers.push(json!({"type": "text",
                "parent": "names",
                "role": "name",
                "text": name,
                "x": x,
                "y": 10,
                "width": 300,
                "fontSize": 36,
                "fontWeight": 700,
                "textAlign": "center",
                "color": navy,
                "constraints": col}));
        layers.push(json!({"type": "text",
            "parent": "names",
            "role": "title",
            "text": title,
            "x": x,
            "y": 60,
            "width": 300,
            "fontSize": 30,
            "fontWeight": 500,
            "textAlign": "center",
            "color": red,
            "constraints": col}));
    }
    let steps = [
        "Request your ballot by October 20",
        "Fill it out at home",
        "Mail it back by November 3",
    ];
    for (i, step) in steps.iter().enumerate() {
        let y = 20 + i * 95;
        layers.push(
            json!({"type": "image", "parent": "steps", "asset": "check", "x": 120, "y": y,
            "width": 48, "height": 48}),
        );
        layers.push(json!({"type": "text",
            "parent": "steps",
            "role": "step",
            "text": step,
            "x": 190,
            "y": y + 4,
            "width": 800,
            "fontSize": 32,
            "fontWeight": 500,
            "color": navy,
            "constraints": {"horizontal": "stretch", "vertical": "top"}}));
    }
    mcp.ok("layer_add", json!({"sceneId": id, "layers": layers}))
        .await;
    id
}

/// A simple stars-and-stripes flag (original drawing for tests).
pub fn flag_svg() -> String {
    let stripes: String = (0..13)
        .map(|i| {
            let color = if i % 2 == 0 { "#B22234" } else { "#FFFFFF" };
            format!(
                r#"<rect y="{}" width="190" height="10" fill="{color}"/>"#,
                i * 10
            )
        })
        .collect();
    let stars: String = (0..5)
        .flat_map(|r| (0..6).map(move |c| (c, r)))
        .map(|(c, r)| {
            format!(
                r##"<circle cx="{}" cy="{}" r="2.2" fill="#FFFFFF"/>"##,
                7 + c * 12,
                7 + r * 13
            )
        })
        .collect();
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 190 130" width="190" height="130">{stripes}<rect width="76" height="70" fill="#3C3B6E"/>{stars}</svg>"##
    )
}

/// A leopard-spot tile that repeats seamlessly (original drawing for tests).
pub const LEOPARD_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 80" width="80" height="80">
<rect width="80" height="80" fill="#D9A441"/>
<g fill="none" stroke="#3B2412" stroke-width="5" stroke-linecap="round">
<path d="M10 14 q8 -9 16 0"/><path d="M50 10 q9 6 2 15"/><path d="M28 44 q-9 8 0 16"/>
<path d="M60 48 q10 -3 12 8"/><path d="M8 66 q7 7 15 1"/></g>
<g fill="#3B2412"><circle cx="18" cy="20" r="3.5"/><circle cx="55" cy="18" r="3"/>
<circle cx="35" cy="52" r="3.5"/><circle cx="66" cy="62" r="3"/><circle cx="14" cy="72" r="2.5"/></g>
</svg>"##;

/// A `render` reply line's size id and file path: `<size> <path> (<facts>)`;
/// the path may hold spaces.
/// Every rendered file a render reply names, `(size, path)`: its lines
/// that aren't drawn text (indented), the fonts or the preview.
pub fn files(reply: &str) -> impl Iterator<Item = (&str, &str)> {
    reply
        .lines()
        .filter(|l| !l.starts_with(' ') && !l.starts_with("fonts: ") && !l.starts_with("preview "))
        .filter_map(file_of)
}

pub fn file_of(line: &str) -> Option<(&str, &str)> {
    let (size, rest) = line.split_once(' ')?;
    Some((size, rest.rsplit_once(" (").map_or(rest, |(path, _)| path)))
}
