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
- **Turns** (Claude Code's `num_turns`, which is the tool calls plus one, not the model requests: one request can make several tool calls, and browser agents often do), **tool calls by name, wall-clock** (`duration_ms`, setup excluded), **images the model saw** (image blocks in tool results, Read included), **fixed overhead** (the first request's input: system prompt plus tool definitions), **peak context** (the largest single request), and for the browser arms whether they measured with JavaScript (Playwright MCP's evaluate or run-code tools, or Node run from Bash).

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

## Prompt version vs3

The same protocol on keyline 0.7.0 (`9e4a66c`). One change to the task, made before any counted run: the flyer's photo is a real one, `bench/photos/farmhouse.jpg` (a farmhouse in a field at sunrise), for every arm. Speaker card, judge prompt and tooling unchanged. Pilots (`*-pilot-v3`, one per arm on the flyer) ran on `80d053e` and aren't counted.

30 counted runs, 5 per arm and task in five interleaved blocks, on 2026-10-03, 10:18–12:40. Two earlier attempts were stopped and none of their runs counted: the first after its first block, so the Gemini schema fix could ship as 0.7.0, and the second because the machine slept overnight and its runs couldn't reach the API (one turn, no tokens). The runner now stops at a run that uses no tokens. **Infrastructure reruns:** one, `speaker-card/browser-mcp-v3-3`, whose first attempt ended on an API connection reset (`ECONNRESET`) after 9 turns without its PNGs.

### Results

Medians (min–max) over all 30 runs. Ratios are browser ÷ keyline: the ratio of the medians, then the range between the extremes.

#### reference-ad

| Arm | Correct | Total tokens | Cost | Cold cost | Turns | Time (s) | Images seen | Fixed overhead | Peak context | Measured with JS |
|---|---|---|---|---|---|---|---|---|---|---|
| keyline | 5/5 | 147k (93k–163k) | $0.47 ($0.37–0.57) | $0.90 | 10 (8–11) | 120 (88–320) | 3 (2–3) | 5k | 22k | – |
| browser-cli | 5/5 | 372k (261k–765k) | $0.91 ($0.73–1.37) | $2.16 | 23 (15–38) | 227 (195–322) | 8 (5–10) | 4k | 35k | 5/5 |
| browser-mcp | 5/5 | 917k (361k–1,259k) | $1.08 ($0.95–1.41) | $4.86 | 37 (31–44) | 216 (180–265) | 5 (4–9) | 9k | 40k | 5/5 |

| Browser ÷ keyline | Total tokens | Cost |
|---|---|---|
| browser-cli | 2.5× (1.6–8.2×) | 1.9× (1.3–3.7×) |
| browser-mcp | 6.2× (2.2–13.5×) | 2.3× (1.7–3.8×) |

#### speaker-card

| Arm | Correct | Total tokens | Cost | Cold cost | Turns | Time (s) | Images seen | Fixed overhead | Peak context | Measured with JS |
|---|---|---|---|---|---|---|---|---|---|---|
| keyline | 5/5 | 144k (90k–195k) | $0.42 ($0.40–0.58) | $0.86 | 10 (7–13) | 103 (91–144) | 4 (2–4) | 5k | 21k | – |
| browser-cli | 4/5 | 505k (387k–1,054k) | $1.14 ($0.87–1.71) | $2.87 | 30 (25–31) | 265 (191–327) | 9 (7–13) | 4k | 47k | 5/5 |
| browser-mcp | 5/5 | 1,053k (448k–2,840k) | $1.23 ($0.65–2.68) | $5.53 | 39 (23–61) | 213 (133–447) | 7 (5–8) | 9k | 45k | 5/5 |

| Browser ÷ keyline | Total tokens | Cost |
|---|---|---|
| browser-cli | 3.5× (2.0–11.8×) | 2.7× (1.5–4.3×) |
| browser-mcp | 7.3× (2.3–31.7×) | 2.9× (1.1–6.8×) |

Every run is in `results.tsv`, and `judge_runs` prints the medians over correct runs only (speaker-card browser-cli: 647k over its 4).

### Reading it

- **The stronger browser arm is browser-cli,** with the lower median total tokens on both tasks, so it's the comparison.
- **Both tasks:** keyline used fewer tokens (2.5× and 3.5×) and was correct at least as often (5/5 and 5/5, against 5/5 and 4/5). The browser run that failed, `speaker-card/browser-cli-v3-4`, left the "Get tickets" button cut off at the square size.
- **The ranges don't overlap:** keyline's most expensive run used fewer tokens than any browser run, on both tasks (163k against 261k, and 195k against 387k).
- **Elsewhere:** keyline was faster on median time (120 s against 227 s, and 103 s against 265 s), made fewer model requests and looked at fewer images. Model requests (distinct assistant messages in `events.jsonl`, median): flyer 9, 17 and 35; speaker card 10, 22 and 38 (keyline, browser-cli, browser-mcp). The Turns column counts tool calls plus one, so it reads higher for the browser arms, which often make several tool calls per request. No keyline flyer run opened the photo or marked a subject; the default crop kept the farmhouse whole at every size.
- **From the turns:** in `speaker-card/keyline-v3-4` the model twice wrote a `layer_add` that wasn't valid JSON, which Claude Code refused before keyline saw it; in `speaker-card/keyline-v3-1` keyline refused gradient stops written as `[color, offset]`.

### What the rules allow

keyline won on both tasks with correctness at least as high, so a general claim is allowed, from the smaller of the two ratios, rounded down to one significant figure:

> keyline used 2× fewer tokens than a headless-browser agent (median of 5 runs on each of two tasks, Claude Opus 5, October 2026).

And per task: 2× on the flyer, 3× on the speaker card. Against browser-mcp alone, by the same rule (keyline won on both tasks, correct as often): 6× fewer tokens, 6× on the flyer and 7× on the speaker card.

## Prompt version vs2

The same protocol (prompts, tasks, judge, tooling unchanged; only the version label) rerun on keyline `43d4acd`. The `commit` column says `f386138`, the commit that bumped the label; its `src/` is identical to `43d4acd`.

30 counted runs, 5 per arm and task in five interleaved blocks, on 2026-10-01. Blocks 1–4 ran 14:07–15:28; the runner then stopped because uncommitted changes appeared in the working tree's `src/`, and block 5 ran 15:54–16:14 once `src/` was identical to `43d4acd` again and the binary was rebuilt from it. Blocks 1–4 used the 14:06 build from `43d4acd`, block 5 a 15:54 rebuild of the same source (the commit column says `5447e97`, a commit that only added runs). `reference-ad/*-v2-4` say `+dirty` only because `src/` changed while they were being saved. Every run ended normally; there were no timeouts and no infrastructure reruns.

### Results

Medians (min–max) over all 30 runs. Ratios are browser ÷ keyline: the ratio of the medians, then the range between the extremes.

#### reference-ad

| Arm | Correct | Total tokens | Cost | Cold cost | Turns | Time (s) | Images seen | Fixed overhead | Peak context | Measured with JS |
|---|---|---|---|---|---|---|---|---|---|---|
| keyline | 4/5 | 169k (115k–185k) | $0.47 ($0.37–0.71) | $1.01 | 10 (9–12) | 128 (102–288) | 1 (1–4) | 5.2k | 19k | – |
| browser-cli | 5/5 | 308k (172k–634k) | $0.83 ($0.57–1.23) | $1.83 | 19 (14–32) | 225 (158–317) | 8 (6–8) | 3.6k | 34k | 5/5 |
| browser-mcp | 5/5 | 1,056k (618k–1,485k) | $1.17 ($0.80–1.56) | $5.55 | 43 (29–53) | 225 (175–274) | 6 (4–8) | 9.3k | 38k | 5/5 |

| Browser ÷ keyline | Total tokens | Cost |
|---|---|---|
| browser-cli | 1.8× (0.9–5.5×) | 1.8× (0.8–3.3×) |
| browser-mcp | 6.3× (3.3–12.9×) | 2.5× (1.1–4.2×) |

#### speaker-card

| Arm | Correct | Total tokens | Cost | Cold cost | Turns | Time (s) | Images seen | Fixed overhead | Peak context | Measured with JS |
|---|---|---|---|---|---|---|---|---|---|---|
| keyline | 5/5 | 199k (93k–290k) | $0.51 ($0.31–0.69) | $1.18 | 13 (8–16) | 121 (92–187) | 4 (2–6) | 5.2k | 21k | – |
| browser-cli | 5/5 | 536k (237k–574k) | $1.03 ($0.61–1.19) | $3.02 | 24 (17–28) | 222 (141–311) | 7 (5–8) | 3.6k | 39k | 5/5 |
| browser-mcp | 5/5 | 1,087k (571k–1,824k) | $1.25 ($1.14–1.82) | $5.69 | 39 (26–56) | 255 (176–270) | 8 (7–11) | 9.3k | 47k | 5/5 |

| Browser ÷ keyline | Total tokens | Cost |
|---|---|---|
| browser-cli | 2.7× (0.8–6.1×) | 2.0× (0.9–3.8×) |
| browser-mcp | 5.5× (2.0–19.5×) | 2.4× (1.7–5.8×) |

Every run is in `results.tsv`, likeness scores included, and the medians over correct runs only are printed by `judge_runs`. On reference-ad they are 170k tokens for keyline's 4 correct runs against 308k for browser-cli.

### Reading it

- **The stronger browser arm is browser-cli,** with the lower median total tokens on both tasks, so it's the comparison. browser-mcp's tool definitions add 9.3k tokens to every request, and it took about twice as many turns.
- **reference-ad:** keyline used fewer tokens (1.8×) but was correct less often (4/5 against 5/5). The run that failed, `keyline-v2-1`, left the wide size's photo band cropped to 53% of its height, and the judge marked the photo cut off. `keyline-v2-5` has the same 53% crop and keyline's `warn crop` too, and the judge passed it. The agent acted on neither warning.
- **speaker-card:** keyline used 2.7× fewer tokens, and all three arms were correct 5/5.
- **The ranges overlap:** browser-cli's cheapest run on each task used fewer tokens than keyline's most expensive one.
- **Elsewhere:** keyline was faster on median time on both tasks (128 s against 225 s, and 121 s against 222 s) and looked at fewer images. Every browser run measured its page with JavaScript.

### What the rules allow

reference-ad allows no token claim, because the browser arm was correct more often. speaker-card allows one by name, rounded down to one significant figure:

> On a speaker-card task, keyline used 2× fewer tokens than a headless-browser agent (median of 5 runs, Claude Opus 5, October 2026).

There is no general "X× fewer tokens" claim, since that would need keyline to win on both tasks with correctness at least as high.

### Owner's blind review

`export_review` wrote 15 anonymised sets per task to `review/<task>/` (the PNGs are gitignored copies). Fill in `review.tsv`, without opening `key.tsv`, then run `judge_runs`, which prints how often owner and judge agree.

## Prompt version vs1 (stopped, superseded by vs2)

**vs1 stopped after 29 runs so keyline changes could land; a full rerun follows as vs2.** Blocks 1–4 ran in full, and block 5 got five of its six runs (`speaker-card/keyline-5` never ran). Every run that ran is kept here and in `results.tsv`, and judged. vs1 numbers are not compared with vs2's.

- **Commit:** keyline at `d098e31`; blocks 3–5 ran after `46fda5b`, which changed one unit-test assertion only (the shipped binary is identical). The `commit` column says which.
- **Model and client:** `claude-opus-5[1m]` in Claude Code 2.1.251, pinned with `KEYLINE_MCP_TEST_MODEL`, the same build as the `bench/reference-ad/` runs. The judge used `claude-opus-5`.
- **Browsers:** the CLI arm's Playwright 1.63.0 uses Chrome Headless Shell 153; `@playwright/mcp` 0.0.83 brings its own Playwright (1.64 alpha) and Chromium 155.
- **Machine:** one Apple-silicon Mac, macOS 27, runs one at a time, on 2026-09-30.
- **Pilot:** one run per arm and task (`*-pilot1`) before the counted runs, not counted. After it, only the harness changed: event logs drop base64 image bytes, since the PNGs are kept beside them.
- **Infrastructure reruns:** none.
