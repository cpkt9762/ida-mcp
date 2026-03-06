# IDA MCP 动态调试工具 — IDAPython Bridge 实现

## TL;DR

> **Quick Summary**: 通过 IDAPython bridge（`run_script` 基础设施）为 ida-mcp-rs 添加 23 个动态调试 MCP 工具，覆盖调试器加载、进程控制、断点管理、执行控制、寄存器/内存读写、线程管理和调试事件 8 个功能组。
>
> **Deliverables**:
> - 23 个新的 `dbg_*` MCP 工具（ToolCategory::Debug）
> - `src/ida/handlers/debug/` 模块（Python 脚本生成 + JSON 解析）
> - 23 个 Request 结构体、23 个 #[tool] 方法、23 个 ToolInfo 注册
> - 通过 HTTP 的端到端集成测试
>
> **Estimated Effort**: Large
> **Parallel Execution**: YES — 4 waves
> **Critical Path**: Task 1 → Task 3-6 → Task 7-8 → Integration Test

---

## Context

### Original Request
用户询问 ida-mcp-rs 项目是否能支持动态调试。经过 IDA SDK 9.3 调试器 API 研究和 headless 可行性测试（FULLY_FEASIBLE），确认通过 IDAPython bridge 实现动态调试完全可行。

### Interview Summary
**Key Discussions**:
- **使用场景**: 本地二进制调试（macOS/Linux Mach-O/ELF）
- **实现路径**: Path A — IDAPython bridge via `run_script`（快速、安全、27 个 API 已验证可用）
- **MVP 范围**: 全部 8 个工具组（进程控制、断点、执行控制、寄存器、内存、线程、事件、调试器加载）

**Research Findings**:
- **可行性测试**: 4/4 级别全部 PASS — import ✅, API existence (27/27) ✅, readonly calls (8/8) ✅, debugger load ✅
- **callui() 路由**: 在 headless/batch 模式下正常路由（之前最担心的问题不存在）
- **断点 CRUD**: 在无调试器状态下完全可用 — `add_bpt=true, check_bpt=1(BPTCK_YES), del_bpt=true`
- **工具模式**: 5 步骤 — Request struct → Handler → #[tool] → Registry → Worker
- **状态管理**: Arc<Mutex<T>>，无全局状态，IDB 在主线程 Option<IDB>
- **并发**: Stdio=单线程 tokio, HTTP=多线程 tokio, 请求队列 mpsc (容量 64)

### Metis Review
**Identified Gaps** (addressed):
- **WFNE_SILENT 强制**: headless 模式必须在所有 `wait_for_next_event` 调用中包含 `WFNE_SILENT` 标志，否则 IDA 会尝试更新不存在的 UI
- **load_debugger 前置条件**: 调试器加载是所有调试操作的前提，且必须调用 `set_debugger_options(0)` 禁用异常对话框
- **WFNE_SUSP 吞事件**: `WFNE_SUSP` 会过滤非暂停事件（如 library_load），MVP 可接受
- **线程安全已保证**: `run_script` 通过 IdaRequest 在 IDA 主线程执行，无需 `execute_sync`
- **状态转换表**: 每个工具的 Python preamble 必须根据 DSTATE 检查前置条件

---

## Work Objectives

### Core Objective
通过 IDAPython bridge 为 ida-mcp-rs 添加完整的动态调试能力，使 AI agent 能通过 MCP 协议控制 IDA Pro 的调试器进行断点设置、单步执行、寄存器/内存读取等操作。

### Concrete Deliverables
- 23 个 `dbg_*` MCP 工具，全部注册在 `ToolCategory::Debug` 下
- `src/ida/handlers/debug/` 目录：`mod.rs`（基础设施）+ 4 个子模块（按功能分组的 Python 脚本生成器）
- `src/server/requests.rs` 中 23 个新的 Request 结构体
- `src/server/mod.rs` 中 23 个 `#[tool]` 方法 + `run_debug_script()` 辅助方法
- `src/tool_registry.rs` 中 23 个 ToolInfo 条目
- HTTP 端到端集成测试脚本

### Definition of Done
- [ ] `cargo build --release` 编译无错误
- [ ] 所有 23 个工具在 `tool_catalog(category: "debug")` 中可见
- [ ] 通过 HTTP 接口成功执行完整调试工作流：load_debugger → start_process → add_breakpoint → continue → get_registers → read_memory → exit_process
- [ ] 每个工具返回结构化 JSON 响应

### Must Have
- 每个 Python 脚本包含 `WFNE_SILENT` 标志（headless 安全）
- `dbg_load_debugger` 自动调用 `set_debugger_options(0)` 禁用异常对话框
- 需要暂停状态的工具（寄存器/内存/线程选择）在 preamble 中检查 `get_process_state() == DSTATE_SUSP`
- 执行控制工具（step/continue）使用 `WFNE_CONT | WFNE_SUSP | WFNE_SILENT` 组合
- 所有工具支持 `db_handle` 参数用于多 IDB 路由
- 所有工具支持可选 `timeout_secs` 参数
- 统一的 JSON 输出格式：`{"success": bool, "error": str|null, "data": {...}}`

### Must NOT Have (Guardrails)
- ❌ 不实现 native Rust FFI 绑定（Path B 留待后续）
- ❌ 不实现 `DBG_Hooks` 异步事件通知（超出 MVP 范围）
- ❌ 不实现远程调试支持（仅本地调试）
- ❌ 不修改 `idalib-sys` 或 `idalib` crate
- ❌ 不添加新的 `IdaRequest` 变体（复用现有 `run_script` 基础设施）
- ❌ 不过度抽象 Python 脚本生成（简单的 `format!()` 字符串模板即可）
- ❌ 不添加 JSDoc/文档注释超过每个函数 3 行
- ❌ 不创建新的全局状态或 lazy_static

---

## Verification Strategy (MANDATORY)

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: YES（Rust 内置测试）
- **Automated tests**: Tests-after（在集成任务中验证）
- **Framework**: `cargo test` + HTTP 集成测试脚本

### QA Policy
Every task MUST include agent-executed QA scenarios.
Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

- **Compilation**: `cargo build --release` — 必须通过
- **Tool Discovery**: HTTP `tools/list` 或 `tool_catalog(category: "debug")` — 工具可见
- **Functional**: HTTP `tools/call` 执行每个工具 — 返回预期 JSON

---

## Execution Strategy

### 23 个 MCP 工具清单

