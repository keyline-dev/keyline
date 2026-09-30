//! What a run cost, read from Claude Code's stream-json events.

use std::collections::{BTreeMap, HashSet};

use serde_json::{Value, json};

/// The events of one `claude -p --output-format stream-json` run.
pub fn parse(stdout: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn n(v: &Value) -> u64 {
    v.as_u64().unwrap_or_default()
}

/// A message's whole input: fresh, cache-written and cache-read tokens.
fn input_of(u: &Value) -> u64 {
    n(&u["input_tokens"]) + n(&u["cache_creation_input_tokens"]) + n(&u["cache_read_input_tokens"])
}

/// The run's measures as `summary.json` keeps them. `result` is `None` when
/// the run was killed before Claude Code reported one.
pub fn summarize(events: &[Value], browser: bool) -> Value {
    let result = events.iter().find(|e| e["type"] == "result");
    let init = events
        .iter()
        .find(|e| e["type"] == "system" && e["subtype"] == "init");

    // Token counts over every model, Haiku side calls included. Thinking is
    // part of output tokens, so it is reported but not added again.
    let (mut input, mut cache_write, mut cache_read, mut output) = (0, 0, 0, 0);
    let mut model = String::new();
    if let Some(usage) = result.and_then(|r| r["modelUsage"].as_object()) {
        for (name, m) in usage {
            input += n(&m["inputTokens"]);
            cache_write += n(&m["cacheCreationInputTokens"]);
            cache_read += n(&m["cacheReadInputTokens"]);
            output += n(&m["outputTokens"]);
            if !name.contains("haiku") {
                model.clone_from(name);
            }
        }
    }
    let usage = result.map(|r| &r["usage"]);
    let thinking = usage.map_or(0, |u| n(&u["output_tokens_details"]["thinking_tokens"]));

    // Per request: the first one's input is the fixed overhead (system
    // prompt plus tool definitions), the largest is the peak context.
    let mut seen = HashSet::new();
    let (mut fixed, mut peak) = (None, 0);
    for e in events.iter().filter(|e| e["type"] == "assistant") {
        let m = &e["message"];
        if e["parent_tool_use_id"].is_string() || !seen.insert(m["id"].to_string()) {
            continue;
        }
        let i = input_of(&m["usage"]);
        fixed.get_or_insert(i);
        peak = peak.max(i);
    }

    let mut calls: BTreeMap<String, usize> = BTreeMap::new();
    let (mut images, mut js) = (0, false);
    for e in events
        .iter()
        .filter(|e| e["type"] == "assistant" || e["type"] == "user")
    {
        for b in e["message"]["content"].as_array().into_iter().flatten() {
            match b["type"].as_str() {
                Some("tool_use") => {
                    let name = b["name"].as_str().unwrap_or_default();
                    let short = name.rsplit("__").next().unwrap_or(name).to_owned();
                    *calls.entry(short).or_default() += 1;
                    js |= browser && measures_with_js(name, &b["input"]);
                }
                Some("tool_result") => {
                    if let Value::Array(parts) = &b["content"] {
                        images += parts.iter().filter(|p| p["type"] == "image").count();
                    }
                }
                _ => {}
            }
        }
    }

    let cost = result.map_or(0.0, |r| r["total_cost_usd"].as_f64().unwrap_or_default());
    json!({
        "claudeVersion": init.map(|i| i["claude_code_version"].clone()),
        "model": model,
        "outcome": result.map_or("killed", |r| r["subtype"].as_str().unwrap_or("?")),
        "totalTokens": input + cache_write + cache_read + output,
        "input": input, "cacheWrite": cache_write, "cacheRead": cache_read,
        "output": output, "thinking": thinking,
        "fixedOverhead": fixed.unwrap_or_default(), "peakContext": peak,
        "costUsd": cost,
        "coldCostUsd": cold_cost(cost, input, cache_write, cache_read, output, usage),
        "turns": result.map(|r| r["num_turns"].clone()),
        "calls": calls.values().sum::<usize>(), "callsByTool": calls,
        "imagesSeen": images, "jsMeasure": js,
        "secs": result.map(|r| r["duration_ms"].as_f64().unwrap_or_default() / 1000.0),
    })
}

/// Whether a tool call measures the page with JavaScript: Playwright MCP's
/// evaluate or run-code tools, or a Bash call that runs a script with Node
/// or npx (a Playwright script can read layout boxes).
fn measures_with_js(tool: &str, input: &Value) -> bool {
    if tool.ends_with("browser_evaluate") || tool.ends_with("browser_run_code_unsafe") {
        return true;
    }
    tool == "Bash"
        && input["command"].as_str().is_some_and(|c| {
            c.split(|ch: char| ch.is_whitespace() || ch == ';' || ch == '&' || ch == '|')
                .any(|w| w == "node" || w == "npx")
        })
}

/// The cost as if nothing had been cached: every input token at the base
/// input price. Anthropic's list prices put a cache read at 0.1× the input
/// price, a cache write at 1.25× (5-minute) or 2× (1-hour) and output at 5×;
/// the run's own cost fixes the base price.
fn cold_cost(
    cost: f64,
    input: u64,
    cache_write: u64,
    cache_read: u64,
    output: u64,
    usage: Option<&Value>,
) -> f64 {
    let (m5, h1) = usage.map_or((0, 0), |u| {
        let c = &u["cache_creation"];
        (
            n(&c["ephemeral_5m_input_tokens"]),
            n(&c["ephemeral_1h_input_tokens"]),
        )
    });
    #[expect(
        clippy::cast_precision_loss,
        reason = "token counts are far below 2^52"
    )]
    let f = |v: u64| v as f64;
    let write = if m5 + h1 == 0 {
        2.0
    } else {
        (1.25 * f(m5) + 2.0 * f(h1)) / f(m5 + h1)
    };
    let billed = f(input) + write * f(cache_write) + 0.1 * f(cache_read) + 5.0 * f(output);
    let cold = f(input + cache_write + cache_read) + 5.0 * f(output);
    if billed == 0.0 {
        0.0
    } else {
        cost * cold / billed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_measurement_is_spotted() {
        let bash = |c: &str| measures_with_js("Bash", &json!({"command": c}));
        assert!(bash("node measure.mjs"));
        assert!(bash("cd /w && node -e 'x'"));
        assert!(!bash("cat measure.js"));
        assert!(!bash(
            "playwright screenshot --viewport-size \"300,600\" file:///w/a.html a.png"
        ));
        assert!(measures_with_js(
            "mcp__playwright__browser_evaluate",
            &json!({})
        ));
    }

    #[test]
    fn cold_cost_undoes_caching() {
        // All 1-hour cache writes: 100 written at 2×, 1000 read at 0.1×.
        let usage = json!({"cache_creation": {"ephemeral_1h_input_tokens": 100}});
        let cost = cold_cost(1.0, 0, 100, 1000, 0, Some(&usage));
        assert!((cost - 1100.0 / 300.0).abs() < 1e-9);
    }
}
