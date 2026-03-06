use crate::error::ToolError;
use serde_json::Value;

pub const HEADLESS_PREAMBLE: &str = r#"
import json, ida_dbg, ida_idd, idaapi

DSTATE_SUSP = -1
DSTATE_NOTASK = 0
DSTATE_RUN = 1

WFNE_SUSP = 0x0001
WFNE_SILENT = 0x0004
WFNE_CONT = 0x0008

def safe_hex(v):
    if v is None:
        return None
    try:
        if v == 0xFFFFFFFFFFFFFFFF:
            return None
        return hex(v)
    except Exception:
        return None

def make_result(success, data=None, error=None):
    r = {"success": success, "error": error, "data": data}
    print(json.dumps(r))
"#;

pub fn parse_debug_output(result: &Value) -> Result<Value, ToolError> {
    let stdout = result
        .get("stdout")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let Some(last_line) = stdout.lines().rev().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) else {
        return Err(ToolError::IdaError(
            "Debug script produced no JSON output".to_string(),
        ));
    };

    let parsed: Value = serde_json::from_str(last_line)
        .map_err(|_| ToolError::IdaError("Debug script produced no JSON output".to_string()))?;

    if parsed.get("success").and_then(Value::as_bool) != Some(true) {
        let error_msg = parsed
            .get("error")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| parsed.get("error").map(|v| v.to_string()))
            .unwrap_or_else(|| "Debug script failed".to_string());
        return Err(ToolError::IdaError(error_msg));
    }

    Ok(parsed)
}

pub fn build_script(body: &str) -> String {
    format!("{}\n{}", HEADLESS_PREAMBLE, body)
}

pub mod breakpoint;
pub mod execution;
pub mod inspect;
pub mod process;