| # | 工具名 | 功能组 | 参数 |
|---|--------|--------|------|
| 1 | `dbg_load_debugger` | 调试器加载 | `debugger_name`, `is_remote` |
| 2 | `dbg_start_process` | 进程控制 | `path`, `args`, `start_dir`, `auto_load_debugger` |
| 3 | `dbg_attach_process` | 进程控制 | `pid` |
| 4 | `dbg_detach_process` | 进程控制 | — |
| 5 | `dbg_exit_process` | 进程控制 | — |
| 6 | `dbg_get_state` | 进程控制 | — |
| 7 | `dbg_add_breakpoint` | 断点管理 | `address`, `size`, `bpt_type`, `condition` |
| 8 | `dbg_del_breakpoint` | 断点管理 | `address` |
| 9 | `dbg_enable_breakpoint` | 断点管理 | `address`, `enable` |
| 10 | `dbg_list_breakpoints` | 断点管理 | — |
| 11 | `dbg_continue` | 执行控制 | `timeout_secs` |
| 12 | `dbg_step_into` | 执行控制 | `timeout_secs` |
| 13 | `dbg_step_over` | 执行控制 | `timeout_secs` |
| 14 | `dbg_step_until_ret` | 执行控制 | `timeout_secs` |
| 15 | `dbg_run_to` | 执行控制 | `address`, `timeout_secs` |
| 16 | `dbg_get_registers` | 寄存器 | `register_names` (optional filter) |
| 17 | `dbg_set_register` | 寄存器 | `register_name`, `value` |
| 18 | `dbg_read_memory` | 内存 | `address`, `size` |
| 19 | `dbg_write_memory` | 内存 | `address`, `data` (hex string) |
| 20 | `dbg_get_memory_info` | 内存 | — |
| 21 | `dbg_list_threads` | 线程 | — |
| 22 | `dbg_select_thread` | 线程 | `thread_id` |
| 23 | `dbg_wait_event` | 事件 | `timeout_secs`, `flags` |

### Parallel Execution Waves

```
Wave 1 (Foundation — infrastructure + request structs):
├── Task 1: Debug handler infrastructure module [deep]
└── Task 2: All 23 debug request structs [quick]

Wave 2 (Script Generators — 4 parallel, each creates NEW file):
├── Task 3: Process & Loader scripts (6 tools) [unspecified-high]
├── Task 4: Breakpoint scripts (4 tools) [unspecified-high]
├── Task 5: Execution control scripts (5 tools) [unspecified-high]
└── Task 6: Inspect scripts (registers/memory/threads/events, 8 tools) [unspecified-high]

Wave 3 (Server Integration — 2 parallel, different files):
├── Task 7: 23 #[tool] methods in server/mod.rs [deep]
└── Task 8: 23 ToolInfo entries in tool_registry.rs [quick]

Wave 4 (Verification — 4 parallel):
├── Task F1: Plan compliance audit [oracle]
├── Task F2: Code quality review [unspecified-high]
├── Task F3: Integration test — full debug workflow via HTTP [deep]
└── Task F4: Scope fidelity check [deep]

Critical Path: Task 1 → Tasks 3-6 → Task 7 → F3
Parallel Speedup: ~60% faster than sequential
Max Concurrent: 4 (Waves 2 & 4)
```

### Dependency Matrix

| Task | Depends On | Blocks | Wave |
|------|-----------|--------|------|
| 1 | — | 3,4,5,6,7 | 1 |
| 2 | — | 7 | 1 |
| 3 | 1 | 7,8 | 2 |
| 4 | 1 | 7,8 | 2 |
| 5 | 1 | 7,8 | 2 |
| 6 | 1 | 7,8 | 2 |
| 7 | 1,2,3,4,5,6 | F1-F4 | 3 |
| 8 | 3,4,5,6 | F1-F4 | 3 |
| F1 | 7,8 | — | 4 |
| F2 | 7,8 | — | 4 |
| F3 | 7,8 | — | 4 |
| F4 | 7,8 | — | 4 |

### Agent Dispatch Summary

- **Wave 1**: **2 tasks** — T1 → `deep`, T2 → `quick`
- **Wave 2**: **4 tasks** — T3-T6 → `unspecified-high`
- **Wave 3**: **2 tasks** — T7 → `deep`, T8 → `quick`
- **Wave 4**: **4 tasks** — F1 → `oracle`, F2 → `unspecified-high`, F3 → `deep`, F4 → `deep`

---

## TODOs

- [ ] 1. Debug Handler Infrastructure Module

  **What to do**:
  - 创建 `src/ida/handlers/debug/mod.rs`，包含：
    - `HEADLESS_PREAMBLE` 常量 — 所有 debug Python 脚本的通用前导代码，包含：
      - `import json, ida_dbg, ida_idd, idaapi`
      - 状态常量定义：`DSTATE_SUSP = -1, DSTATE_NOTASK = 0, DSTATE_RUN = 1`
      - `WFNE_SILENT` 标志常量
      - `safe_hex()` 辅助函数（处理 None 和 BADADDR）
      - `make_result(success, data=None, error=None)` 辅助函数 — 生成统一 JSON 输出
    - `parse_debug_output(run_script_result: &Value) -> Result<Value, ToolError>` 函数 — 从 run_script 的 stdout 中提取 JSON 结果
    - `build_script(body: &str) -> String` 函数 — 将 HEADLESS_PREAMBLE + body 组合成完整脚本
    - `pub mod process;` / `pub mod breakpoint;` / `pub mod execution;` / `pub mod inspect;` — 子模块声明
  - 在 `src/ida/handlers/mod.rs` 中添加 `pub mod debug;`
  - 在 `src/server/mod.rs` 中添加 `run_debug_script()` 辅助方法：
    ```rust
    async fn run_debug_script(
        &self,
        db_handle: Option<&str>,
        tool_name: &str,
        script: &str,
        timeout: u64,
        route_args: Value,
    ) -> Result<CallToolResult, McpError>
    ```
    此方法封装：路由检查 → `self.worker.run_script()` → `parse_debug_output()` → `CallToolResult`

  **Must NOT do**:
  - 不创建新的 IdaRequest 变体
  - 不添加全局状态或 lazy_static
  - 不在 preamble 中使用 `WFNE_ANY`（必须用 `WFNE_SUSP | WFNE_SILENT`）

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: 基础设施模块需要仔细设计，Python preamble 模板和 JSON 解析逻辑是后续所有工具的基础
  - **Skills**: [`rust-skills`]
    - `rust-skills`: Rust 代码规范、错误处理模式

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 2)
  - **Parallel Group**: Wave 1 (with Task 2)
  - **Blocks**: Tasks 3, 4, 5, 6, 7
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/ida/handlers/mod.rs` — 现有处理器模块声明模式（`pub mod functions;` 等）
  - `src/ida/handlers/script.rs` — `handle_run_script` 实现，展示 run_python + stdout/stderr 捕获 + 错误分类
  - `src/server/mod.rs:4728-4792` — `run_script` #[tool] 方法实现，展示 run_script_succeeded/failure_message 辅助函数用法

  **API/Type References**:
  - `src/error.rs` — `ToolError` 枚举和 `to_tool_result()` 方法
  - `src/server/mod.rs` — `CallToolResult::success(vec![Content::text(...)])` 响应构造

  **External References**:
  - IDA SDK `dbg.hpp` — `WFNE_SUSP=0x0001`, `WFNE_SILENT=0x0004`, `WFNE_CONT=0x0008`, `WFNE_ANY=0`
  - IDAPython `ida_dbg` — `get_process_state()`, `wait_for_next_event()` 签名

  **WHY Each Reference Matters**:
  - `script.rs` 展示了 `run_python()` 如何捕获输出，新的 `parse_debug_output` 需要处理相同的 stdout/stderr 格式
  - `server/mod.rs` 的 `run_script` 方法展示了 timeout 处理和路由检查模式，`run_debug_script` 需要复制此模式
  - `error.rs` 的 `ToolError` 枚举可能需要新增 `DebugError(String)` 变体（或复用 `IdaError`）

  **Acceptance Criteria**:

  - [ ] `cargo build --release` 编译无错误
  - [ ] `src/ida/handlers/debug/mod.rs` 文件存在且包含 HEADLESS_PREAMBLE、parse_debug_output、build_script
  - [ ] `src/ida/handlers/mod.rs` 包含 `pub mod debug;`
  - [ ] HEADLESS_PREAMBLE 中包含 `WFNE_SILENT` 标志定义
  - [ ] Python preamble 是有效 Python 语法

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Python preamble 语法验证
    Tool: Bash
    Preconditions: Task 1 完成
    Steps:
      1. 从 debug/mod.rs 中提取 HEADLESS_PREAMBLE 常量的字符串内容
      2. 运行 python3 -c "compile('''<preamble_content>''', '<test>', 'exec')"
      3. 检查返回码为 0
    Expected Result: Python 编译成功，无 SyntaxError
    Failure Indicators: python3 返回非 0 退出码
    Evidence: .sisyphus/evidence/task-1-preamble-syntax.txt

  Scenario: 编译验证
    Tool: Bash
    Preconditions: Task 1 完成
    Steps:
      1. cargo build --release 2>&1
      2. 检查退出码为 0
    Expected Result: 编译成功
    Failure Indicators: 编译错误
    Evidence: .sisyphus/evidence/task-1-build.txt
  ```

  **Commit**: YES (group with Task 2)
  - Message: `feat(debug): add debug handler infrastructure and request types`
  - Files: `src/ida/handlers/debug/mod.rs`, `src/ida/handlers/mod.rs`, `src/server/mod.rs`
  - Pre-commit: `cargo build --release`

