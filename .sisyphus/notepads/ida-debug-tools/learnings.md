# Learnings — ida-debug-tools

## [2026-03-06] Session Start
Plan: IDA MCP 动态调试工具 — IDAPython Bridge 实现
Goal: 23个 dbg_* MCP工具, 通过run_script IDAPython bridge实现
## [Task 1] Debug Handler Infrastructure
- parse_debug_output: extracts last non-empty JSON line from stdout
- build_script: concatenates HEADLESS_PREAMBLE + body
- run_debug_script: Router→route_or_err, Worker→run_script+parse
- HEADLESS_PREAMBLE: Python preamble with safe_hex, make_result, WFNE/DSTATE constants
