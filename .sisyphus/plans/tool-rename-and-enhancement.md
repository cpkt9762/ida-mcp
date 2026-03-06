# IDA MCP Tool Rename, Enhancement & Skill Update

## TL;DR

> **Quick Summary**: 重构 ida-mcp-rs 的 69 个工具命名为 verb_object 风格以提升 LLM 工具选择准确率，新增 7 个缺失工具，合并 3 个字符串工具为 1 个，全面更新 skill 知识文件。旧名通过 aliases 机制 100% 向后兼容。
> 
> **Deliverables**:
> - 69 个工具重命名（主名更新，旧名保留为 alias）
> - 7 个新工具实现
> - 3 个字符串工具合并为 `list_strings`
> - `ida-pro/SKILL.md` 全面更新（MCP 工具速查集中于此）
> - `solana-sbpf-reverse/SKILL.md` + `swap-pipeline.md` 工具名更新
> - `docs/TOOLS.md` 自动重新生成
> 
> **Estimated Effort**: Large
> **Parallel Execution**: YES - 5 waves
> **Critical Path**: Task 1 → Task 2 → Task 3 → Task 4 → Task 5 → Task 8 → Task 10 → F1-F4

---

## Context

### Original Request
对比 ida-pro-mcp (Python 插件) 发现我们的工具命名对 LLM 不够友好（裸名词无动词、过度缩写、泛化歧义），且缺少部分常用工具。用户要求：
1. 全面重命名为 verb_object 风格
2. 新增 7 个工具弥补功能差距
3. 合并 `strings`/`find_string`/`analyze_strings` 为一个 `list_strings`
4. 更新所有引用工具名的 skill 文件，MCP 工具速查集中在 `ida-pro/SKILL.md`

### Interview Summary
**Key Discussions**:
- 命名原则: verb_object, 不缩写, 读写分离(read/list vs set/rename/declare), 旧名保留为 alias
- 新增工具范围: 7 个（set_function_prototype, rename_stack_variable, set_stack_variable_type, get_function_prototype, batch_rename, list_enums/create_enum, set_function_comment）
- 字符串工具合并: 3→1, 统一为 `list_strings(query, filter, offset, limit)`
- Skill 更新策略: MCP 工具集中在 ida-pro skill，solana-sbpf-reverse 只更新工具名不重复工具文档

### Research Findings
- `tool_registry.rs`: 69 个 ToolInfo，有 name/category/short_desc/full_desc/example/default/keywords 字段
- `rpc_dispatch.rs`: 2804 行，match tool name string → 解析参数 → 调用 WorkerDispatch trait 方法
- `server/mod.rs`: MCP tools/list handler，tool name → JSON schema 映射
- `worker_trait.rs`: 408 行 WorkerDispatch trait，每个 worker 方法对应一个工具
- `ida-pro/SKILL.md`: 587 行，Part 2 有完整工具速查，100+ 处工具名引用
- `solana-sbpf-reverse/SKILL.md` + `swap-pipeline.md`: 84 + 50+ 处工具名引用

---

## Work Objectives

### Core Objective
提升 LLM 对 ida-mcp-rs 工具的选择准确率，通过统一的 verb_object 命名、补全缺失工具、简化重复工具，并确保 skill 知识与工具集同步。

### Concrete Deliverables
- `src/tool_registry.rs` — ToolInfo 结构体增加 `aliases` 字段，所有 69 个工具更新主名
- `src/rpc_dispatch.rs` — dispatch 匹配支持 aliases
- `src/server/mod.rs` — MCP tools/list 暴露新主名，JSON schema 支持新工具
- `src/ida/worker_trait.rs` — 新工具的 trait 方法
- `src/ida/worker.rs` — 新工具的实现
- `src/ida/handlers/` — 新工具的 FFI handler（annotations.rs, types.rs, functions.rs 等）
- `~/.config/opencode/skills/ida-pro/SKILL.md` — 全面更新
- `~/.config/opencode/skills/solana-sbpf-reverse/SKILL.md` — 工具名更新
- `~/.config/opencode/skills/solana-sbpf-reverse/references/swap-pipeline.md` — 工具名更新
- `docs/TOOLS.md` — 自动重新生成

### Definition of Done
- [ ] `cargo build` 无错误
- [ ] `cargo test` 全部通过
- [ ] 所有旧工具名通过 alias dispatch 仍然可用
- [ ] MCP tools/list 暴露新主名
- [ ] 7 个新工具可调用并返回正确结果
- [ ] `strings`/`find_string`/`analyze_strings` 合并为 `list_strings`，旧名均为 alias
- [ ] skill 文件中无任何旧工具名引用（除 alias 说明外）

### Must Have
- 100% 向后兼容：所有旧名作为 alias 永久可用
- verb_object 命名一致性
- 新工具 7 个全部实现
- Skill 文件同步更新

### Must NOT Have (Guardrails)
- **不改内部 Rust 方法名** — 只改面向用户的 tool name 字符串，worker trait 方法名保持不变
- **不改 JSON 参数名** — 参数 schema 不变，只改工具名
- **不删除旧工具入口** — 旧名通过 alias 永远可用
- **不合并非字符串工具** — 只合并 strings/find_string/analyze_strings，其他保持独立
- **不改 open_idb/close_idb/open_sbpf/open_dsc** — Core 数据库工具名已清晰，不改
- **Skill 文件不加 emoji** — 保持技术文档风格

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: YES
- **Automated tests**: Tests-after (cargo test)
- **Framework**: cargo test (Rust built-in)

### QA Policy
Every task MUST include agent-executed QA scenarios.
Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

- **Rust code**: Use Bash (cargo build, cargo test) — Compile, run tests, assert pass
- **Tool dispatch**: Use Bash (cargo test) — Unit tests for alias resolution + new tool dispatch
- **Skill files**: Use Bash (grep) — Verify no stale tool names remain

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Foundation — aliases infrastructure + naming table):
├── Task 1: Add aliases field to ToolInfo + alias lookup functions [quick]
├── Task 2: Create complete naming mapping table (old→new) as const [quick]

Wave 2 (Core rename — registry + dispatch + server):
├── Task 3: Update tool_registry.rs — all 69 entries with new names + aliases [unspecified-high]
├── Task 4: Update rpc_dispatch.rs — match new names + alias fallback [unspecified-high]
├── Task 5: Update server/mod.rs — MCP tools/list + JSON schema for new names [unspecified-high]

Wave 3 (New tools + merge — parallel implementations):
├── Task 6: Implement 4 new tools: get/set_function_prototype, set_function_comment, batch_rename [deep]
├── Task 7: Implement 3 new tools: rename_stack_variable, set_stack_variable_type, list_enums/create_enum [deep]
├── Task 8: Merge strings/find_string/analyze_strings → list_strings [unspecified-high]