- [ ] 2. All 23 Debug Request Structs

  **What to do**:
  - 在 `src/server/requests.rs` 文件末尾追加 23 个 Request 结构体
  - 每个结构体遵循现有模式：`#[derive(Debug, Deserialize, Serialize, JsonSchema)]`
  - 所有结构体包含 `db_handle: Option<String>` 字段（多 IDB 路由）
  - 具体结构体列表：

  ```rust
  // 1. 调试器加载
  DbgLoadDebuggerRequest { debugger_name: Option<String>, is_remote: Option<bool> }
  // 2-6. 进程控制
  DbgStartProcessRequest { path: Option<String>, args: Option<String>, start_dir: Option<String>, timeout_secs: Option<u64> }
  DbgAttachProcessRequest { pid: Option<u64> }
  DbgDetachProcessRequest { }
  DbgExitProcessRequest { }
  DbgGetStateRequest { }
  // 7-10. 断点
  DbgAddBreakpointRequest { address: String, size: Option<u64>, bpt_type: Option<String>, condition: Option<String> }
  DbgDelBreakpointRequest { address: String }
  DbgEnableBreakpointRequest { address: String, enable: Option<bool> }
  DbgListBreakpointsRequest { }
  // 11-15. 执行控制
  DbgContinueRequest { timeout_secs: Option<u64> }
  DbgStepIntoRequest { timeout_secs: Option<u64> }
  DbgStepOverRequest { timeout_secs: Option<u64> }
  DbgStepUntilRetRequest { timeout_secs: Option<u64> }
  DbgRunToRequest { address: String, timeout_secs: Option<u64> }
  // 16-17. 寄存器
  DbgGetRegistersRequest { register_names: Option<Vec<String>> }
  DbgSetRegisterRequest { register_name: String, value: String }
  // 18-20. 内存
  DbgReadMemoryRequest { address: String, size: u64 }
  DbgWriteMemoryRequest { address: String, data: String }
  DbgGetMemoryInfoRequest { }
  // 21-22. 线程
  DbgListThreadsRequest { }
  DbgSelectThreadRequest { thread_id: u64 }
  // 23. 事件
  DbgWaitEventRequest { timeout_secs: Option<u64>, flags: Option<String> }
  ```

  - 每个字段添加 `#[schemars(description = "...")]` 注解
  - 地址字段使用 String 类型（支持 "0x..." 格式）
  - 可选超时字段默认不填，在 handler 中设默认值

  **Must NOT do**:
  - 不修改现有 request 结构体
  - 不在 request 中添加非调试相关的字段

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: 纯结构体定义，模式统一，机械性工作
  - **Skills**: [`rust-skills`]

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 1)
  - **Parallel Group**: Wave 1 (with Task 1)
  - **Blocks**: Task 7
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/server/requests.rs:1-30` — 现有 Request 结构体模式（OpenIdbRequest, RunScriptRequest 等）
  - `src/server/requests.rs:1169-1186` — RunScriptRequest 作为最相似的模板（包含 timeout_secs, code 等）

  **WHY Each Reference Matters**:
  - requests.rs 展示了 schemars description 注解、serde 属性、Option 字段处理的精确模式，必须严格遵循

  **Acceptance Criteria**:

  - [ ] `cargo build --release` 编译无错误
  - [ ] requests.rs 中存在全部 23 个 Dbg*Request 结构体
  - [ ] 每个结构体包含 `db_handle: Option<String>` 字段
  - [ ] 每个字段有 `#[schemars(description)]` 注解

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: 结构体数量验证
    Tool: Bash
    Preconditions: Task 2 完成
    Steps:
      1. grep -c "pub struct Dbg.*Request" src/server/requests.rs
      2. 检查输出为 23
    Expected Result: 23 个 Dbg*Request 结构体
    Failure Indicators: 数量不等于 23
    Evidence: .sisyphus/evidence/task-2-struct-count.txt

  Scenario: db_handle 字段验证
    Tool: Bash
    Preconditions: Task 2 完成
    Steps:
      1. 对每个 Dbg*Request 结构体检查是否包含 db_handle 字段
      2. grep -A5 "pub struct Dbg" src/server/requests.rs | grep -c "db_handle"
    Expected Result: 23 个 db_handle 字段
    Failure Indicators: 某个结构体缺少 db_handle
    Evidence: .sisyphus/evidence/task-2-db-handle.txt
  ```

  **Commit**: YES (group with Task 1)
  - Message: `feat(debug): add debug handler infrastructure and request types`
  - Files: `src/server/requests.rs`
  - Pre-commit: `cargo build --release`

- [ ] 3. Process & Loader Script Generators (6 tools)

  **What to do**:
  - 创建 `src/ida/handlers/debug/process.rs`
  - 实现 6 个 Python 脚本生成函数，每个返回 `String`：

  **3a. `generate_load_debugger_script(debugger_name: &str, is_remote: bool) -> String`**
  Python 逻辑：
  ```python
  import ida_dbg
  # 检查是否已加载
  if ida_dbg.dbg_is_loaded():
      print(make_result(True, {"already_loaded": True}))
  else:
      ok = ida_dbg.load_debugger("{debugger_name}", {is_remote})
      if ok:
          ida_dbg.set_debugger_options(0)  # EXCDLG_NEVER — 禁用异常对话框
          print(make_result(True, {"loaded": True, "debugger": "{debugger_name}"}))
      else:
          print(make_result(False, error="Failed to load debugger '{debugger_name}'"))
  ```

  **3b. `generate_start_process_script(path: Option<&str>, args: Option<&str>, start_dir: Option<&str>, timeout: u64) -> String`**
  Python 逻辑：
  ```python
  # 自动检测并加载调试器（如果未加载）
  if not ida_dbg.dbg_is_loaded():
      import platform
      dbg_name = {"Darwin": "mac", "Linux": "linux", "Windows": "win32"}.get(platform.system(), "gdb")
      ida_dbg.load_debugger(dbg_name, False)
      ida_dbg.set_debugger_options(0)
  state = ida_dbg.get_process_state()
  if state != 0:  # DSTATE_NOTASK
      print(make_result(False, error=f"Cannot start: process state is {state}"))
  else:
      ret = ida_dbg.start_process({path}, {args}, {start_dir})
      if ret == 1:
          code = ida_dbg.wait_for_next_event(WFNE_SUSP | WFNE_SILENT, {timeout})
          ip = safe_hex(ida_dbg.get_ip_val())
          print(make_result(True, {"event_code": code, "ip": ip, "state": ida_dbg.get_process_state()}))
      else:
          print(make_result(False, error=f"start_process returned {ret}"))
  ```

  **3c. `generate_attach_process_script(pid: Option<u64>, timeout: u64) -> String`**
  Python 逻辑：附加到进程，等待暂停事件

  **3d. `generate_detach_process_script() -> String`**
  Python 逻辑：检查进程存在，调用 detach_process()

  **3e. `generate_exit_process_script() -> String`**
  Python 逻辑：检查进程存在，调用 exit_process()

  **3f. `generate_get_state_script() -> String`**
  Python 逻辑：
  ```python
  state = ida_dbg.get_process_state()
  state_names = {-1: "DSTATE_SUSP", 0: "DSTATE_NOTASK", 1: "DSTATE_RUN"}
  data = {
      "state": state,
      "state_name": state_names.get(state, "unknown"),
      "debugger_loaded": ida_dbg.dbg_is_loaded(),
      "is_debugger_on": ida_dbg.is_debugger_on(),
      "thread_count": ida_dbg.get_thread_qty(),
  }
  if state == -1:  # DSTATE_SUSP
      data["ip"] = safe_hex(ida_dbg.get_ip_val())
      data["sp"] = safe_hex(ida_dbg.get_sp_val())
      data["current_thread"] = ida_dbg.get_current_thread()
  print(make_result(True, data))
  ```

  **Must NOT do**:
  - 不在 Python 脚本中使用 `WFNE_ANY`（必须 `WFNE_SUSP | WFNE_SILENT`）
  - 不跳过 `set_debugger_options(0)` 调用
  - 不省略状态前置检查

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Python 脚本生成需要对 IDA API 精确理解，错误处理需仔细
  - **Skills**: [`rust-skills`]

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 4, 5, 6)
  - **Parallel Group**: Wave 2 (with Tasks 4, 5, 6)
  - **Blocks**: Tasks 7, 8
  - **Blocked By**: Task 1

  **References**:

  **Pattern References**:
  - `src/ida/handlers/debug/mod.rs` (Task 1 output) — `HEADLESS_PREAMBLE`, `build_script()`, `make_result()`

  **API/Type References**:
  - IDA SDK `dbg.hpp:start_process()` — `int start_process(path, args, sdir)` 返回 -1/0/1
  - IDA SDK `dbg.hpp:attach_process()` — `int attach_process(pid, event_id)` 返回 -4/-3/-2/-1/0/1
  - IDA SDK `dbg.hpp:load_debugger()` — `bool load_debugger(dbgname, use_remote)`
  - `scripts/test_dbg_headless.py` — Level 4 测试代码，展示了 `load_debugger("mac", False)` 和断点 CRUD 的工作模式

  **WHY Each Reference Matters**:
  - `test_dbg_headless.py` 是经过验证的参考 — 其中的 API 调用模式已在真实 headless 环境下通过测试
  - `start_process` 返回值有 3 种语义（-1=失败, 0=取消, 1=成功），脚本必须正确处理全部

  **Acceptance Criteria**:
  - [ ] `src/ida/handlers/debug/process.rs` 文件存在且包含 6 个 `generate_*` 函数
  - [ ] 所有 Python 脚本使用 `WFNE_SUSP | WFNE_SILENT`
  - [ ] `load_debugger` 脚本包含 `set_debugger_options(0)`
  - [ ] `start_process` 脚本包含自动加载调试器逻辑
  - [ ] `cargo build --release` 编译无错误

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Python 脚本语法验证（全部 6 个）
    Tool: Bash
    Preconditions: Tasks 1, 3 完成
    Steps:
      1. 编写 Rust 测试或脚本，调用每个 generate_* 函数
      2. 将输出的 Python 字符串传给 python3 -c "compile(source, 'test', 'exec')"
      3. 验证全部 6 个脚本编译成功
    Expected Result: 6/6 脚本语法正确
    Failure Indicators: 任何 SyntaxError
    Evidence: .sisyphus/evidence/task-3-python-syntax.txt

  Scenario: load_debugger 脚本包含安全措施
    Tool: Bash (grep)
    Preconditions: Task 3 完成
    Steps:
      1. grep "set_debugger_options" src/ida/handlers/debug/process.rs
      2. grep "WFNE_SILENT" src/ida/handlers/debug/process.rs
    Expected Result: 两者都存在
    Failure Indicators: 缺少 set_debugger_options 或 WFNE_SILENT
    Evidence: .sisyphus/evidence/task-3-safety-checks.txt
  ```

  **Commit**: YES (group with Tasks 4, 5, 6)
  - Message: `feat(debug): add IDAPython script generators for debug tools`
  - Files: `src/ida/handlers/debug/process.rs`

