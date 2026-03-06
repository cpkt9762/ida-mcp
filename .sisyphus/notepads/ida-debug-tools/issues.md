# Issues — ida-debug-tools

## [2026-03-06] Known Constraints
- WFNE_SILENT必须强制用于所有wait_for_next_event调用 (headless模式无UI)
- load_debugger前必须调用set_debugger_options(0)禁用异常对话框
- 需要暂停状态(DSTATE_SUSP=-1)的工具必须在preamble检查前置条件
- read_memory上限4096字节
