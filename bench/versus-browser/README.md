# keyline vs. a headless-browser agent

The same model, in Claude Code (headless, on a Claude subscription), makes the same images three ways, and every run is kept here: prompt, event log, final PNGs, the agent's own files, a summary and the blind judge's verdict.

| Arm | Tools the agent has | How it sees its work |
|---|---|---|
| `keyline` | the keyline MCP server only | keyline's measured replies (problems per size); renders on request |
| `browser-cli` | Bash, Write, Edit, Read; Playwright's screenshot CLI on PATH | opens its PNGs with Read |
| `browser-mcp` | Playwright MCP (`--headless --isolated --browser chromium`), Write, Edit, Read; the folder served over HTTP | Playwright MCP's screenshots and snapshots, or Read |

## Method

- **One prompt for every arm** (version `vs1`, in `tests/versus_browser/prompts.rs`): the same system prompt ("You make images. Work autonomously; don't ask questions."), the same task text without keyline vocabulary, and one tools paragraph that differs per arm. Each run's `prompt.md` has the exact text.
- **The same bytes everywhere:** the photo, icons, logo and portrait come from the test code; the browser arms also get `Inter.ttf` (the font keyline bundles), so Chrome doesn't fall back to Times. The keyline arm's scene gets the sizes and assets before the agent starts, since uploading isn't what's tested.
- **Two tasks, reported separately:** `reference-ad` (the vote-by-mail flyer from `tests/llm_e2e.rs`, at 1080×1350, 1200×1000 at 0.85× and 300×600 at 0.28×) and `speaker-card` (a conference speaker card at 1080×1080, 1920×1080 and 1080×1920, written and committed before any run).
- **Same limits:** `--max-turns 60`, `--max-budget-usd 8`, a 30-minute timeout, `--setting-sources ""`, `--no-session-persistence`, `--strict-mcp-config`, one pinned model and Claude Code build, one machine.
- **Every run counts.** A run that stops early, hits a cap or makes wrong files counts as incorrect, and its tokens count. Only infrastructure failures (a Chrome crash, an API error, a harness bug) are rerun, and they're listed below.
- **Interleaved:** each block is one run of every arm on every task, in rotating order, so drift over the day hits every arm alike.

### What's measured

From Claude Code's `result` event (`modelUsage`, so Haiku side calls count too):

- **Total tokens:** input + cache writes + cache reads + output, over every model. Caching doesn't change it. Thinking is part of output and is shown on its own, not added twice.
- **Cost:** Claude Code's API-equivalent `total_cost_usd`, and a *cold* cost re-priced as if nothing had been cached (list-price ratios: cache read 0.1×, cache write 1.25× or 2×, output 5× the input price). Neither is what a subscription costs.
- **Turns, tool calls by name, wall-clock** (`duration_ms`, setup excluded), **images the model saw** (image blocks in tool results, Read included), **fixed overhead** (the first request's input: system prompt plus tool definitions), **peak context** (the largest single request), and for the browser arms whether they measured with JavaScript (Playwright MCP's evaluate or run-code tools, or Node run from Bash).

### Correctness, the same for every arm, from the final PNGs only

1. **Files:** all three PNGs at exactly the right pixel size, or the run fails. The keyline arm's PNGs are rendered by the harness from the scene the agent left; the browser arms' are the files the agent saved.
2. **Blind judge:** a separate `claude -p` call (Read only, prompt in `judge-prompt.md`, model `claude-opus-5`) sees each run's PNGs under random names and fills in a checklist per size: each content item present and readable, items cut off or overflowing, items overlapping, smallest text legible, and the task's own checks (photo distorted, bands full width, columns even; or the portrait a true circle). It never learns the arm, and its tokens aren't counted. **A run is correct** when, at every size, every item is present and none is cut off or overflowing.
3. **Owner's blind review:** `export_review` writes anonymised PNG sets to `review/<task>/` and a `review.tsv` to fill in (pass 1 or 0, look 1–5); `judge_runs` then prints how often owner and judge agree.
4. **Likeness** (reference ad only, secondary, never claimed): 0–255, lower is closer, against two references, the reference ad built in keyline (`build_reference_ad`) and the same by hand in HTML (`reference-ad/reference.html`) rendered by Chrome, since either alone favours its own renderer. It catches blank or garbage output.

## Reproduce

Pinned tooling (Playwright 1.63.0 and `@playwright/mcp` 0.0.83, bench-only, never a keyline dependency):

```sh
cd bench/versus-browser/tooling
npm ci
npx playwright install chromium-headless-shell
node node_modules/@playwright/mcp/node_modules/playwright/cli.js install chromium chromium-headless-shell
```

One run (labels are never reused; a run's folder is `<task>/<arm>-<label>/`):

```sh
CLAUDE_BIN=<claude> KEYLINE_MCP_TEST_MODEL=<model> \
KEYLINE_BENCH_ARM=keyline|browser-cli|browser-mcp KEYLINE_BENCH_TASK=reference-ad|speaker-card \
KEYLINE_MCP_BENCH=<label> \
  cargo test --release --test versus_browser claude_makes_the_images -- --ignored --nocapture
```

Then judge the unjudged runs, rewrite `results.tsv` and print the medians:

```sh
cargo test --release --test versus_browser judge_runs -- --ignored --nocapture
```

## Prompt version vs2

The same protocol (prompts, tasks, judge, tooling unchanged; only the version label) rerun on keyline `43d4acd`. The `commit` column says `f386138`, the commit that bumped the label; its `src/` is identical to `43d4acd`.

**In progress:** blocks 1–4 ran (24 runs, all judged); the runner stopped before block 5 because another change landed in the working tree's `src/` and rebuilt the binary. All 24 runs used the binary built from `43d4acd` at 14:06 (the next release build was at 15:27, after block 4); `reference-ad/*-v2-4` say `+dirty` only because `src/` changed while they were being saved. No results table or claim until all five blocks have run.

## Prompt version vs1 (stopped, superseded by vs2)

**vs1 stopped after 29 runs so keyline changes could land; a full rerun follows as vs2.** Blocks 1–4 ran in full, and block 5 got five of its six runs (`speaker-card/keyline-5` never ran). Every run that ran is kept here and in `results.tsv`, and judged. vs1 numbers are not compared with vs2's.

- **Commit:** keyline at `d098e31`; blocks 3–5 ran after `46fda5b`, which changed one unit-test assertion only (the shipped binary is identical). The `commit` column says which.
- **Model and client:** `claude-opus-5[1m]` in Claude Code 2.1.251, pinned with `KEYLINE_MCP_TEST_MODEL`, the same build as the `bench/reference-ad/` runs. The judge used `claude-opus-5`.
- **Browsers:** the CLI arm's Playwright 1.63.0 uses Chrome Headless Shell 153; `@playwright/mcp` 0.0.83 brings its own Playwright (1.64 alpha) and Chromium 155.
- **Machine:** one Apple-silicon Mac, macOS 27, runs one at a time, on 2026-09-30.
- **Pilot:** one run per arm and task (`*-pilot1`) before the counted runs, not counted. After it, only the harness changed: event logs drop base64 image bytes, since the PNGs are kept beside them.
- **Infrastructure reruns:** none.