- [ ] 4. Breakpoint Script Generators (4 tools)

  **What to do**:
  - 创建 `src/ida/handlers/debug/breakpoint.rs`
  - 实现 4 个 Python 脚本生成函数：

  **4a. `generate_add_breakpoint_script(address: u64, size: u64, bpt_type: &str, condition: Option<&str>) -> String`**
  Python 逻辑：
  ```python
  bpt_types = {"soft": 4, "exec": 8, "write": 1, "read": 2, "rdwr": 3, "default": 12}
  btype = bpt_types.get("{bpt_type}", 12)  # BPT_DEFAULT = BPT_SOFT|BPT_EXEC
  ok = ida_dbg.add_bpt({address}, {size}, btype)
  if ok and {condition}:
      bpt = ida_idd.bpt_t()
      if ida_dbg.get_bpt({address}, bpt):
          bpt.condition = "{condition}"
          ida_dbg.update_bpt(bpt)
  print(make_result(ok, {"address": safe_hex({address}), "added": ok}))
  ```

  **4b. `generate_del_breakpoint_script(address: u64) -> String`**
  **4c. `generate_enable_breakpoint_script(address: u64, enable: bool) -> String`**
  **4d. `generate_list_breakpoints_script() -> String`**
  Python 逻辑（list）：
  ```python
  bpts = []
  for i in range(ida_dbg.get_bpt_qty()):
      bpt = ida_idd.bpt_t()
      if ida_dbg.getn_bpt(i, bpt):
          bpts.append({
              "address": safe_hex(bpt.ea),
              "type": bpt.type,
              "size": bpt.size,
              "enabled": bool(bpt.flags & 0x008),  # BPT_ENABLED
              "condition": str(bpt.condition) if bpt.condition else None,
          })
  print(make_result(True, {"breakpoints": bpts, "count": len(bpts)}))
  ```

  **Must NOT do**:
  - 不实现条件断点的 IDC 表达式求值
  - 不实现跟踪断点（BPT_TRACE）

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: [`rust-skills`]

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 3, 5, 6)
  - **Parallel Group**: Wave 2
  - **Blocks**: Tasks 7, 8
  - **Blocked By**: Task 1

  **References**:

  **Pattern References**:
  - `src/ida/handlers/debug/mod.rs` (Task 1 output) — `build_script()`, preamble
  - `scripts/test_dbg_headless.py:186-199` — Level 4 断点测试代码：`add_bpt(entry, 0, BPT_SOFT)`, `check_bpt(entry)`, `del_bpt(entry)` — 已验证可用

  **API/Type References**:
  - IDA SDK `dbg.hpp` — `add_bpt(ea, size, type)`, `del_bpt(ea)`, `enable_bpt(ea, enable)`, `getn_bpt(n, &bpt)`, `get_bpt(ea, &bpt)`
  - IDA SDK `idd.hpp` — `bpt_t` 结构体字段：`ea`, `type`, `size`, `flags`, `condition`
  - `bpttype_t`: `BPT_WRITE=1, BPT_READ=2, BPT_RDWR=3, BPT_SOFT=4, BPT_EXEC=8, BPT_DEFAULT=12`
  - `bpt_t.flags`: `BPT_BRK=0x001, BPT_ENABLED=0x008`

  **Acceptance Criteria**:
  - [ ] `src/ida/handlers/debug/breakpoint.rs` 文件存在且包含 4 个 `generate_*` 函数
  - [ ] list_breakpoints 脚本遍历 `get_bpt_qty()` 并提取完整断点信息
  - [ ] `cargo build --release` 编译无错误

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: 断点脚本 Python 语法验证
    Tool: Bash
    Preconditions: Tasks 1, 4 完成
    Steps:
      1. 调用每个 generate_* 函数获取 Python 脚本
      2. python3 -c "compile(source, 'test', 'exec')" 验证全部 4 个
    Expected Result: 4/4 语法正确
    Evidence: .sisyphus/evidence/task-4-python-syntax.txt

  Scenario: bpt_type 映射完整性
    Tool: Bash (grep)
    Preconditions: Task 4 完成
    Steps:
      1. grep -c "BPT_SOFT\|BPT_EXEC\|BPT_WRITE\|BPT_READ\|BPT_RDWR" src/ida/handlers/debug/breakpoint.rs
    Expected Result: 至少 5 个匹配（全部断点类型都有映射）
    Evidence: .sisyphus/evidence/task-4-bpt-types.txt
  ```

  **Commit**: YES (group with Tasks 3, 5, 6)
  - Message: `feat(debug): add IDAPython script generators for debug tools`
  - Files: `src/ida/handlers/debug/breakpoint.rs`

- [ ] 5. Execution Control Script Generators (5 tools)

  **What to do**:
  - 创建 `src/ida/handlers/debug/execution.rs`
  - 实现 5 个 Python 脚本生成函数：

  **5a. `generate_continue_script(timeout: u64) -> String`**
  Python 逻辑：
  ```python
  state = ida_dbg.get_process_state()
  if state != -1:  # DSTATE_SUSP
      print(make_result(False, error=f"Cannot continue: state={state}, need DSTATE_SUSP(-1)"))
  else:
      ida_dbg.continue_process()
      code = ida_dbg.wait_for_next_event(WFNE_CONT | WFNE_SUSP | WFNE_SILENT, {timeout})
      ip = safe_hex(ida_dbg.get_ip_val())
      evt = ida_dbg.get_debug_event()
      print(make_result(True, {
          "event_code": code,
          "ip": ip,
          "state": ida_dbg.get_process_state(),
          "event_id": evt.eid() if evt else None,
      }))
  ```

  **5b. `generate_step_into_script(timeout: u64) -> String`**
  **5c. `generate_step_over_script(timeout: u64) -> String`**
  **5d. `generate_step_until_ret_script(timeout: u64) -> String`**
  （以上 3 个结构相同，只是 API 调用不同：`step_into()` / `step_over()` / `step_until_ret()`）

  **5e. `generate_run_to_script(address: u64, timeout: u64) -> String`**
  Python 逻辑：检查暂停状态 → `run_to(address)` → `wait_for_next_event` → 返回 IP

  **关键约束**：
  - 所有执行控制脚本必须先验证 `get_process_state() == DSTATE_SUSP (-1)`
  - `wait_for_next_event` 标志必须包含 `WFNE_SUSP | WFNE_SILENT`
  - `continue` 使用 `WFNE_CONT | WFNE_SUSP | WFNE_SILENT`（WFNE_CONT 让它从暂停继续）
  - step 系列不需要 `WFNE_CONT`（step_into/step_over 自身会恢复执行）

  **Must NOT do**:
  - 不实现源码级单步（RESMOD_SRCINTO/SRCOVER）
  - 不使用 WFNE_ANY

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: [`rust-skills`]

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 3, 4, 6)
  - **Parallel Group**: Wave 2
  - **Blocks**: Tasks 7, 8
  - **Blocked By**: Task 1

  **References**:

  **Pattern References**:
  - `src/ida/handlers/debug/mod.rs` (Task 1) — preamble, `build_script()`
  - `src/ida/handlers/debug/process.rs` (Task 3) — `generate_start_process_script` 展示了 wait_for_next_event 的使用模式

  **API/Type References**:
  - IDA SDK `dbg.hpp` — `continue_process()→bool`, `step_into()→bool`, `step_over()→bool`, `step_until_ret()→bool`, `run_to(ea)→bool`
  - IDA SDK `dbg.hpp` — `wait_for_next_event(wfne, timeout)` — `WFNE_SUSP=0x1, WFNE_SILENT=0x4, WFNE_CONT=0x8`
  - IDA SDK `dbg.hpp` — `get_debug_event()` 返回 `debug_event_t*`，`.eid()` 获取事件类型

  **Acceptance Criteria**:
  - [ ] `src/ida/handlers/debug/execution.rs` 存在且包含 5 个函数
  - [ ] 每个函数检查 `get_process_state() == -1` 前置条件
  - [ ] 每个 wait_for_next_event 调用包含 WFNE_SILENT
  - [ ] `cargo build --release` 编译无错误

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: 状态前置检查存在
    Tool: Bash (grep)
    Preconditions: Task 5 完成
    Steps:
      1. grep -c "get_process_state" src/ida/handlers/debug/execution.rs
    Expected Result: 至少 5 个匹配（每个函数一个状态检查）
    Evidence: .sisyphus/evidence/task-5-state-checks.txt

  Scenario: WFNE_SILENT 无遗漏
    Tool: Bash (grep)
    Preconditions: Task 5 完成
    Steps:
      1. grep -c "wait_for_next_event" src/ida/handlers/debug/execution.rs
      2. grep -c "WFNE_SILENT" src/ida/handlers/debug/execution.rs
      3. 两个计数应该相等
    Expected Result: wait_for_next_event 调用数 == WFNE_SILENT 出现数
    Evidence: .sisyphus/evidence/task-5-wfne-silent.txt
  ```

  **Commit**: YES (group with Tasks 3, 4, 6)
  - Message: `feat(debug): add IDAPython script generators for debug tools`
  - Files: `src/ida/handlers/debug/execution.rs`

