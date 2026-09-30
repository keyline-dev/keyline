# CLAUDE.md

## Project

Server-side image composition engine driven by an AI agent over MCP.

- Scene JSON + MCP — Design Spec: https://claude.ai/artifact/JA2qcvuxn3GgL1aTDkjMEk (Claude Doc; read and edit it with the docs tools, no local copy)
- Concepts: [docs/concepts.md](docs/concepts.md); scene format reference: [docs/scene.md](docs/scene.md); tools, replies and configuration: [docs/tools.md](docs/tools.md); install and setup per MCP client: the README's Quick start. Each fact has one home; the others link to it.

## Commands

- Build / run the MCP server (stdio): `cargo run --release`. Data lives in `--data <folder>` (default `~/.keyline-mcp`); every setting is a flag (`keyline-mcp --help`).
- Tests: `cargo test`. Golden PNGs are per OS in `tests/golden/<os>/` (macOS and Windows; CI compares both). Regenerate deliberately: macOS locally with `UPDATE_GOLDEN=1 cargo test --tests`; Windows only in CI: on a branch, set `UPDATE_GOLDEN: "1"` on the Windows end-to-end step in `.github/workflows/ci.yml` and upload `tests/golden/windows/` as an artifact, download it with `gh run download`, review it by eye, commit it, and drop the CI change before squashing into `main`.
- Web-font tests (need the network): `cargo test -- --ignored web_fonts google_fonts`.
- Rendering: GPU by default (Metal on macOS, Vulkan on Linux and Windows), CPU when no GPU opens or with `--renderer cpu`. Tests and reference images always use the CPU, which is deterministic on one OS version; glyph edges still differ slightly between OS versions (macOS 26 vs 27), so goldens are compared with a small tolerance.
- Real-LLM test (runs Claude Code headless on the Claude subscription, no API key): `cargo test --test llm_e2e -- --ignored --nocapture`. Set `CLAUDE_BIN` if `claude` isn't on PATH.
- Benchmarks: set `KEYLINE_MCP_BENCH=<label>` on `llm_e2e` (kept in `bench/reference-ad/`, committed) or `recreate_e2e` (rebuilds a design from its image; designs and runs stay in the gitignored `bench/recreate/local/`). Runs vary up to 2× in cost, so judge a change on at least 3 runs, and compare prompt versions only with each other (see `bench/reference-ad/README.md`).
- Release: bump the version in `Cargo.toml`, `plugin/.claude-plugin/plugin.json`, `mcpb/manifest.json` and `action.yml`'s `version` default, commit, and push a matching `vX.Y.Z` tag together with the commit (`git push --atomic origin main vX.Y.Z`; the plugin on `main` downloads that version, and the workflow refuses a mismatch; only full versions trigger it). `.github/workflows/release.yml` builds the Linux `.deb` (via `cargo deb`, a build tool, not a dependency) and tarballs (Linux amd64 and arm64, macOS arm64), a Windows x64 `.zip`, and the Claude Desktop `.mcpb` (macOS and Windows binaries), and attaches them to a GitHub release with `SHA256SUMS` and build provenance attestations. Then it runs the plugin, bundle and action checks, publishes the Docker image (`ghcr.io/keyline-dev/keyline-mcp`, with and without ffmpeg), bumps the Homebrew tap (`keyline-dev/homebrew-tap`, with the `HOMEBREW_TAP_TOKEN` secret), lists the server in the MCP registry, and moves the action's `v0` tag. The workflow's notes are near empty (nothing goes through pull requests), so write release notes and add them with `gh release edit vX.Y.Z --notes-file …`, with anything breaking under its own heading.
- Try a branch before releasing: the manual Preview workflow (`gh workflow run preview.yml --ref <branch>`) builds its Windows `.zip` and `.mcpb` as run artifacts, for testing on a machine or VM.

## Working agreements

- No worktrees: finish the current work, then run anything else (benchmarks included) in the main tree.
- No pull requests: commit straight to `main`; when work happens on a branch, squash it into one commit on `main`.
- Run the reference-ad benchmark (3 runs, see Commands) before committing anything that changes what the agent sees: tool text, instructions, replies, field names. Report the numbers and wait for the owner's approval.
- Docs describe what exists: no roadmap, no v2/MVP/phase labels in docs, comments or test names.