Wave 4 (Skill files — parallel updates):
├── Task 9: Update solana-sbpf-reverse SKILL.md + swap-pipeline.md — tool name replacement [quick]
├── Task 10: Rewrite ida-pro/SKILL.md Part 2 (MCP tool reference) with new names + new tools [unspecified-high]
├── Task 11: Update ida-pro/SKILL.md Part 3-5 (workflows, errors, headless) with new names [unspecified-high]

Wave 5 (Verification + docs):
├── Task 12: Update tests + regenerate docs/TOOLS.md [quick]

Wave FINAL (After ALL tasks — independent review, 4 parallel):
├── Task F1: Plan compliance audit (oracle)
├── Task F2: Code quality review (unspecified-high)
├── Task F3: Real manual QA (unspecified-high)
├── Task F4: Scope fidelity check (deep)

Critical Path: Task 1 → Task 3 → Task 4 → Task 5 → Task 8 → Task 10 → Task 12 → F1-F4
Parallel Speedup: ~60% faster than sequential
Max Concurrent: 3 (Waves 2, 3, 4)
```

### Dependency Matrix

| Task | Depends On | Blocks | Wave |
|------|-----------|--------|------|
| 1 | — | 2, 3, 4, 5 | 1 |
| 2 | — | 3, 4 | 1 |
| 3 | 1, 2 | 5, 6, 7, 8, 12 | 2 |
| 4 | 1, 2, 3 | 6, 7, 8 | 2 |
| 5 | 3, 4 | 6, 7, 8, 12 | 2 |
| 6 | 3, 4, 5 | 10, 12 | 3 |
| 7 | 3, 4, 5 | 10, 12 | 3 |
| 8 | 3, 4, 5 | 9, 10, 12 | 3 |
| 9 | 8 | — | 4 |
| 10 | 6, 7, 8 | — | 4 |
| 11 | 3 | — | 4 |
| 12 | 3, 6, 7, 8 | F1-F4 | 5 |

### Agent Dispatch Summary

- **Wave 1**: 2 tasks — T1 `quick`, T2 `quick`
- **Wave 2**: 3 tasks — T3 `unspecified-high`, T4 `unspecified-high`, T5 `unspecified-high`
- **Wave 3**: 3 tasks — T6 `deep`, T7 `deep`, T8 `unspecified-high`
- **Wave 4**: 3 tasks — T9 `quick`, T10 `unspecified-high`, T11 `unspecified-high`
- **Wave 5**: 1 task — T12 `quick`
- **FINAL**: 4 tasks — F1 `oracle`, F2 `unspecified-high`, F3 `unspecified-high`, F4 `deep`

---

## TODOs

> Implementation + Test = ONE Task. Never separate.
> EVERY task MUST have: Recommended Agent Profile + Parallelization info + QA Scenarios.
> **A task WITHOUT QA Scenarios is INCOMPLETE. No exceptions.**

- [ ] 1. Add aliases infrastructure to ToolInfo

  **What to do**:
  - Add `pub aliases: &'static [&'static str]` field to `ToolInfo` struct in `src/tool_registry.rs`
  - Add `get_tool_by_alias(name: &str) -> Option<&'static ToolInfo>` function that checks both `name` and `aliases`
  - Update `get_tool()` to also check aliases as fallback
  - Add `pub fn primary_name_for(name: &str) -> &'static str` that resolves alias → primary name
  - For now set all `aliases: &[]` — actual values filled in Task 3
  - Update the `search_tools()` function to also match against aliases

  **Must NOT do**:
  - Do NOT rename any existing tool names yet (that's Task 3)
  - Do NOT change any other ToolInfo fields

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: [`rust-skills`]
  - **Skills Evaluated but Omitted**:
    - `ida-pro`: Not needed — pure data structure change, no IDA domain knowledge

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Task 2)
  - **Blocks**: Tasks 3, 4, 5
  - **Blocked By**: None

  **References**:
  - `src/tool_registry.rs:127-142` — Current ToolInfo struct definition
  - `src/tool_registry.rs:1003-1006` — Current `get_tool()` function
  - `src/tool_registry.rs:1014-1077` — Current `search_tools()` function

  **Acceptance Criteria**:
  - [ ] `cargo build` succeeds
  - [ ] `cargo test` passes (existing tests unchanged)
  - [ ] `get_tool("disasm")` still returns Some (unchanged behavior)
  - [ ] New function `get_tool_by_alias` compiles and is callable

  **QA Scenarios:**
  ```
  Scenario: ToolInfo struct has aliases field
    Tool: Bash (cargo build)
    Steps:
      1. cargo build 2>&1
      2. Assert exit code 0
    Expected Result: Compiles without errors
    Evidence: .sisyphus/evidence/task-1-build.txt

  Scenario: Existing tests still pass
    Tool: Bash (cargo test)
    Steps:
      1. cargo test 2>&1
      2. Assert exit code 0, all tests pass
    Expected Result: All existing tests pass unchanged
    Evidence: .sisyphus/evidence/task-1-tests.txt
  ```

  **Commit**: YES (groups with Task 2)
  - Message: `refactor(tools): add aliases field to ToolInfo struct`
  - Files: `src/tool_registry.rs`
  - Pre-commit: `cargo test`