- [ ] 6. Inspect Script Generators — Registers, Memory, Threads, Events (8 tools)

  **What to do**:
  - 创建 `src/ida/handlers/debug/inspect.rs`
  - 实现 8 个 Python 脚本生成函数：

  **6a. `generate_get_registers_script(register_names: Option<&[String]>) -> String`**
  Python 逻辑：
  ```python
  state = ida_dbg.get_process_state()
  if state != -1:
      print(make_result(False, error=f"Cannot read registers: state={state}"))
  else:
      regs = {}
      # 获取所有寄存器名
      if {filter_names}:
          names = {filter_names}
      else:
          # 枚举所有通用寄存器
          import ida_idp
          names = []
          for i in range(ida_idp.ph_get_regnames().__len__()):
              names.append(ida_idp.ph_get_regnames()[i])
      for name in names:
          rv = ida_dbg.regval_t()
          if ida_dbg.get_reg_val(name, rv):
              regs[name] = safe_hex(rv.ival)
      ip = safe_hex(ida_dbg.get_ip_val())
      sp = safe_hex(ida_dbg.get_sp_val())
      print(make_result(True, {"registers": regs, "ip": ip, "sp": sp}))
  ```

  **6b. `generate_set_register_script(register_name: &str, value: u64) -> String`**
  Python 逻辑：验证暂停 → `set_reg_val(name, value)` → 返回成功/失败

  **6c. `generate_read_memory_script(address: u64, size: u64) -> String`**
  Python 逻辑：
  ```python
  state = ida_dbg.get_process_state()
  if state != -1:
      print(make_result(False, error=f"Cannot read memory: state={state}"))
  else:
      import ida_bytes
      data = ida_dbg.read_dbg_memory({address}, {size})
      if data is not None and len(data) > 0:
          hex_str = data.hex() if isinstance(data, bytes) else ""
          # 尝试 ASCII 解读
          ascii_str = "".join(chr(b) if 32 <= b < 127 else "." for b in data)
          print(make_result(True, {
              "address": safe_hex({address}),
              "size": len(data),
              "hex": hex_str,
              "ascii": ascii_str,
          }))
      else:
          print(make_result(False, error=f"Failed to read {size} bytes at {safe_hex({address})}"))
  ```
  **注意**: size 上限 4096 字节，防止超大读取

  **6d. `generate_write_memory_script(address: u64, data_hex: &str) -> String`**
  **6e. `generate_get_memory_info_script() -> String`**
  Python 逻辑：`get_dbg_memory_info()` → 枚举内存区域（start, end, name, perm, bitness）

  **6f. `generate_list_threads_script() -> String`**
  Python 逻辑：
  ```python
  threads = []
  for i in range(ida_dbg.get_thread_qty()):
      tid = ida_dbg.getn_thread(i)
      name = ida_dbg.getn_thread_name(i) or ""
      threads.append({"id": tid, "name": name, "index": i})
  current = ida_dbg.get_current_thread()
  print(make_result(True, {"threads": threads, "current_thread": current, "count": len(threads)}))
  ```

  **6g. `generate_select_thread_script(thread_id: u64) -> String`**
  Python 逻辑：验证暂停 → `select_thread(tid)` → 返回成功

  **6h. `generate_wait_event_script(timeout: u64, flags: u32) -> String`**
  Python 逻辑：`wait_for_next_event(flags | WFNE_SILENT, timeout)` → 返回事件信息

  **Must NOT do**:
  - 内存读取不超过 4096 字节
  - 不实现连续内存 dump
  - 不实现 DBG_Hooks

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: [`rust-skills`]

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 3, 4, 5)
  - **Parallel Group**: Wave 2
  - **Blocks**: Tasks 7, 8
  - **Blocked By**: Task 1

  **References**:

  **Pattern References**:
  - `src/ida/handlers/debug/mod.rs` (Task 1) — preamble, helpers

  **API/Type References**:
  - IDA SDK `dbg.hpp` — `get_reg_val(name, &regval)→bool`, `set_reg_val(name, &regval)→bool`
  - IDA SDK `dbg.hpp` — `read_dbg_memory(ea, buf, size)→ssize_t`, `write_dbg_memory(ea, buf, size)→ssize_t`
  - IDA SDK `dbg.hpp` — `get_thread_qty()→int`, `getn_thread(n)→thid_t`, `select_thread(tid)→bool`
  - IDA SDK `idd.hpp` — `regval_t`: `ival` (uint64 整数值), `rvtype` (-2=int, -1=float)
  - IDA SDK `dbg.hpp` — `get_dbg_memory_info(&ranges)→int`, `memory_info_t`: start_ea, end_ea, name, perm

  **Acceptance Criteria**:
  - [ ] `src/ida/handlers/debug/inspect.rs` 存在且包含 8 个函数
  - [ ] 寄存器/内存读取脚本检查 DSTATE_SUSP 前置条件
  - [ ] read_memory 脚本限制 size ≤ 4096
  - [ ] `cargo build --release` 编译无错误

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: 内存读取大小限制
    Tool: Bash (grep)
    Preconditions: Task 6 完成
    Steps:
      1. grep "4096" src/ida/handlers/debug/inspect.rs
    Expected Result: 存在内存大小限制检查
    Evidence: .sisyphus/evidence/task-6-memory-limit.txt

  Scenario: 暂停状态检查（寄存器/内存/线程选择）
    Tool: Bash (grep)
    Preconditions: Task 6 完成
    Steps:
      1. grep -c "get_process_state" src/ida/handlers/debug/inspect.rs
    Expected Result: 至少 4 次（get_registers, set_register, read_memory, select_thread 各一次）
    Evidence: .sisyphus/evidence/task-6-state-checks.txt
  ```

  **Commit**: YES (group with Tasks 3, 4, 5)
  - Message: `feat(debug): add IDAPython script generators for debug tools`
  - Files: `src/ida/handlers/debug/inspect.rs`

- [ ] 7. Server #[tool] Methods — All 23 Debug Tools

  **What to do**:
  - 在 `src/server/mod.rs` 的 `#[tool_router] impl IdaMcpServer` 块中追加 23 个 `#[tool]` 方法
  - 每个方法遵循统一模式（使用 Task 1 的 `run_debug_script()` 辅助方法）：

  ```rust
  #[tool(description = "Tool description here")]
  #[instrument(skip(self))]
  async fn dbg_xxx(
      &self,
      Parameters(req): Parameters<DbgXxxRequest>,
  ) -> Result<CallToolResult, McpError> {
      debug!("Tool call: dbg_xxx");
      let timeout = req.timeout_secs.unwrap_or(DEFAULT).min(MAX);
      let script = debug::submodule::generate_xxx_script(params...);
      self.run_debug_script(
          req.db_handle.as_deref(),
          "dbg_xxx",
          &script,
          timeout,
          json!({...route_args...}),
      ).await
  }
  ```

  - 23 个方法的具体映射：

  | 方法名 | Request 类型 | 脚本生成函数 | 默认超时 |
  |--------|-------------|-------------|---------|
  | `dbg_load_debugger` | `DbgLoadDebuggerRequest` | `process::generate_load_debugger_script` | 30s |
  | `dbg_start_process` | `DbgStartProcessRequest` | `process::generate_start_process_script` | 60s |
  | `dbg_attach_process` | `DbgAttachProcessRequest` | `process::generate_attach_process_script` | 30s |
  | `dbg_detach_process` | `DbgDetachProcessRequest` | `process::generate_detach_process_script` | 30s |
  | `dbg_exit_process` | `DbgExitProcessRequest` | `process::generate_exit_process_script` | 30s |
  | `dbg_get_state` | `DbgGetStateRequest` | `process::generate_get_state_script` | 10s |
  | `dbg_add_breakpoint` | `DbgAddBreakpointRequest` | `breakpoint::generate_add_breakpoint_script` | 10s |
  | `dbg_del_breakpoint` | `DbgDelBreakpointRequest` | `breakpoint::generate_del_breakpoint_script` | 10s |
  | `dbg_enable_breakpoint` | `DbgEnableBreakpointRequest` | `breakpoint::generate_enable_breakpoint_script` | 10s |
  | `dbg_list_breakpoints` | `DbgListBreakpointsRequest` | `breakpoint::generate_list_breakpoints_script` | 10s |
  | `dbg_continue` | `DbgContinueRequest` | `execution::generate_continue_script` | 30s |
  | `dbg_step_into` | `DbgStepIntoRequest` | `execution::generate_step_into_script` | 30s |
  | `dbg_step_over` | `DbgStepOverRequest` | `execution::generate_step_over_script` | 30s |
  | `dbg_step_until_ret` | `DbgStepUntilRetRequest` | `execution::generate_step_until_ret_script` | 60s |
  | `dbg_run_to` | `DbgRunToRequest` | `execution::generate_run_to_script` | 60s |
  | `dbg_get_registers` | `DbgGetRegistersRequest` | `inspect::generate_get_registers_script` | 10s |
  | `dbg_set_register` | `DbgSetRegisterRequest` | `inspect::generate_set_register_script` | 10s |
  | `dbg_read_memory` | `DbgReadMemoryRequest` | `inspect::generate_read_memory_script` | 10s |
  | `dbg_write_memory` | `DbgWriteMemoryRequest` | `inspect::generate_write_memory_script` | 10s |
  | `dbg_get_memory_info` | `DbgGetMemoryInfoRequest` | `inspect::generate_get_memory_info_script` | 10s |
  | `dbg_list_threads` | `DbgListThreadsRequest` | `inspect::generate_list_threads_script` | 10s |
  | `dbg_select_thread` | `DbgSelectThreadRequest` | `inspect::generate_select_thread_script` | 10s |
  | `dbg_wait_event` | `DbgWaitEventRequest` | `inspect::generate_wait_event_script` | 30s |

  - 地址参数需要通过 `parse_address_str()` 解析（支持 "0x..." 格式）
  - 所有超时通过 `.min(600)` 上限限制（与现有 run_script 一致）
  - `run_debug_script` 的 `route_args` 参数传递所有请求字段（用于多 IDB 路由转发）

  **Must NOT do**:
  - 不修改现有 #[tool] 方法
  - 不在 #[tool] 方法中直接构造 Python 脚本（使用 handlers/debug/ 的生成函数）
  - 方法体不超过 15 行（复杂逻辑在 handler 中）

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: 23 个方法 × 统一模式 + 正确的路由处理 + 类型安全，需要仔细确保一致性
  - **Skills**: [`rust-skills`]

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 8)
  - **Parallel Group**: Wave 3 (with Task 8)
  - **Blocks**: F1, F2, F3, F4
  - **Blocked By**: Tasks 1, 2, 3, 4, 5, 6

  **References**:

  **Pattern References**:
  - `src/server/mod.rs:4728-4792` — `run_script` #[tool] 方法实现，展示了完整的路由 + worker 调用 + 错误处理模式
  - `src/server/mod.rs:660-720` — 典型的 #[tool] 方法模式：description、instrument、Parameters 解包、Router 检查
  - `src/ida/handlers/debug/mod.rs` (Task 1) — `run_debug_script()` 辅助方法签名

  **API/Type References**:
  - `src/server/requests.rs` (Task 2) — 全部 23 个 Dbg*Request 结构体
  - `src/ida/handlers/debug/process.rs` (Task 3) — 6 个 generate_* 函数签名
  - `src/ida/handlers/debug/breakpoint.rs` (Task 4) — 4 个 generate_* 函数签名
  - `src/ida/handlers/debug/execution.rs` (Task 5) — 5 个 generate_* 函数签名
  - `src/ida/handlers/debug/inspect.rs` (Task 6) — 8 个 generate_* 函数签名

  **WHY Each Reference Matters**:
  - `run_script` 方法（第 4728 行）展示了将 `worker.run_script()` 结果转换为 CallToolResult 的精确模式
  - 现有的 #[tool] 方法展示了 `#[instrument]` 宏用法、日志格式和路由检查的必须步骤

  **Acceptance Criteria**:
  - [ ] `cargo build --release` 编译无错误
  - [ ] server/mod.rs 中存在全部 23 个 `dbg_*` 方法
  - [ ] 每个方法使用 `run_debug_script()` 辅助方法（不直接调用 worker.run_script）
  - [ ] 每个方法包含 `ServerMode::Router` 路由检查
  - [ ] 地址参数通过 `parse_address_str()` 解析

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: 全部 23 个工具方法存在
    Tool: Bash (grep)
    Preconditions: Task 7 完成
    Steps:
      1. grep -c "async fn dbg_" src/server/mod.rs
    Expected Result: 23
    Failure Indicators: 数量不等于 23
    Evidence: .sisyphus/evidence/task-7-method-count.txt

  Scenario: 路由检查无遗漏
    Tool: Bash (grep)
    Preconditions: Task 7 完成
    Steps:
      1. grep -c "ServerMode::Router" src/server/mod.rs 记录总数 A
      2. 与修改前的数量 B 对比
      3. A - B >= 23 (新增 23 个路由检查)
    Expected Result: 新增至少 23 个路由检查
    Evidence: .sisyphus/evidence/task-7-route-checks.txt

  Scenario: 编译验证
    Tool: Bash
    Preconditions: Task 7 完成
    Steps:
      1. cargo build --release 2>&1
    Expected Result: 编译成功
    Evidence: .sisyphus/evidence/task-7-build.txt
  ```

  **Commit**: YES
  - Message: `feat(debug): register 23 debug MCP tools in server and tool catalog`
  - Files: `src/server/mod.rs`
  - Pre-commit: `cargo build --release`

- [ ] 8. Tool Registry — All 23 ToolInfo Entries

  **What to do**:
  - 在 `src/tool_registry.rs` 的 `TOOL_REGISTRY` 数组中追加 23 个 `ToolInfo` 条目
  - 全部使用 `category: ToolCategory::Debug`
  - 全部设置 `default: false`（通过 `tool_catalog(category: "debug")` 发现）
  - 每个工具需要：
    - `name`: 与 #[tool] 方法名一致
    - `short_desc`: <100 字符的简短描述
    - `full_desc`: 完整描述，包含参数说明和使用示例
    - `example`: JSON 格式的调用示例
    - `keywords`: 搜索关键词（如 `["debug", "breakpoint", "software"]`）
    - `aliases`: 空（新工具无需别名）

  **示例条目**：
  ```rust
  ToolInfo {
      name: "dbg_load_debugger",
      category: ToolCategory::Debug,
      short_desc: "Load a debugger module for the current binary",
      full_desc: "Load a debugger module (e.g., 'mac', 'linux', 'gdb'). Must be called before starting a debug session. Automatically disables exception dialogs for headless operation.",
      example: r#"{"debugger_name": "mac"}"#,
      default: false,
      keywords: &["debug", "debugger", "load", "attach"],
      aliases: &[],
  },
  ```

  - keywords 需要覆盖用户可能搜索的术语：
    - 调试器加载：`debug, debugger, load`
    - 进程控制：`debug, process, start, attach, detach, exit, state, status`
    - 断点：`debug, breakpoint, bpt, break, software, hardware, watchpoint`
    - 执行：`debug, step, continue, resume, run, next, into, over, return`
    - 寄存器：`debug, register, reg, ip, sp, pc, value`
    - 内存：`debug, memory, read, write, dump, hex`
    - 线程：`debug, thread, select, list`
    - 事件：`debug, event, wait, notification`

  **Must NOT do**:
  - 不将 debug 工具设为 `default: true`
  - 不修改现有 ToolInfo 条目

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: 纯数据填写，模式统一，无复杂逻辑
  - **Skills**: [`rust-skills`]

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 7)
  - **Parallel Group**: Wave 3 (with Task 7)
  - **Blocks**: F1, F2, F3, F4
  - **Blocked By**: Tasks 3, 4, 5, 6

  **References**:

  **Pattern References**:
  - `src/tool_registry.rs:251-1274` — 现有 TOOL_REGISTRY 数组，展示了 ToolInfo 条目的精确格式
  - `src/tool_registry.rs:128-144` — ToolInfo 结构体定义

  **WHY Each Reference Matters**:
  - 必须严格遵循现有 ToolInfo 的字段格式和 keywords 风格

  **Acceptance Criteria**:
  - [ ] `cargo build --release` 编译无错误
  - [ ] TOOL_REGISTRY 中存在 23 个 `ToolCategory::Debug` 条目
  - [ ] 所有条目 `default: false`
  - [ ] 所有条目有非空 `keywords`

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Debug 工具数量验证
    Tool: Bash (grep)
    Preconditions: Task 8 完成
    Steps:
      1. grep -c "ToolCategory::Debug" src/tool_registry.rs
    Expected Result: 23
    Evidence: .sisyphus/evidence/task-8-registry-count.txt

  Scenario: 工具名称一致性
    Tool: Bash
    Preconditions: Tasks 7, 8 完成
    Steps:
      1. 提取 server/mod.rs 中的 async fn dbg_* 名称列表
      2. 提取 tool_registry.rs 中 Debug 类别的 name 字段列表
      3. 对比两个列表，应完全一致
    Expected Result: 23 个名称完全匹配
    Failure Indicators: 名称不匹配或数量不一致
    Evidence: .sisyphus/evidence/task-8-name-consistency.txt
  ```

  **Commit**: YES (group with Task 7)
  - Message: `feat(debug): register 23 debug MCP tools in server and tool catalog`
  - Files: `src/tool_registry.rs`
  - Pre-commit: `cargo build --release`