## When you change…

The agent-facing text lives in `src/server/mod.rs` (instructions and tool descriptions) and `src/server/schema.rs`; `tool_surface_stays_small` in `tests/e2e.rs` caps it at `TOOLS_LIST_MAX_CHARS` (10,000 characters). Each change below touches every listed place in the same commit:

| Change | Also update |
|---|---|
| A scene field or feature | `docs/scene.md`; the tool text if the agent should know it; a guess in `src/ops/guesses/` if models will write a common alternative; unit and end-to-end tests |
| The scene syntax (a field's name, a placeholder like `{{name}}`) | Every scene written in it: `tests/fixtures/`, `docs/scene.md`'s examples, the benchmark prompts (`tests/llm_e2e.rs`, `tests/recreate_e2e.rs`), and the site's `scenes/` once a release reads the new syntax |
| A server flag | `src/options.rs` (its `HELP` and tests); the flags table in `docs/tools.md`; the README's Options line; `mcpb/manifest.json` if Claude Desktop users should set it |
| A tool's reply | `docs/tools.md`; the reply readers in `tests/common/mod.rs` (`file_of`) and `src/server/cli.rs`; the reply snippets on the site |
| A client or install channel | The README's Quick start; the site's Install tabs |
| The one-line description | The GitHub repo description, `Cargo.toml`, `plugin/.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json`, `mcpb/manifest.json`, `server.json`, the `Dockerfile` label, `--help` in `src/options.rs`, the Homebrew formula in `keyline-dev/homebrew-tap`, the Glama listing and the site |
| Anything that changes pixels | The macOS and Windows goldens (see Commands), reviewed by eye |
| A distribution file (`plugin/`, `mcpb/`, `server.json`, `action.yml`, `Dockerfile`, the workflows) | The release checks that exercise it in `release.yml`; `claude plugin validate --strict` and `mcpb validate` pass |

Other repos in the `keyline-dev` org:

- `keyline-dev/keyline.dev`: the site. One static page; its images are keyline scenes in `scenes/`, rendered into `assets/` by `render.sh`; the logo is in `logo/`. Hosted on Cloudflare Pages from `main`.
- `keyline-dev/homebrew-tap`: the Homebrew formula, bumped by the release workflow; edit it by hand only to change more than the version and checksum.

Listings: the official MCP registry (from `server.json`, each release), Glama (claimed through `glama.json`), and awesome-mcp-servers. Contributions come under the agreement in `CONTRIBUTING.md`.

## Language

Rust only. Stable toolchain, latest edition. Packaging and client setup files sit beside the Rust code and are the exception: JSON manifests, workflow YAML, and the plugin's POSIX shell launcher (`plugin/scripts/launch.sh`).

## License

The project is source-available under PolyForm Shield 1.0.0 (`LICENSE`): anyone may use, modify and redistribute it, commercially too, for any purpose except providing a product that competes with it or with any product the owner provides using it (a hosted version counts). Keep it that way:

- Only add dependencies and assets under permissive licenses (MIT, Apache-2.0, BSD, OFL, …); no GPL/AGPL/copyleft, which would conflict.
- Third-party assets keep their own license file next to them (e.g. `fonts/OFL.txt`).
- Fonts or other files that may not be redistributed (e.g. Clash Display in `fonts/local/`) stay gitignored and are never committed.

## Third-party crates

- Prefer a good, popular crate over writing our own code.
- Pick only popular, actively maintained crates: high download counts on crates.io, recent releases, widely used in the ecosystem.
- Every new dependency must be approved by the owner before it is added. Propose it with its name, what it replaces, and why; do not add it to `Cargo.toml` until approved.
- Crates already named in the spec (e.g. skia-safe, rmcp, tokio, rayon, image, fast_image_resize, resvg) still need approval when first added.

## MCP tools: optimize for tokens

Every MCP tool is designed to minimize tokens the agent spends on tool definitions, arguments, and results (target: ~2k tokens of tool traffic per full composition). See "Token-efficiency requirements" in the spec.

- Few, verb-shaped tools; no per-property setters. Short tool and parameter descriptions.
- Batch by default: mutating tools take an array of ops, applied atomically.
- Never echo the scene. Mutations return changed ids and a `version` only.
- Omit defaults in both inputs and outputs; every field has a documented default.
- Compact text over JSON dumps. Edits reply `ok` or problems per size; `scene_describe` returns only problems unless `full` is set.
- Address layers by `role`, so the agent needs no read just to find an id.
- The server measures (text fit, overflow, crop), so the agent never has to render to check.
- Images: return file paths, not bytes. Previews only on request, as one small contact sheet of all sizes.
- Verify with measurements, not pixels: defects (`!overflow` …) to fix, advisories (`warn contrast`) to judge, facts (smallest text, upscaling) with no threshold.
- Errors: one short line saying what failed and how to fix it.

## Rust practices

Rust rules come from the **rust-skills** skill (265 rules in 26 categories, [leonardomso/rust-skills](https://github.com/leonardomso/rust-skills), MIT), installed at `.claude/skills/rust-skills` and pinned in `skills-lock.json` (restore with `npx skills experimental_install`).

- **Apply it to every Rust change.** Before writing or reviewing Rust, invoke `/rust-skills` and apply the categories for the task (its "Rule Application by Task" table), CRITICAL and HIGH first: `own-`, `err-`, `mem-`, `unsafe-`, `api-`, `async-`, `conc-`, `opt-`, `num-`. Read the rule file in `rules/` whenever a rule applies and you're unsure.
- **Review against it before calling work done.** Check the diff against `anti-` and `lint-` and the categories you touched, and fix violations. Cite the rule id (e.g. `err-no-unwrap-prod`) when a review comment or commit relies on one.
- **Machine-enforced.** `Cargo.toml` `[lints]` turns the rules a compiler can check into warnings: clippy correctness (deny), suspicious, style, complexity, perf, selected pedantic lints, no `unwrap`/`expect` outside tests (`clippy.toml` exempts `#[test]`), `missing_docs`, and `unsafe_code = "deny"`. Only `src/gpu.rs` (opening Metal or Vulkan, which are C APIs) allows `unsafe`, and every block there carries a `// SAFETY:` comment (rust-skills `unsafe-safety-comment`). Work is done only when `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` all pass and the diff passes a rust-skills review (above).
- **Allowances must say why.** Silence a lint only where it's intended, with `#[expect(lint, reason = "…")]` on the item (not `allow`, so it fails once it's no longer needed). Test support files allow `unwrap`/`expect` crate-wide, because a panic is how tests fail.
- **Project rules win on conflict.** This file overrides rust-skills. In particular, a rule that recommends a new crate (`thiserror`, `smallvec`, `compact_str`, …) still needs the owner's approval (see Third-party crates); a rule that adds structure the code doesn't need yet (builders, typestate, `#[non_exhaustive]` on internal types) is skipped until it's needed.
- Every public item has a `///` doc comment; for the scene model, the field docs are the schema's reference, so say what the value means, its unit and its default.

## Testing

Every change ships with tests. A feature is not done until it has both unit tests and end-to-end tests.

- **Unit tests:** in a `#[cfg(test)] mod tests` next to the code. Cover layout math (constraints, scale, text resize modes), schema parsing and defaults, and error paths.
- **End-to-end tests (deterministic):** in `tests/`. Start the real MCP server over stdio, call tools the way an agent would, and check the results. Rendered PNGs are compared against golden files in `tests/golden/`, allowing only the glyph anti-aliasing that differs between OS versions. These run in `cargo test` and in CI.
- **End-to-end tests with a real LLM:** a real model drives the MCP server to build a scene, e.g. the reference ad from the spec. These tests check that the task succeeded and track token usage against the ~2k target. They run Claude Code headless on the owner's Claude subscription and use its quota, so they are opt-in: mark them `#[ignore]` and run them with `cargo test -- --ignored`. They never run in the default `cargo test`.
- Updating golden files is an explicit step. Never regenerate them just to make a failing test pass.