- [ ] 2. Create complete old→new naming mapping table

  **What to do**:
  - Create a const mapping table (or documented reference in code comments) in `src/tool_registry.rs` with the complete 69-tool rename mapping. This serves as the single source of truth for Task 3, 4, 5.
  - The mapping (old_name → new_name):
  ```
  // === Core (mostly unchanged) ===
  analysis_status     → get_analysis_status     (alias: analysis_status)
  idb_meta            → get_database_info       (alias: idb_meta)
  task_status         → get_task_status          (alias: task_status)
  // open_idb, open_dsc, open_sbpf, close_idb, dsc_add_dylib, load_debug_info, tool_catalog, tool_help — UNCHANGED
  
  // === Functions ===
  list_functions      → UNCHANGED
  list_funcs          → UNCHANGED (already alias)
  resolve_function    → get_function_by_name     (alias: resolve_function)
  function_at         → get_function_at_address   (alias: function_at)
  lookup_funcs        → batch_lookup_functions    (alias: lookup_funcs)
  analyze_funcs       → run_auto_analysis         (alias: analyze_funcs)
  
  // === Disassembly ===
  disasm              → disassemble              (alias: disasm)
  disasm_by_name      → disassemble_function      (alias: disasm_by_name)
  disasm_function_at  → disassemble_function_at   (alias: disasm_function_at)
  
  // === Decompile ===
  decompile           → decompile_function        (alias: decompile)
  pseudocode_at       → get_pseudocode_at         (alias: pseudocode_at)
  decompile_structured → UNCHANGED
  batch_decompile     → UNCHANGED
  diff_functions      → diff_pseudocode           (alias: diff_functions)
  search_pseudocode   → UNCHANGED
  
  // === Xrefs ===
  xrefs_to            → get_xrefs_to             (alias: xrefs_to)
  xrefs_from          → get_xrefs_from           (alias: xrefs_from)
  xrefs_to_string     → get_xrefs_to_string      (alias: xrefs_to_string)
  xref_matrix         → build_xref_matrix         (alias: xref_matrix)
  xrefs_to_field      → get_xrefs_to_struct_field (alias: xrefs_to_field)
  
  // === Control Flow ===
  basic_blocks        → get_basic_blocks          (alias: basic_blocks)
  callers             → get_callers               (alias: callers)
  callees             → get_callees               (alias: callees)
  callgraph           → build_callgraph           (alias: callgraph)
  find_paths          → find_control_flow_paths   (alias: find_paths)
  
  // === Memory ===
  get_bytes           → read_bytes                (alias: get_bytes)
  get_string          → read_string               (alias: get_string)
  get_u8              → read_byte                 (alias: get_u8)
  get_u16             → read_word                 (alias: get_u16)
  get_u32             → read_dword                (alias: get_u32)
  get_u64             → read_qword               (alias: get_u64)
  get_global_value    → read_global_variable      (alias: get_global_value)
  int_convert         → convert_number            (alias: int_convert)
  table_scan          → scan_memory_table         (alias: table_scan)
  
  // === Search ===
  find_bytes          → search_bytes              (alias: find_bytes)
  search              → search_text               (alias: search)
  strings + find_string + analyze_strings → list_strings (aliases: strings, find_string, analyze_strings)
  find_insns          → search_instructions       (alias: find_insns)
  find_insn_operands  → search_instruction_operands (alias: find_insn_operands)
  
  // === Metadata ===
  segments            → list_segments             (alias: segments)
  addr_info           → get_address_info          (alias: addr_info)
  imports             → list_imports              (alias: imports)
  exports             → list_exports              (alias: exports)
  export_funcs        → export_functions          (alias: export_funcs)
  entrypoints         → list_entry_points         (alias: entrypoints)
  list_globals        → UNCHANGED
  
  // === Types / Structs ===
  local_types         → list_local_types          (alias: local_types)
  declare_type        → declare_c_type            (alias: declare_type)
  apply_types         → apply_type                (alias: apply_types)
  infer_types         → infer_type                (alias: infer_types)
  stack_frame         → get_stack_frame           (alias: stack_frame)
  declare_stack       → create_stack_variable      (alias: declare_stack)
  delete_stack        → delete_stack_variable      (alias: delete_stack)
  structs             → list_structs              (alias: structs)
  struct_info         → get_struct_info           (alias: struct_info)
  read_struct         → read_struct_at_address     (alias: read_struct)
  search_structs      → UNCHANGED
  
  // === Editing ===
  rename              → rename_symbol              (alias: rename)
  rename_lvar         → rename_local_variable      (alias: rename_lvar)
  set_lvar_type       → set_local_variable_type    (alias: set_lvar_type)
  set_comments        → set_comment                (alias: set_comments)
  set_decompiler_comment → UNCHANGED
  patch               → patch_bytes                (alias: patch)
  patch_asm           → patch_assembly             (alias: patch_asm)
  
  // === Scripting ===
  run_script          → UNCHANGED
  ```
  - Total: 49 renames, 20 unchanged, 3 merged into 1

  **Must NOT do**:
  - Do NOT apply the renames yet (that's Task 3)
  - This is a reference document only

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: [`rust-skills`]

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Task 1)
  - **Blocks**: Tasks 3, 4
  - **Blocked By**: None

  **References**:
  - `src/tool_registry.rs:145-991` — All 69 ToolInfo entries
  - The ida-pro-mcp comparison discussion above — naming decisions rationale

  **Acceptance Criteria**:
  - [ ] Mapping table is in code as comments or const
  - [ ] All 69 tools accounted for (49 renamed + 20 unchanged)
  - [ ] 3 merged tools documented (strings → list_strings)

  **QA Scenarios:**
  ```
  Scenario: Mapping table completeness
    Tool: Bash (grep)
    Steps:
      1. Count unique tool names in tool_registry.rs TOOL_REGISTRY array
      2. Count entries in mapping table
      3. Assert both equal 69
    Expected Result: 69 tools mapped
    Evidence: .sisyphus/evidence/task-2-mapping-count.txt
  ```

  **Commit**: YES (groups with Task 1)
  - Message: `docs(tools): add old-to-new naming mapping reference`
  - Files: `src/tool_registry.rs`
  - Pre-commit: `cargo build`

- [ ] 3. Apply all 69 tool renames in tool_registry.rs

  **What to do**:
  - For each of the 49 renamed tools: set `name` to new name, set `aliases` to `&["old_name"]`
  - For each of the 20 unchanged tools: set `aliases: &[]`
  - For `list_funcs` (existing alias): set `aliases: &["list_funcs"]` on the `list_functions` entry, REMOVE the separate `list_funcs` ToolInfo entry
  - For merged string tools: keep ONE entry `list_strings` with `aliases: &["strings", "find_string", "analyze_strings"]`, remove the other 2 ToolInfo entries
  - Update `short_desc` and `full_desc` where the tool name appears in descriptions
  - Update keywords to include both old and new name tokens
  - Update all tests at bottom of file to use new primary names
  - Net tool count: 69 - 1 (list_funcs) - 2 (merged strings) = 66 ToolInfo entries + 7 new (Task 6,7) = 73 final

  **Must NOT do**:
  - Do NOT change `example` JSON parameter names — only tool names in descriptions
  - Do NOT change the `default: bool` settings
  - Do NOT change category assignments

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: [`rust-skills`]

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 2 (sequential after Wave 1)
  - **Blocks**: Tasks 4, 5, 6, 7, 8, 12
  - **Blocked By**: Tasks 1, 2

  **References**:
  - `src/tool_registry.rs:145-991` — All 69 ToolInfo entries to modify
  - `src/tool_registry.rs:1079-1105` — Tests to update
  - Task 2 mapping table — the authoritative rename list

  **Acceptance Criteria**:
  - [ ] `cargo build` succeeds
  - [ ] All 49 renamed tools have new primary names
  - [ ] All 49 renamed tools have old names in aliases
  - [ ] `list_funcs` separate entry removed
  - [ ] `find_string` and `analyze_strings` separate entries removed
  - [ ] `list_strings` entry has 3 aliases
  - [ ] Tests updated and passing

  **QA Scenarios:**
  ```
  Scenario: All renames applied correctly
    Tool: Bash (grep + cargo test)
    Steps:
      1. grep -c 'name: "disasm"' src/tool_registry.rs → expect 0 (renamed)
      2. grep -c 'name: "disassemble"' src/tool_registry.rs → expect 1
      3. grep '"disasm"' src/tool_registry.rs → only in aliases
      4. cargo test -- tool_registry → all tests pass
    Expected Result: Primary names updated, old names only in aliases
    Evidence: .sisyphus/evidence/task-3-renames.txt

  Scenario: String tools merged
    Tool: Bash (grep)
    Steps:
      1. grep -c 'name: "list_strings"' src/tool_registry.rs → expect 1
      2. grep -c 'name: "find_string"' src/tool_registry.rs → expect 0
      3. grep -c 'name: "analyze_strings"' src/tool_registry.rs → expect 0
      4. grep 'strings.*find_string.*analyze_strings' src/tool_registry.rs → in aliases array
    Expected Result: Single list_strings entry with 3 aliases
    Evidence: .sisyphus/evidence/task-3-merge.txt
  ```

  **Commit**: YES
  - Message: `refactor(tools): rename all 69 tools to verb_object convention with backward-compatible aliases`
  - Files: `src/tool_registry.rs`
  - Pre-commit: `cargo test`

