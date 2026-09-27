# Reference-ad benchmark

Claude Code (headless, on a Claude subscription) builds the reference ad in three sizes (portrait 1080×1350, wide 1200×1000 at scale 0.85, sky 300×600 at scale 0.28) through the MCP server. Each run is kept so changes can be compared on cost and result.

Run one (from the repo root):

```sh
KEYLINE_MCP_BENCH=<label> cargo test --test llm_e2e -- --ignored --nocapture
```

Each `<label>/` holds `prompt.md` (system prompt and task), `events.jsonl` (every turn and tool call), `layout.txt` (problems, then every layer's box per size), a PNG per size and `summary.json`. `results.tsv` has one row per run: cost (API-equivalent), turns, tool calls, tokens, smallest text per size and defects left. The commit column ends in `+dirty` when `src/` had uncommitted changes.

Runs vary a lot with how long the model thinks, so compare several runs per change, not one.

Prompt versions: `baseline-*` used v1, which said "Every size must look right with no per-size edits". From `v2-*` on, that clause is gone, so the model may adapt a size with `at`. `v2-before-at-*` ran the v2 prompt on the commit before `at` existed, for a fair comparison.