---

## Final Verification Wave (MANDATORY — after ALL implementation tasks)

> 4 review agents run in PARALLEL. ALL must APPROVE. Rejection → fix → re-run.

- [ ] F1. **Plan Compliance Audit** — `oracle`
  Read the plan end-to-end. For each "Must Have": verify implementation exists (read file, curl endpoint, run command). For each "Must NOT Have": search codebase for forbidden patterns — reject with file:line if found. Check evidence files exist in .sisyphus/evidence/. Compare deliverables against plan.
  Output: `Must Have [N/N] | Must NOT Have [N/N] | Tasks [N/N] | VERDICT: APPROVE/REJECT`

- [ ] F2. **Code Quality Review** — `unspecified-high`
  Run `cargo build --release` + `cargo clippy`. Review all changed files for: `unsafe` blocks without justification, `unwrap()` in non-test code, empty error handling, unused imports. Check AI slop: excessive comments, over-abstraction, generic names. Verify all Python script strings are valid Python syntax (test with `python3 -c "compile(..., 'exec')"` for each script generator).
  Output: `Build [PASS/FAIL] | Clippy [PASS/FAIL] | Python Syntax [N/N valid] | Files [N clean/N issues] | VERDICT`

- [ ] F3. **Integration Test — Full Debug Workflow via HTTP** — `deep`
  Start MCP server (`target/release/ida-mcp serve-http --stateless --json-response --bind 127.0.0.1:18765`). Execute this complete debug workflow via curl:
  1. `initialize` → verify server responds
  2. `open_idb(path: "target/release/ida-mcp")` → get db_handle
  3. `tool_catalog(category: "debug")` → verify all 23 tools visible
  4. `dbg_load_debugger(debugger_name: "mac")` → verify loaded
  5. `dbg_get_state()` → verify DSTATE_NOTASK
  6. `dbg_add_breakpoint(address: <entrypoint>)` → verify added
  7. `dbg_list_breakpoints()` → verify breakpoint in list
  8. `dbg_del_breakpoint(address: <entrypoint>)` → verify deleted
  9. Each tool returns valid JSON with `success` field
  Save all responses to `.sisyphus/evidence/final-qa/`.
  Output: `Tools Tested [N/23] | Workflow Steps [N/N] | JSON Valid [N/N] | VERDICT`