- [ ] 4. Update rpc_dispatch.rs for new names + alias fallback

  **What to do**:
  - In `dispatch_rpc_request()` function (~2804 lines), update all match arms from old tool names to new primary names
  - Add alias resolution at the TOP of dispatch: before the main match, call `primary_name_for(method)` to resolve aliases → primary name, then match on primary name
  - This means the main match block uses NEW names only, and alias resolution happens once upfront
  - Update the `record()` calls in the recorder/test infrastructure to use new names
  - Update all test assertions that check tool name strings
  - For merged `list_strings`: route all 3 old dispatch arms (`"strings"`, `"find_string"`, `"analyze_strings"`) to a single handler. The handler should accept the UNION of all 3 tools' parameters (query, filter, offset, limit, exact) with sensible defaults

  **Must NOT do**:
  - Do NOT change WorkerDispatch trait method names (e.g., `disasm()` stays `disasm()` internally)
  - Do NOT change parameter parsing logic — only the match arm labels
  - Do NOT change the JSON response structure

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: [`rust-skills`]

  **Parallelization**:
  - **Can Run In Parallel**: NO (depends on Task 3)
  - **Parallel Group**: Wave 2 (after Task 3)
  - **Blocks**: Tasks 6, 7, 8
  - **Blocked By**: Tasks 1, 2, 3

  **References**:
  - `src/rpc_dispatch.rs:1-50` — Module structure, parse helpers
  - `src/rpc_dispatch.rs:187-224` — Current `disasm` dispatch (example of match arm)
  - `src/rpc_dispatch.rs:266-300` — Current `strings`/`find_string`/`analyze_strings` dispatch (merge target)
  - `src/rpc_dispatch.rs:530-540` — Current `rename` dispatch
  - `src/rpc_dispatch.rs:596-600` — Current `callers` dispatch
  - `src/rpc_dispatch.rs:1270-1370` — Recorder test infrastructure
  - `src/rpc_dispatch.rs:1890-2650` — Test assertions to update
  - `src/tool_registry.rs` — `primary_name_for()` function from Task 1

  **Acceptance Criteria**:
  - [ ] `cargo build` succeeds
  - [ ] `cargo test` passes — all dispatch tests updated for new names
  - [ ] Calling old name (e.g., `"disasm"`) dispatches correctly via alias resolution
  - [ ] Calling new name (e.g., `"disassemble"`) dispatches correctly directly
  - [ ] Merged strings: `"strings"`, `"find_string"`, `"analyze_strings"` all route to same handler

  **QA Scenarios:**
  ```
  Scenario: Alias resolution dispatches correctly
    Tool: Bash (cargo test)
    Steps:
      1. cargo test -- rpc_dispatch → all tests pass
      2. Verify test coverage includes both old and new names
    Expected Result: All dispatch tests pass with new names + alias fallback
    Evidence: .sisyphus/evidence/task-4-dispatch.txt

  Scenario: No old primary names in match arms
    Tool: Bash (grep)
    Steps:
      1. grep '"disasm" =>' src/rpc_dispatch.rs → expect 0 (resolved by alias before match)
      2. grep '"disassemble" =>' src/rpc_dispatch.rs → expect 1
    Expected Result: Match arms only use new primary names
    Evidence: .sisyphus/evidence/task-4-match-arms.txt
  ```

  **Commit**: YES (groups with Task 3)
  - Message: `refactor(dispatch): update RPC dispatch for renamed tools with alias fallback`
  - Files: `src/rpc_dispatch.rs`
  - Pre-commit: `cargo test`


- [ ] 5. Update server/mod.rs — MCP tools/list + JSON schema for new names

  **What to do**:
  - Update the `tools/list` handler to expose new primary names (not aliases) in MCP tool listing
  - Update all `match tool_name { ... }` blocks that map tool names to JSON schemas
  - For merged `list_strings`: create a unified JSON schema that accepts the union of `strings`, `find_string`, `analyze_strings` parameters: `query` (optional string), `filter` (optional string, alias for query), `offset` (int, default 0), `limit` (int, default 100), `exact` (bool, default false)
  - Update tool description strings shown to MCP clients to use new names
  - Add JSON schemas for the 7 new tools (Task 6, 7 will implement handlers, but schemas can be defined here first)

  **Must NOT do**:
  - Do NOT change the MCP protocol structure (JSON-RPC envelope, tools/list response format)
  - Do NOT change parameter names within schemas

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: [`rust-skills`]

  **Parallelization**:
  - **Can Run In Parallel**: NO (depends on Task 3, 4)
  - **Parallel Group**: Wave 2 (after Task 4)
  - **Blocks**: Tasks 6, 7, 8, 12
  - **Blocked By**: Tasks 3, 4

  **References**:
  - `src/server/mod.rs:576-711` — Current tool name references in MCP handlers
  - `src/server/mod.rs:4585-4625` — Tool name → JSON schema match block
  - `src/server/mod.rs:1018` — `rename` schema reference
  - `src/server/requests.rs` — Request types (StringsRequest, FindStringRequest, AnalyzeStringsRequest to merge)

  **Acceptance Criteria**:
  - [ ] `cargo build` succeeds
  - [ ] MCP tools/list returns new primary names
  - [ ] JSON schemas defined for all 7 new tools
  - [ ] `list_strings` schema accepts union of old 3 tools' params

  **QA Scenarios:**
  ```
  Scenario: MCP tools/list uses new names
    Tool: Bash (cargo test)
    Steps:
      1. cargo test -- server 2>&1
      2. Assert all server tests pass
    Expected Result: Server module compiles and tests pass with new names
    Evidence: .sisyphus/evidence/task-5-server.txt

  Scenario: No old primary names in schema match
    Tool: Bash (grep)
    Steps:
      1. grep '"disasm" =>' src/server/mod.rs → expect 0
      2. grep '"disassemble" =>' src/server/mod.rs → expect 1 (or via alias resolution)
    Expected Result: Schema match uses new primary names
    Evidence: .sisyphus/evidence/task-5-schema.txt
  ```

  **Commit**: YES (groups with Task 3, 4)
  - Message: `refactor(server): update MCP tools/list and schemas for renamed tools`
  - Files: `src/server/mod.rs`, `src/server/requests.rs`
  - Pre-commit: `cargo test`

