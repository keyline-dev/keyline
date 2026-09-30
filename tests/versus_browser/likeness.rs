//! How alike two images look, and the reference ad rendered two ways to
//! compare against. `likeness` is copied from `tests/recreate_e2e.rs`, so
//! the two benchmarks can change independently.

use std::path::{Path, PathBuf};

use serde_json::json;
use skia_safe::{Data, Image, surfaces};

use crate::common::{self, Mcp};
use crate::prompts::Task;
use crate::{bench_root, random_hex, tooling_bin, write_assets};

pub fn decode(bytes: &[u8]) -> Image {
    Image::from_encoded(Data::new_copy(bytes)).expect("decodable image")
}

/// How far apart two images look, 0–255, lower is closer. Both are
/// averaged into 8 px blocks at the reference's size, then compared in
/// 64 px tiles, each tile at its best offset within ±64 px: a region that
/// sits a little off (a photo cropped 50 px higher) still matches, while
/// wrong colors, missing text or a misplaced block still count, since a
/// tile moves as one piece.
pub fn likeness(reference: &Image, img: &Image) -> f32 {
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

/// Renders the reference ad two ways, for likeness: by keyline
/// (`build_reference_ad`) and by Chrome (`reference.html`).
pub async fn references() -> PathBuf {
    let root = bench_root().join("reference-ad/reference");
    if root.join("chrome/sky.png").exists() && root.join("keyline/sky.png").exists() {
        return root;
    }
    std::fs::create_dir_all(root.join("keyline")).unwrap();
    std::fs::create_dir_all(root.join("chrome")).unwrap();
    let mcp = Mcp::start("vs-ref").await;
    let scene = common::build_reference_ad(&mcp).await;
    let rendered = mcp.ok("render", json!({"sceneId": scene})).await;
    for line in rendered.lines().filter(|l| !l.starts_with(' ')) {
        if let Some((size, path)) = common::file_of(line) {
            std::fs::copy(path, root.join(format!("keyline/{size}.png"))).unwrap();
        }
    }
    mcp.stop().await;

    let dir = std::env::temp_dir().join(format!("vs-ref-{}", random_hex()));
    std::fs::create_dir_all(&dir).unwrap();
    write_assets(Task::ReferenceAd, &dir);
    std::fs::copy(
        bench_root().join("reference-ad/reference.html"),
        dir.join("index.html"),
    )
    .unwrap();
    for (size, w, h, _) in Task::ReferenceAd.sizes() {
        let status = std::process::Command::new(tooling_bin().join("playwright"))
            .args(["screenshot", "--viewport-size", &format!("{w},{h}")])
            .arg(format!("file://{}/index.html", dir.display()))
            .arg(root.join(format!("chrome/{size}.png")))
            .status()
            .unwrap();
        assert!(status.success(), "playwright screenshot failed");
    }
    let _ = std::fs::remove_dir_all(&dir);
    root
}

/// The run's likeness to one reference, averaged over the sizes.
pub fn likeness_to(run: &Path, reference: &Path) -> f64 {
    let sizes = Task::ReferenceAd.sizes();
    let sum: f64 = sizes
        .iter()
        .map(|(size, ..)| {
            let read = |d: &Path| decode(&std::fs::read(d.join(format!("{size}.png"))).unwrap());
            f64::from(likeness(&read(reference), &read(run)))
        })
        .sum();
    sum / 3.0
}