- [ ] F4. **Scope Fidelity Check** — `deep`
  For each task: read "What to do", read actual diff. Verify 1:1 — everything in spec was built (no missing), nothing beyond spec was built (no creep). Check "Must NOT do" compliance: no native FFI bindings, no DBG_Hooks, no remote debug, no idalib-sys changes. Flag unaccounted files.
  Output: `Tasks [N/N compliant] | Must NOT Have [CLEAN/N violations] | Unaccounted [CLEAN/N files] | VERDICT`

---

## Commit Strategy

- **Wave 1**: `feat(debug): add debug handler infrastructure and request types`
- **Wave 2**: `feat(debug): add IDAPython script generators for debug tools`
- **Wave 3**: `feat(debug): register 23 debug MCP tools in server and tool catalog`
- **Wave 4**: `test(debug): add integration test for debug workflow` (if applicable)

---

## Success Criteria

### Verification Commands
```bash
cargo build --release        # Expected: success, no errors
cargo clippy                 # Expected: no warnings in new code
# HTTP integration:
curl -s POST http://127.0.0.1:18765/mcp -H "Content-Type: application/json" -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"tool_catalog","arguments":{"category":"debug"}}}' \
  | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(json.loads(d['result']['content'][0]['text'])['tools']), 'debug tools')"
# Expected: 23 debug tools
```

### Final Checklist
- [ ] All 23 `dbg_*` tools registered and callable
- [ ] All "Must Have" present (WFNE_SILENT, state checks, unified JSON format)
- [ ] All "Must NOT Have" absent (no native FFI, no DBG_Hooks, no remote debug)
- [ ] `cargo build --release` passes
- [ ] Integration test passes (load_debugger → breakpoint CRUD → state query)