- [ ] 6. Implement 4 new tools: get/set_function_prototype, set_function_comment, batch_rename

  **What to do**:
  - **`get_function_prototype`**: Read-only. Given address or name, return the current function prototype string (e.g., `"int __fastcall foo(void *ctx, int len)"`). Use idalib's `get_func_type()` / `idc_get_type()` equivalent. Add to worker_trait.rs, worker.rs, handlers/functions.rs, rpc_dispatch.rs, server/mod.rs, tool_registry.rs.
  - **`set_function_prototype`**: Write. Given address/name and a C prototype string, parse and apply it. Similar to ida-pro-mcp's implementation using `tinfo_t` constructor + `apply_tinfo()`. Add to handlers/types.rs or annotations.rs.
  - **`set_function_comment`**: Write. Set a function-level comment (repeatable comment at function entry). Accepts address/name + comment string. Internally calls `set_func_cmt()`. Different from `set_comment` (which is per-address) and `set_decompiler_comment` (which is per-decompiler-line).
  - **`batch_rename`**: Write. Accept array of `{address/name, new_name}` pairs. Apply `set_name()` for each. Return success/failure per entry. Reduces round-trips for leaf-first workflows.
  - For each tool: add ToolInfo entry, WorkerDispatch trait method, worker implementation, rpc_dispatch match arm, server schema

  **Must NOT do**:
  - Do NOT modify existing tool handlers — these are NEW additions only
  - Do NOT make `batch_rename` stop on first error — process all entries, report per-entry results

  **Recommended Agent Profile**:
  - **Category**: `deep`
  - **Skills**: [`rust-skills`, `ida-pro`]
    - `rust-skills`: Rust implementation patterns
    - `ida-pro`: IDA API knowledge for FFI calls (set_func_cmt, apply_tinfo, set_name)

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 3 (with Tasks 7, 8)
  - **Blocks**: Tasks 10, 12
  - **Blocked By**: Tasks 3, 4, 5

  **References**:
  - `src/ida/handlers/annotations.rs` — Existing `set_comments` handler pattern to follow for `set_function_comment`
  - `src/ida/handlers/types.rs` — Existing `apply_types` handler pattern for `set_function_prototype`
  - `src/ida/handlers/functions.rs` — Existing function handlers for `get_function_prototype`
  - `src/ida/worker_trait.rs:44-80` — WorkerDispatch trait method signature patterns
  - `src/ida/worker.rs` — Worker implementation patterns
  - `src/rpc_dispatch.rs:530-540` — Current `rename` dispatch as template for `batch_rename`
  - ida-pro-mcp `mcp-plugin.py:1543-1561` — `set_function_prototype` reference implementation

  **Acceptance Criteria**:
  - [ ] `cargo build` succeeds
  - [ ] 4 new ToolInfo entries in tool_registry.rs
  - [ ] 4 new WorkerDispatch trait methods
  - [ ] 4 new rpc_dispatch match arms
  - [ ] 4 new JSON schemas in server/mod.rs

  **QA Scenarios:**
  ```
  Scenario: New tools compile and register
    Tool: Bash (cargo build + cargo test)
    Steps:
      1. cargo build 2>&1 → expect success
      2. cargo test -- tool_registry 2>&1 → expect pass
      3. grep 'get_function_prototype' src/tool_registry.rs → expect 1 entry
      4. grep 'set_function_prototype' src/tool_registry.rs → expect 1 entry
      5. grep 'set_function_comment' src/tool_registry.rs → expect 1 entry
      6. grep 'batch_rename' src/tool_registry.rs → expect 1 entry
    Expected Result: All 4 tools registered and compilable
    Evidence: .sisyphus/evidence/task-6-new-tools.txt
  ```

  **Commit**: YES
  - Message: `feat(tools): add get/set_function_prototype, set_function_comment, batch_rename`
  - Files: `src/ida/handlers/annotations.rs`, `src/ida/handlers/types.rs`, `src/ida/handlers/functions.rs`, `src/ida/worker_trait.rs`, `src/ida/worker.rs`, `src/rpc_dispatch.rs`, `src/server/mod.rs`, `src/tool_registry.rs`
  - Pre-commit: `cargo test`

- [ ] 7. Implement 3 new tools: rename_stack_variable, set_stack_variable_type, list_enums/create_enum

  **What to do**:
  - **`rename_stack_variable`**: Write. Given function address/name, old variable name, new name. Rename a stack frame variable. Use `define_stkvar()` pattern from ida-pro-mcp (get frame tif → get udm → validate not special/arg → define with new name).
  - **`set_stack_variable_type`**: Write. Given function address/name, variable name, type string. Set the type of a stack variable. Use `set_frame_member_type()` pattern from ida-pro-mcp.
  - **`list_enums`**: Read-only. List all enum types in the database with their members and values. Iterate ordinals, filter `tif.is_enum()`.
  - **`create_enum`**: Write. Create an enum type from a C declaration or member list. Can be implemented via `declare_c_type` internally, but provides a more ergonomic interface: `{name: "ErrorCode", members: [{name: "OK", value: 0}, {name: "ERR", value: 1}]}`.
  - For each tool: add ToolInfo entry, WorkerDispatch trait method, worker implementation, rpc_dispatch match arm, server schema

  **Must NOT do**:
  - Do NOT modify existing stack/type handlers — these are NEW additions
  - For `rename_stack_variable`: Do NOT allow renaming special frame members (return address, saved registers) or function arguments — follow ida-pro-mcp's safety checks

  **Recommended Agent Profile**:
  - **Category**: `deep`
  - **Skills**: [`rust-skills`, `ida-pro`]
    - `rust-skills`: Rust implementation patterns
    - `ida-pro`: IDA frame API, enum API

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 3 (with Tasks 6, 8)
  - **Blocks**: Tasks 10, 12
  - **Blocked By**: Tasks 3, 4, 5

  **References**:
  - `src/ida/handlers/structs.rs` — Existing struct handlers as pattern for enum listing
  - `src/ida/handlers/types.rs` — Existing type handlers for `declare_stack`/`delete_stack` pattern
  - ida-pro-mcp `mcp-plugin.py:1743-1797` — `rename_stack_frame_variable` and `create_stack_frame_variable` reference impl
  - ida-pro-mcp `mcp-plugin.py:1799-1861` — `set_stack_frame_variable_type` and `delete_stack_frame_variable` reference impl
  - `3rd-github/idalib/idalib/src/udt.rs` — idalib UDT/enum type support

  **Acceptance Criteria**:
  - [ ] `cargo build` succeeds
  - [ ] 3-4 new ToolInfo entries in tool_registry.rs (list_enums + create_enum may be 1 or 2)
  - [ ] All new WorkerDispatch trait methods added
  - [ ] All new rpc_dispatch match arms added

  **QA Scenarios:**
  ```
  Scenario: New tools compile and register
    Tool: Bash (cargo build + grep)
    Steps:
      1. cargo build 2>&1 → expect success
      2. grep 'rename_stack_variable' src/tool_registry.rs → expect 1 entry
      3. grep 'set_stack_variable_type' src/tool_registry.rs → expect 1 entry
      4. grep 'list_enums\|create_enum' src/tool_registry.rs → expect entries
    Expected Result: All tools registered and compilable
    Evidence: .sisyphus/evidence/task-7-new-tools.txt
  ```

  **Commit**: YES
  - Message: `feat(tools): add rename_stack_variable, set_stack_variable_type, enum support`
  - Files: `src/ida/handlers/types.rs`, `src/ida/handlers/structs.rs`, `src/ida/worker_trait.rs`, `src/ida/worker.rs`, `src/rpc_dispatch.rs`, `src/server/mod.rs`, `src/tool_registry.rs`
  - Pre-commit: `cargo test`

