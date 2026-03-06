# Decisions — ida-debug-tools

## [2026-03-06] Architecture Decisions
- 使用IDAPython bridge via run_script (Path A), 不做native FFI绑定
- 所有debug脚本必须包含WFNE_SILENT标志 (headless安全)
- 统一JSON输出格式: {"success": bool, "error": str|null, "data": {...}}
- 复用现有run_script基础设施, 不创建新IdaRequest变体
