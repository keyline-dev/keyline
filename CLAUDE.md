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
- Benchmarks: set `KEYLINE_MCP_BENCH=<label>` on `llm_e2e` (kept in `reference-ad/` of `keyline-dev/keyline-bench`, checked out at `../keyline-bench` or `KEYLINE_BENCH`; commit and push each run there) or `recreate_e2e` (rebuilds a design from its image; designs and runs stay in the gitignored `bench/recreate/local/`). Runs vary up to 2× in cost, so judge a change on at least 3 runs, and compare prompt versions only with each other (see keyline-bench's `reference-ad/README.md`). Run data never goes in this repo: Anthropic's plugin directory downloads it whole and refuses anything over 50 MiB compressed.
- Release: bump the version in `Cargo.toml`, `plugin/.claude-plugin/plugin.json`, `mcpb/manifest.json` and `action.yml`'s `version` default, commit, and push a matching `vX.Y.Z` tag together with the commit (`git push --atomic origin main vX.Y.Z`; the plugin on `main` downloads that version, and the workflow refuses a mismatch; only full versions trigger it). `.github/workflows/release.yml` builds the Linux `.deb` (via `cargo deb`, a build tool, not a dependency) and tarballs (Linux amd64 and arm64, macOS arm64), a Windows x64 `.zip`, and the Claude Desktop `.mcpb` (macOS and Windows binaries), and attaches them to a GitHub release with `SHA256SUMS` and build provenance attestations. Then it runs the plugin, bundle and action checks, publishes the Docker image (`ghcr.io/keyline-dev/keyline-mcp`, with and without ffmpeg), bumps the Homebrew tap (`keyline-dev/homebrew-tap`, with the `HOMEBREW_TAP_TOKEN` secret), lists the server in the MCP registry, and moves the action's `v0` tag. The workflow's notes are near empty (nothing goes through pull requests), so write release notes and add them with `gh release edit vX.Y.Z --notes-file …`, with anything breaking under its own heading.
- After every release, update the site (`keyline-dev/keyline.dev`, checked out at `../keyline.dev`) in the same sitting, then commit and push it:
  1. `npm run docs` there: rebuilds `/docs/`, `/benchmark/`, `/license/`, `/privacy/`, `/security/`, `llms.txt`, `llms-full.txt` and the sitemap from this repo's README Quick start, `docs/`, `LICENSE`, `PRIVACY.md` and `SECURITY.md`, and keyline-bench's `versus-browser/README.md`. Change `docs.mjs` or the source Markdown, never the output.
  2. `./render.sh` with the released binary (and `scenes/` first if the scene syntax changed). Look at every asset that changes; a changed render needs a new file name (`-v1`, `.r2`), since `assets/` is cached for a year. Video posters are `--time` stills.
  3. `index.html`, whose facts are written by hand: the Install tabs (plugin, Homebrew, Docker, `.mcpb`, the action's `@v0` and inputs); the reply snippets in the hero and in "How it works" (real replies of this version); the feature cards and the Questions answers (tool count, output formats, clients, the Cursor snippet), repeated word for word in the FAQPage JSON-LD; and the SoftwareApplication JSON-LD `description` and `featureList`. `docs.mjs` copies that description into `llms.txt`, `llms-full.txt` and the install page. The version number isn't written anywhere: `site.js` reads it from GitHub.
  4. `docs.mjs`'s `FACTS` (clients, platforms, formats, benchmark figures) and its Cursor line for `llms.txt`.
  5. Only when keyline-bench's `versus-browser/` has new runs: the benchmark table and `2×` / `5×` stat, the Puppeteer answer in Questions (text and JSON-LD) and `FACTS`, claimed only as the benchmark README's "What the rules allow" permits.
  6. Check the page at 1440, 768 and 390px wide in light and dark, push (Cloudflare Pages deploys `main`), and check the live home page, a missing path (a 404) and `llms.txt`. Listings need nothing beyond the Updates column of the Listings table below: the release workflow updates the MCP registry, which Glama and PulseMCP read.
- Try a branch before releasing: the manual Preview workflow (`gh workflow run preview.yml --ref <branch>`) builds its Windows `.zip` and `.mcpb` as run artifacts, for testing on a machine or VM.

## Working agreements

- No worktrees: finish the current work, then run anything else (benchmarks included) in the main tree.
- No pull requests: commit straight to `main`; when work happens on a branch, squash it into one commit on `main`.
- Run the reference-ad benchmark (3 runs, see Commands) before committing anything that changes what the agent sees: tool text, instructions, replies, field names. Report the numbers and wait for the owner's approval.
- After any benchmark or sample run, read the agents' turns before the totals or the images: every call, what it sent, and what came back. Look for refused edits, fixes repeated on one value, replies the agent misread or ignored, and where it stopped. The numbers say how much a run cost; the turns say why.
- Docs describe what exists: no roadmap, no v2/MVP/phase labels in docs, comments or test names.

## When you change…

The agent-facing text lives in `src/server/mod.rs` (instructions and tool descriptions) and `src/server/schema.rs`; `tool_surface_stays_small` in `tests/e2e.rs` caps it at `TOOLS_LIST_MAX_CHARS` (10,000 characters). Each change below touches every listed place in the same commit:

| Change | Also update |
|---|---|
| A scene field or feature | `docs/scene.md`; the tool text if the agent should know it; a guess in `src/ops/guesses/` if models will write a common alternative; unit and end-to-end tests |
| The scene syntax (a field's name, a placeholder like `{{name}}`) | Every scene written in it: `tests/fixtures/`, `docs/scene.md`'s examples, the benchmark prompts (`tests/llm_e2e.rs`, `tests/recreate_e2e.rs`), and the site's `scenes/` once a release reads the new syntax |
| A server flag | `src/options.rs` (its `HELP` and tests); the flags table in `docs/tools.md`; the README's Options line; `mcpb/manifest.json` if Claude Desktop users should set it |
| A tool's reply | `docs/tools.md`; the reply readers in `tests/common/mod.rs` (`file_of`) and `src/server/cli.rs`; the reply snippets on the site (hero and "How it works") |
| A client or install channel | The README's Quick start; the site's Install tabs, its Questions answers and JSON-LD, and `FACTS` in its `docs.mjs` |
| The one-line description | The GitHub repo description, `Cargo.toml`, `plugin/.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json`, `mcpb/manifest.json`, `server.json`, the `Dockerfile` label, `--help` in `src/options.rs`, the Homebrew formula in `keyline-dev/homebrew-tap`, the Glama listing and the site |
| What keyline fetches, stores or runs (a new host, a new file location, a change to `plugin/scripts/launch.sh`) | `PRIVACY.md` (built into the site's `/privacy/`) and `plugin/README.md`: Anthropic's directory scans each commit and rejects behavior the README doesn't disclose |
| A tool's effect (it starts deleting or only reads) | Its `annotations` in `src/server/mod.rs` and the check in `tool_surface_stays_small` |
| Anything that changes pixels | The macOS and Windows goldens (see Commands), reviewed by eye |
| A distribution file (`plugin/`, `mcpb/`, `server.json`, `action.yml`, `Dockerfile`, the workflows) | The release checks that exercise it in `release.yml`; `claude plugin validate --strict` and `mcpb validate` pass |

Other repos in the `keyline-dev` org:

- `keyline-dev/keyline.dev`: the site. Static pages; its images are keyline scenes in `scenes/`, rendered into `assets/` by `render.sh`; the logo is in `logo/`. Its `/docs/`, `/benchmark/`, `llms.txt` and `sitemap.xml` are built from this repo's `README.md` Quick start, `docs/`, `LICENSE`, `PRIVACY.md` and `SECURITY.md`, and keyline-bench's `versus-browser/README.md`, by `npm run docs` there: rerun it and commit the output whenever any of those change. Hosted on Cloudflare Pages from `main`.
- `keyline-dev/keyline-bench`: every benchmark run (prompt, events, renders, summary) and the benchmark READMEs, checked out at `../keyline-bench`; the harness stays here (`tests/llm_e2e.rs`, `tests/versus_browser/`), which writes runs there. The flyer's photo, an input, is in `tests/fixtures/photos/`.
- `keyline-dev/homebrew-tap`: the Homebrew formula, bumped by the release workflow; edit it by hand only to change more than the version and checksum.

Listings, and how each stays current (a listing marked *submitted* waits on its directory; once it's live, change the mark to *live*):

| Listing | How it got there | Updates |
|---|---|---|
| Official MCP registry (`io.github.keyline-dev/keyline`), *live* | `server.json` | The release workflow, each release |
| PulseMCP, *live* | Reads the official registry | Automatic |
| Glama, *live* | Claimed through `glama.json` | Reads the repo |
| GitHub MCP registry (github.com/mcp), which VS Code's MCP gallery reads; *not listed* | GitHub curates it from the official registry; there's no submission | Automatic once picked |
| awesome-mcp-servers, *submitted* | Pull request to `punkpeye/awesome-mcp-servers` | Never: one line, no version |
| Docker MCP Catalog, *submitted* | Pull request to `docker/mcp-registry` (#5432, from `yuvalt/mcp-registry`) adding `servers/keyline/` (`server.yaml` pins a commit and uses our `ghcr.io/keyline-dev/keyline-mcp` image; `tools.json` lists the tools) | Docker's bot bumps the pinned commit; refresh `tools.json` when a tool is added or renamed |
| Anthropic's directory (claude.ai/directory), one listing across Claude's apps, *submitted*; it no longer takes `.mcpb` extensions | The developer portal at claude.ai/directory/manage: a plugin bundle, path `plugin`, branch `main`; listing fields (icon, URLs) from `plugin/.claude-plugin/` | Scans each commit on `main` (push webhook); auto-publish is off, so a version goes live only on Publish. Once approved, track the `v0` tag instead (moved only after a release's files are up, which `launch.sh` downloads) and turn auto-publish on |
| Cline MCP Marketplace, *submitted* | An issue on `cline/mcp-marketplace` (#2761): the repo URL, the 400×400 logo (keyline.dev's `logo/app-icon-400.png`), and a check that Cline installs it from the README | Installs from the README |
| Cursor directory, Smithery, mcp.so, mcpservers.org, mcpmarket.com, LobeHub | Each site's submit form (Smithery: a stdio listing with no hosting) | Read the repo |

Install channels kept by the release workflow: GitHub releases, the plugin marketplace in this repo, the Homebrew tap, the Docker image on ghcr.io and the action's `v0` tag. Contributions come under the agreement in `CONTRIBUTING.md`.

## Language

Rust only. Stable toolchain, latest edition. Packaging and client setup files sit beside the Rust code and are the exception: JSON manifests, workflow YAML, and the plugin's POSIX shell launcher (`plugin/scripts/launch.sh`).

## License

The project is source-available under the Functional Source License 1.1 with the Apache 2.0 future license, FSL-1.1-ALv2 (`LICENSE`): anyone may use, modify and redistribute it, commercially too, for any purpose except a competing use (making it available to others in a commercial product or service that substitutes for it or offers substantially the same functionality; a hosted version counts), and each release also becomes Apache-2.0 two years after it ships. Keep it that way:

- Only add dependencies and assets under permissive licenses (MIT, Apache-2.0, BSD, OFL, …); no GPL/AGPL/copyleft, which would conflict with the license and with its Apache-2.0 future.
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
- Schemas every client accepts: Gemini (and other clients that read schemas as OpenAPI) rejects a whole tool list over one unsupported construct. A `type` is one string, never a list (`["string", "null"]`, what schemars writes for `Option<T>`): an optional argument is simply left out of `required`. `items` is a schema, never `true` (what `Vec<Value>` writes). `src/server/schema.rs` rewrites both before `tools/list` returns them, and a test over the real tool list keeps it so; check a new argument's schema against it.

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