- [ ] 8. Merge strings/find_string/analyze_strings → list_strings

  **What to do**:
  - In `rpc_dispatch.rs`: create a single unified handler for `list_strings` that accepts the UNION of parameters from all 3 old tools:
    - `query` (string, optional) — substring search (from find_string + analyze_strings)
    - `filter` (string, optional) — alias for query (from strings)
    - `offset` (int, default 0) — pagination
    - `limit` (int, default 100) — pagination
    - `exact` (bool, default false) — exact match mode (from find_string)
  - If both `query` and `filter` are provided, prefer `query`; if only `filter`, treat as `query`
  - In `server/mod.rs`: single JSON schema for `list_strings`
  - In `server/requests.rs`: single `ListStringsRequest` struct replacing 3 old request types
  - Worker trait: can reuse existing `strings()` worker method internally, just route params differently
  - Aliases `strings`, `find_string`, `analyze_strings` all resolve to `list_strings` (already set up in Task 3)

  **Must NOT do**:
  - Do NOT change the worker trait method signature for strings — adapt at the dispatch layer
  - Do NOT break existing callers using old parameter names

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: [`rust-skills`]

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 3 (with Tasks 6, 7)
  - **Blocks**: Tasks 9, 10, 12
  - **Blocked By**: Tasks 3, 4, 5

  **References**:
  - `src/rpc_dispatch.rs:266-300` — Current 3 separate dispatch arms for strings
  - `src/server/mod.rs:2062-2095` — Current string tool schemas
  - `src/server/requests.rs` — StringsRequest, FindStringRequest, AnalyzeStringsRequest structs
  - `src/ida/handlers/strings.rs` — Underlying string handler implementation

  **Acceptance Criteria**:
  - [ ] `cargo build` succeeds
  - [ ] Single `list_strings` dispatch arm handles all 3 old calling patterns
  - [ ] Old parameter names (`filter` from `strings`) still work
  - [ ] `query` parameter works for substring search
  - [ ] `exact` parameter works for exact match
  - [ ] Pagination works (offset, limit)

  **QA Scenarios:**
  ```
  Scenario: Merged handler accepts all old parameter formats
    Tool: Bash (cargo test)
    Steps:
      1. cargo test 2>&1 → all tests pass
      2. grep -c '"list_strings" =>' src/rpc_dispatch.rs → expect 1
      3. grep -c '"strings" =>' src/rpc_dispatch.rs → expect 0 (resolved by alias)
      4. grep -c '"find_string" =>' src/rpc_dispatch.rs → expect 0
    Expected Result: Single dispatch arm, old names aliased away
    Evidence: .sisyphus/evidence/task-8-merge.txt
  ```

  **Commit**: YES
  - Message: `refactor(tools): merge strings/find_string/analyze_strings into list_strings`
  - Files: `src/rpc_dispatch.rs`, `src/server/mod.rs`, `src/server/requests.rs`
  - Pre-commit: `cargo test`

- [ ] 9. Update solana-sbpf-reverse skill files — tool name replacement

  **What to do**:
  - In `~/.config/opencode/skills/solana-sbpf-reverse/SKILL.md`: find-and-replace all old tool names with new names throughout the file (~84 occurrences)
  - In `~/.config/opencode/skills/solana-sbpf-reverse/references/swap-pipeline.md`: same replacement (~50+ occurrences)
  - Key replacements:
    - `open_sbpf` → UNCHANGED
    - `open_idb` → UNCHANGED
    - `decompile` (standalone call) → `decompile_function`
    - `disasm` → `disassemble`
    - `find_bytes` → `search_bytes`
    - `list_functions` → UNCHANGED
    - `xrefs_to` → `get_xrefs_to`
    - `callers` → `get_callers`
    - `callees` → `get_callees`
    - `function_at` → `get_function_at_address`
    - `rename(` → `rename_symbol(`
    - `basic_blocks` → `get_basic_blocks`
    - `callgraph` → `build_callgraph`
    - `search(kind:` → `search_text(kind:`
    - `get_u32`/`get_u64` → `read_dword`/`read_qword`
    - `declare_type` → `declare_c_type`
    - `apply_types` → `apply_type`
    - `xrefs_to_field` → `get_xrefs_to_struct_field`
    - `strings(` → `list_strings(`
    - `find_string` → `list_strings`
  - Do NOT add new MCP tool reference sections — those stay in ida-pro skill only
  - Remove any duplicate MCP tool documentation that overlaps with ida-pro skill

  **Must NOT do**:
  - Do NOT add comprehensive tool reference to this file — point to ida-pro skill instead
  - Do NOT change Solana-specific methodology content
  - Do NOT change file structure or section organization

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: [`rust-skills`]

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 4 (with Tasks 10, 11)
  - **Blocks**: None
  - **Blocked By**: Task 8 (need final merged name for strings)

  **References**:
  - `~/.config/opencode/skills/solana-sbpf-reverse/SKILL.md` — Full file, ~250 lines
  - `~/.config/opencode/skills/solana-sbpf-reverse/references/swap-pipeline.md` — Full file, ~320 lines
  - Task 2 mapping table — authoritative rename list

  **Acceptance Criteria**:
  - [ ] Zero old primary tool names remain in either file (except in alias documentation)
  - [ ] All tool calls use new names
  - [ ] `open_sbpf`/`open_idb` unchanged
  - [ ] Solana methodology content untouched

  **QA Scenarios:**
  ```
  Scenario: No stale tool names in solana-sbpf-reverse skill
    Tool: Bash (grep)
    Steps:
      1. grep -n 'disasm(' ~/.config/opencode/skills/solana-sbpf-reverse/SKILL.md → expect 0
      2. grep -n 'disasm(' ~/.config/opencode/skills/solana-sbpf-reverse/references/swap-pipeline.md → expect 0
      3. grep -n 'xrefs_to(' ~/.config/opencode/skills/solana-sbpf-reverse/ -r → expect 0 (should be get_xrefs_to)
      4. grep -n 'decompile_function' ~/.config/opencode/skills/solana-sbpf-reverse/ -r → expect multiple
    Expected Result: All tool names updated to new convention
    Evidence: .sisyphus/evidence/task-9-sbpf-skill.txt
  ```

  **Commit**: NO (skill files outside repo, not git-tracked)

- [ ] 10. Rewrite ida-pro/SKILL.md Part 2 — MCP tool reference with new names + new tools

  **What to do**:
  - Complete rewrite of Part 2 "MCP 工具速查" section in `~/.config/opencode/skills/ida-pro/SKILL.md`
  - Update ALL tool names to new primary names
  - Add documentation for the 7 new tools in appropriate subsections:
    - 标注 section: `set_function_comment`, `batch_rename_symbols` (batch_rename), `rename_stack_variable`, `set_stack_variable_type`
    - 函数 section: `get_function_prototype`, `set_function_prototype`
    - 类型 section: `list_enums`, `create_enum`
  - Show `list_strings` as the unified string tool (mention old aliases for reference)
  - Update the "69 个工具" count to final count (~73)
  - Ensure Part 2 is THE authoritative MCP tool reference (solana skill points here)
  - Key renames to apply in code examples:
    ```
    decompile(address: "0x1000") → decompile_function(address: "0x1000")
    disasm(address:...) → disassemble(address:...)
    disasm_by_name(name:...) → disassemble_function(name:...)
    disasm_function_at(address:...) → disassemble_function_at(address:...)
    rename(address:...) → rename_symbol(address:...)
    rename_lvar(...) → rename_local_variable(...)
    set_lvar_type(...) → set_local_variable_type(...)
    xrefs_to(address:...) → get_xrefs_to(address:...)
    xrefs_from(address:...) → get_xrefs_from(address:...)
    callers(address:...) → get_callers(address:...)
    callees(address:...) → get_callees(address:...)
    callgraph(roots:...) → build_callgraph(roots:...)
    basic_blocks(address:...) → get_basic_blocks(address:...)
    segments() → list_segments()
    imports() → list_imports()
    exports() → list_exports()
    entrypoints() → list_entry_points()
    idb_meta() → get_database_info()
    analysis_status() → get_analysis_status()
    addr_info(address:...) → get_address_info(address:...)
    get_bytes(address:...) → read_bytes(address:...)
    get_u32/get_u64(address:...) → read_dword/read_qword(address:...)
    get_string(address:...) → read_string(address:...)
    get_global_value(query:...) → read_global_variable(query:...)
    find_bytes(pattern:...) → search_bytes(pattern:...)
    search(targets:...) → search_text(targets:...)
    strings(filter:...) → list_strings(query:...)
    find_insns(patterns:...) → search_instructions(patterns:...)
    find_insn_operands(patterns:...) → search_instruction_operands(patterns:...)
    local_types(query:...) → list_local_types(query:...)
    declare_type(decl:...) → declare_c_type(decl:...)
    apply_types(name:...) → apply_type(name:...)
    infer_types(name:...) → infer_type(name:...)
    stack_frame(address:...) → get_stack_frame(address:...)
    declare_stack(...) → create_stack_variable(...)
    structs(filter:...) → list_structs(filter:...)
    struct_info(name:...) → get_struct_info(name:...)
    read_struct(address:...) → read_struct_at_address(address:...)
    set_comments(address:...) → set_comment(address:...)
    patch(address:...) → patch_bytes(address:...)
    patch_asm(address:...) → patch_assembly(address:...)
    int_convert(inputs:...) → convert_number(inputs:...)
    table_scan(base:...) → scan_memory_table(base:...)
    ```

  **Must NOT do**:
  - Do NOT change Part 1 methodology content (F5-first, leaf-first, etc.)
  - Do NOT change Part 5 Headless API content
  - Do NOT add emoji

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: [`ida-pro`]
    - `ida-pro`: Needs domain context for tool descriptions

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 4 (with Tasks 9, 11)
  - **Blocks**: None
  - **Blocked By**: Tasks 6, 7, 8 (need final new tool list)

  **References**:
  - `~/.config/opencode/skills/ida-pro/SKILL.md:172-338` — Current Part 2 to rewrite
  - Task 2 mapping table — authoritative rename list
  - Task 6, 7 — new tools to document

  **Acceptance Criteria**:
  - [ ] All tool names in Part 2 use new primary names
  - [ ] 7 new tools documented with examples
  - [ ] `list_strings` documented as unified tool (old aliases mentioned)
  - [ ] Tool count updated from 69 to actual final count
  - [ ] Zero stale old tool names in code examples

  **QA Scenarios:**
  ```
  Scenario: No stale tool names in Part 2
    Tool: Bash (grep)
    Steps:
      1. grep -n 'disasm(' ~/.config/opencode/skills/ida-pro/SKILL.md → expect 0 (should be disassemble)
      2. grep -n 'xrefs_to(' ~/.config/opencode/skills/ida-pro/SKILL.md → expect 0 (should be get_xrefs_to)
      3. grep -n 'set_function_prototype' ~/.config/opencode/skills/ida-pro/SKILL.md → expect ≥1
      4. grep -n 'batch_rename' ~/.config/opencode/skills/ida-pro/SKILL.md → expect ≥1
      5. grep -n 'list_enums' ~/.config/opencode/skills/ida-pro/SKILL.md → expect ≥1
    Expected Result: All new names present, no stale names
    Evidence: .sisyphus/evidence/task-10-skill-part2.txt
  ```

  **Commit**: NO (skill files outside repo)

- [ ] 11. Update ida-pro/SKILL.md Part 3-4 — workflows and error recovery with new names

  **What to do**:
  - Update Part 3 "通用 Workflow" (6 workflows) — all tool calls use new names
  - Update Part 4 "错误恢复" — all tool calls use new names
  - Update "Common Pitfalls" section — all tool references use new names
  - Enhance Workflow 2 (Struct Reconstruction) to include `rename_stack_variable` and `set_stack_variable_type` in the flow
  - Enhance Workflow 1 (Binary Orientation) to include `get_function_prototype` example
  - Add a new Workflow 7: Batch Annotation — demonstrating `batch_rename` + `set_function_comment` in leaf-first flow
  - Apply all the same name substitutions listed in Task 10

  **Must NOT do**:
  - Do NOT change Part 1 (methodology) or Part 5 (Headless API)
  - Do NOT restructure existing workflows — only update tool names and add enhancements

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: [`ida-pro`]

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 4 (with Tasks 9, 10)
  - **Blocks**: None
  - **Blocked By**: Task 3 (need rename list)

  **References**:
  - `~/.config/opencode/skills/ida-pro/SKILL.md:343-450` — Part 3 workflows
  - `~/.config/opencode/skills/ida-pro/SKILL.md:414-449` — Part 4 error recovery + pitfalls
  - Task 2 mapping table — authoritative rename list

  **Acceptance Criteria**:
  - [ ] All 6 existing workflows use new tool names
  - [ ] Error recovery section uses new names
  - [ ] Common Pitfalls uses new names
  - [ ] Workflow 2 enhanced with `rename_stack_variable` / `set_stack_variable_type`
  - [ ] New Workflow 7 added for batch annotation
  - [ ] Zero stale tool names

  **QA Scenarios:**
  ```
  Scenario: No stale tool names in Part 3-4
    Tool: Bash (grep)
    Steps:
      1. sed -n '343,450p' ~/.config/opencode/skills/ida-pro/SKILL.md | grep -c 'disasm\|xrefs_to(' → expect 0
      2. grep -n 'rename_stack_variable' ~/.config/opencode/skills/ida-pro/SKILL.md → expect ≥1 (in workflow 2)
      3. grep -n 'batch_rename' ~/.config/opencode/skills/ida-pro/SKILL.md → expect ≥1 (in workflow 7)
    Expected Result: Workflows and error recovery fully updated
    Evidence: .sisyphus/evidence/task-11-skill-workflows.txt
  ```

  **Commit**: NO (skill files outside repo)

- [ ] 12. Update tests + regenerate docs/TOOLS.md

  **What to do**:
  - Run `cargo test` and fix any remaining test failures from the rename
  - Update any integration test fixtures that reference old tool names
  - Run `cargo run --bin gen_tools_doc` to regenerate `docs/TOOLS.md` from the updated tool_registry
  - Verify `docs/TOOLS.md` shows new primary names and documents aliases
  - Run `cargo clippy` and fix any warnings
  - Final `cargo build && cargo test && cargo clippy` to ensure clean state

  **Must NOT do**:
  - Do NOT manually edit `docs/TOOLS.md` — it's auto-generated
  - Do NOT change test behavior — only update name strings

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: [`rust-skills`]

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 5 (after all implementation)
  - **Blocks**: F1-F4
  - **Blocked By**: Tasks 3, 6, 7, 8

  **References**:
  - `src/bin/gen_tools_doc.rs` — TOOLS.md generator
  - `docs/TOOLS.md` — Auto-generated tool documentation
  - `src/rpc_dispatch.rs` — Integration tests at bottom of file
  - `src/tool_registry.rs:1079-1105` — Unit tests

  **Acceptance Criteria**:
  - [ ] `cargo build` succeeds
  - [ ] `cargo test` — 100% pass
  - [ ] `cargo clippy` — 0 warnings
  - [ ] `docs/TOOLS.md` regenerated with new names
  - [ ] `docs/TOOLS.md` mentions aliases for renamed tools

  **QA Scenarios:**
  ```
  Scenario: Full build + test + clippy clean
    Tool: Bash
    Steps:
      1. cargo build 2>&1 → exit 0
      2. cargo test 2>&1 → exit 0, all tests pass
      3. cargo clippy 2>&1 → no warnings
    Expected Result: Clean build, all tests pass, no clippy warnings
    Evidence: .sisyphus/evidence/task-12-final-build.txt

  Scenario: TOOLS.md regenerated
    Tool: Bash
    Steps:
      1. cargo run --bin gen_tools_doc 2>&1 → success
      2. grep 'disassemble' docs/TOOLS.md → expect ≥1
      3. grep 'disasm' docs/TOOLS.md → expect only in alias mentions
    Expected Result: TOOLS.md uses new names, aliases documented
    Evidence: .sisyphus/evidence/task-12-tools-doc.txt
  ```

  **Commit**: YES
  - Message: `chore(docs): regenerate TOOLS.md and fix remaining tests after tool rename`
  - Files: `docs/TOOLS.md`, any test files with fixes
  - Pre-commit: `cargo test`

## Final Verification Wave

> 4 review agents run in PARALLEL. ALL must APPROVE. Rejection → fix → re-run.

- [ ] F1. **Plan Compliance Audit** — `oracle`
  Read the plan end-to-end. For each "Must Have": verify implementation exists. For each "Must NOT Have": search codebase for forbidden patterns. Check: all 69 tools have new names, all 7 new tools exist, strings merged, skill files updated, aliases work.
  Output: `Must Have [N/N] | Must NOT Have [N/N] | Tasks [N/N] | VERDICT: APPROVE/REJECT`

- [ ] F2. **Code Quality Review** — `unspecified-high`
  Run `cargo build && cargo test && cargo clippy`. Review all changed .rs files for: unused imports, dead code, inconsistent naming in new code, missing error handling in new tools. Check AI slop: excessive comments, over-abstraction.
  Output: `Build [PASS/FAIL] | Tests [N pass/N fail] | Clippy [PASS/FAIL] | VERDICT`

- [ ] F3. **Real Manual QA** — `unspecified-high` (+ `ida-pro` skill)
  Start ida-mcp server. Call every renamed tool by OLD name (alias) — verify dispatch works. Call every renamed tool by NEW name — verify dispatch works. Call all 7 new tools with valid params. Call `list_strings` (merged) with filter, query, pagination. Verify `tool_catalog` returns new names. Save evidence.
  Output: `Alias tests [N/N pass] | New name tests [N/N pass] | New tools [N/N] | VERDICT`

- [ ] F4. **Scope Fidelity Check** — `deep`
  For each task: read spec, read actual changes. Verify: no internal Rust method names changed, no JSON parameter names changed, no tools deleted (only aliased), skill files have zero stale tool names (grep for old names).
  Output: `Tasks [N/N compliant] | Guardrails [N/N respected] | VERDICT`

---

## Commit Strategy

- **Wave 1-2**: `refactor(tools): add alias infrastructure and rename all 69 tools to verb_object convention` — tool_registry.rs, rpc_dispatch.rs, server/mod.rs
- **Wave 3**: `feat(tools): add 7 new tools and merge string tools into list_strings` — handlers/, worker_trait.rs, worker.rs, tool_registry.rs, rpc_dispatch.rs, server/mod.rs
- **Wave 4**: `docs(skills): update ida-pro and solana-sbpf-reverse skills with new tool names` — SKILL.md files
- **Wave 5**: `test(tools): update tests and regenerate TOOLS.md` — tests, docs/TOOLS.md

---

## Success Criteria

### Verification Commands
```bash
cargo build              # Expected: Compiles successfully
cargo test               # Expected: All tests pass
cargo clippy             # Expected: No warnings
grep -rn '"disasm"' src/ # Expected: Only in aliases arrays, not as primary names
grep -rn 'analyze_strings\|find_string' src/tool_registry.rs  # Expected: Only in aliases
```

### Final Checklist
- [ ] All 69 tools have verb_object primary names
- [ ] All old names exist as aliases
- [ ] 7 new tools implemented and callable
- [ ] strings/find_string/analyze_strings merged into list_strings
- [ ] ida-pro/SKILL.md fully updated with new names + new tool docs
- [ ] solana-sbpf-reverse skill files updated
- [ ] docs/TOOLS.md regenerated
- [ ] cargo build && cargo test && cargo clippy all pass
