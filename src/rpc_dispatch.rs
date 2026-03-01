//! RPC dispatch layer.
//!
//! Contains the generic [`dispatch_rpc`] function that routes JSON-RPC
//! requests to the appropriate [`WorkerDispatch`] method, plus helpers.

use crate::error::ToolError;
use tracing::debug;
use crate::ida::worker_trait::WorkerDispatch;
use crate::router::protocol::RpcRequest;
use crate::server::*;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

/// Parse a JSON value as a u64 address (supports numeric and hex-string forms).
pub fn parse_address_value(v: &Value) -> Option<u64> {
    if let Some(n) = v.as_u64() {
        return Some(n);
    }
    if let Some(s) = v.as_str() {
        let s = s.trim();
        if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            return u64::from_str_radix(hex, 16).ok();
        }
        return s.parse().ok();
    }
    None
}

/// Parse a JSON value as one or more u64 addresses.
/// Accepts a single string/number OR an array of strings/numbers.
fn parse_address_values(v: &Value) -> Result<Vec<u64>, ToolError> {
    if let Some(arr) = v.as_array() {
        let mut addrs = Vec::with_capacity(arr.len());
        for (i, item) in arr.iter().enumerate() {
            addrs.push(parse_address_value(item).ok_or_else(|| {
                ToolError::InvalidAddress(format!("invalid address at index {i}: {item}"))
            })?);
        }
        if addrs.is_empty() {
            return Err(ToolError::InvalidAddress("empty address array".to_string()));
        }
        Ok(addrs)
    } else {
        let addr = parse_address_value(v)
            .ok_or_else(|| ToolError::InvalidAddress(format!("invalid address: {v}")))?;
        Ok(vec![addr])
    }
}

fn parse_params<T: DeserializeOwned>(p: &Value) -> Result<T, ToolError> {
    serde_json::from_value(p.clone())
        .map_err(|e| ToolError::InvalidParams(format!("invalid params: {e}")))
}

fn parse_string_list(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_patch_bytes(v: &Value) -> Vec<u8> {
    if let Some(arr) = v.as_array() {
        return arr
            .iter()
            .filter_map(|item| item.as_u64().map(|n| n as u8))
            .collect();
    }
    if let Some(hex_str) = v.as_str() {
        return hex_str
            .split_whitespace()
            .filter_map(|s| {
                u8::from_str_radix(
                    s.strip_prefix("0x")
                        .or_else(|| s.strip_prefix("0X"))
                        .unwrap_or(s),
                    16,
                )
                .ok()
            })
            .collect();
    }
    Vec::new()
}

/// Dispatch a JSON-RPC request to the appropriate worker method.
pub async fn dispatch_rpc<W: WorkerDispatch>(
    req: &RpcRequest,
    worker: &W,
) -> Result<Value, ToolError> {
    let p = &req.params;
    debug!(method = %req.method, params = %p, "dispatch_rpc");

    match req.method.as_str() {
        // ── Database management ──────────────────────────────────────────
        "open" => {
            let path = p["path"].as_str().unwrap_or("").to_string();
            let auto_analyse = p["auto_analyse"].as_bool().unwrap_or(false);
            let load_debug_info = p["load_debug_info"].as_bool().unwrap_or(false);
            let debug_info_path = p["debug_info_path"].as_str().map(String::from);
            let debug_info_verbose = p["debug_info_verbose"].as_bool().unwrap_or(false);
            let force = p["force"].as_bool().unwrap_or(false);
            let file_type = p["file_type"].as_str().map(String::from);
            let extra_args = p["extra_args"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let result = worker
                .open(
                    &path,
                    load_debug_info,
                    debug_info_path,
                    debug_info_verbose,
                    force,
                    file_type,
                    auto_analyse,
                    extra_args,
                )
                .await?;
            Ok(serde_json::to_value(result).unwrap_or(json!(null)))
        }
        "close" => {
            worker.close().await?;
            Ok(json!({"ok": true}))
        }
        "shutdown" => {
            worker.shutdown().await?;
            Ok(json!({"ok": true}))
        }
        "load_debug_info" => {
            let req: LoadDebugInfoRequest = parse_params(p)?;
            let path = req.path;
            let verbose = req.verbose.unwrap_or(false);
            worker.load_debug_info(path, verbose).await
        }
        "analysis_status" => {
            let r = worker.analysis_status().await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }

        // ── Functions ────────────────────────────────────────────────────
        "list_functions" => {
            let req: ListFunctionsRequest = parse_params(p)?;
            let offset = req.offset.unwrap_or(0);
            let limit = req.limit.unwrap_or(100);
            let filter = req.filter;
            let timeout = req.timeout_secs;
            let r = worker
                .list_functions(offset, limit, filter, timeout)
                .await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }
        "resolve_function" => {
            let req: ResolveFunctionRequest = parse_params(p)?;
            let r = worker.resolve_function(&req.name).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }
        "function_at" => {
            let req: FunctionAtRequest = parse_params(p)?;
            let addr = req.address.as_ref().and_then(parse_address_value);
            let name = req.target_name;
            let offset = req.offset.unwrap_or(0);
            let r = worker.function_at(addr, name, offset).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }
        "lookup_funcs" => {
            let req: LookupFuncsRequest = parse_params(p)?;
            let queries = parse_string_list(&req.queries);
            worker.lookup_funcs(queries).await
        }
        "export_funcs" => {
            let req: ExportFuncsRequest = parse_params(p)?;
            let offset = req.offset.unwrap_or(0);
            let limit = req.limit.unwrap_or(100);
            let r = worker.export_funcs(offset, limit).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }

        // ── Disassembly / Decompilation ──────────────────────────────────
        "disasm" => {
            let req: DisasmRequest = parse_params(p)?;
            let addrs = parse_address_values(&req.address)?;
            let count = req.count.unwrap_or(10);
            if addrs.len() == 1 {
                let r = worker.disasm(addrs[0], count).await?;
                Ok(json!({"disasm": r}))
            } else {
                let mut results = Vec::new();
                for addr in addrs {
                    match worker.disasm(addr, count).await {
                        Ok(text) => results.push(json!({
                            "address": format!("{:#x}", addr),
                            "disasm": text
                        })),
                        Err(e) => results.push(json!({
                            "address": format!("{:#x}", addr),
                            "error": e.to_string()
                        })),
                    }
                }
                Ok(json!({"results": results}))
            }
        }
        "disasm_by_name" => {
            let req: DisasmByNameRequest = parse_params(p)?;
            let count = req.count.unwrap_or(10);
            let r = worker.disasm_by_name(&req.name, count).await?;
            Ok(json!({"disasm": r}))
        }
        "disasm_function_at" => {
            let req: DisasmFunctionAtRequest = parse_params(p)?;
            let addr = req.address.as_ref().and_then(parse_address_value);
            let name = req.target_name;
            let offset = req.offset.unwrap_or(0);
            let count = req.count.unwrap_or(200);
            let r = worker.disasm_function_at(addr, name, offset, count).await?;
            Ok(json!({"disasm": r}))
        }
        "decompile" => {
            let req: DecompileRequest = parse_params(p)?;
            let addrs = parse_address_values(&req.address)?;
            if addrs.len() == 1 {
                let r = worker.decompile(addrs[0]).await?;
                Ok(json!({"code": r}))
            } else {
                let mut results = Vec::new();
                for addr in addrs {
                    match worker.decompile(addr).await {
                        Ok(code) => results.push(json!({
                            "address": format!("{:#x}", addr),
                            "code": code
                        })),
                        Err(e) => results.push(json!({
                            "address": format!("{:#x}", addr),
                            "error": e.to_string()
                        })),
                    }
                }
                Ok(json!({"results": results}))
            }
        }
        "pseudocode_at" => {
            let req: PseudocodeAtRequest = parse_params(p)?;
            let addr = parse_address_value(&req.address).unwrap_or(0);
            let end_addr = req
                .end_address
                .as_deref()
                .and_then(|s| parse_address_value(&Value::String(s.to_string())));
            worker.pseudocode_at(addr, end_addr).await
        }

        // ── Segments ─────────────────────────────────────────────────────
        "segments" => {
            let r = worker.segments().await?;
            Ok(serde_json::to_value(r).unwrap_or(json!([])))
        }

        // ── Strings ──────────────────────────────────────────────────────
        "strings" => {
            let req: StringsRequest = parse_params(p)?;
            let offset = req.offset.unwrap_or(0);
            let limit = req.limit.unwrap_or(100);
            let filter = req.filter;
            let timeout = req.timeout_secs;
            let r = worker.strings(offset, limit, filter, timeout).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }
        "find_string" => {
            let req: FindStringRequest = parse_params(p)?;
            let query = req.query;
            let exact = req.exact.unwrap_or(false);
            let case_insensitive = req.case_insensitive.unwrap_or(true);
            let offset = req.offset.unwrap_or(0);
            let limit = req.limit.unwrap_or(100);
            let timeout = req.timeout_secs;
            let r = worker
                .find_string(query, exact, case_insensitive, offset, limit, timeout)
                .await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }
        "analyze_strings" => {
            let req: AnalyzeStringsRequest = parse_params(p)?;
            let query = req.query;
            let offset = req.offset.unwrap_or(0);
            let limit = req.limit.unwrap_or(100);
            let timeout = req.timeout_secs;
            worker.analyze_strings(query, offset, limit, timeout).await
        }
        "xrefs_to_string" => {
            let req: XrefsToStringRequest = parse_params(p)?;
            let query = req.query;
            let exact = req.exact.unwrap_or(false);
            let case_insensitive = req.case_insensitive.unwrap_or(true);
            let offset = req.offset.unwrap_or(0);
            let limit = req.limit.unwrap_or(100);
            let max_xrefs = req.max_xrefs.unwrap_or(10);
            let timeout = req.timeout_secs;
            let r = worker
                .xrefs_to_string(
                    query,
                    exact,
                    case_insensitive,
                    offset,
                    limit,
                    max_xrefs,
                    timeout,
                )
                .await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }

        // ── Types ────────────────────────────────────────────────────────
        "local_types" => {
            let req: LocalTypesRequest = parse_params(p)?;
            let offset = req.offset.unwrap_or(0);
            let limit = req.limit.unwrap_or(100);
            let filter = req.filter;
            let timeout = req.timeout_secs;
            let r = worker.local_types(offset, limit, filter, timeout).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }
        "declare_type" => {
            let req: DeclareTypeRequest = parse_params(p)?;
            let decl = req.decl;
            let relaxed = req.relaxed.unwrap_or(false);
            let replace = req.replace.unwrap_or(false);
            let multi = req.multi.unwrap_or(false);
            worker.declare_type(decl, relaxed, replace, multi).await
        }
        "apply_types" => {
            let req: ApplyTypesRequest = parse_params(p)?;
            let addr = req.address.as_ref().and_then(parse_address_value);
            let name = req.target_name;
            let offset = req.offset.unwrap_or(0);
            let stack_offset = req.stack_offset;
            let stack_name = req.stack_name;
            let decl = req.decl;
            let type_name = req.type_name;
            let relaxed = req.relaxed.unwrap_or(false);
            let delay = req.delay.unwrap_or(false);
            let strict = req.strict.unwrap_or(false);
            worker
                .apply_types(
                    addr,
                    name,
                    offset,
                    stack_offset,
                    stack_name,
                    decl,
                    type_name,
                    relaxed,
                    delay,
                    strict,
                )
                .await
        }
        "infer_types" => {
            let req: InferTypesRequest = parse_params(p)?;
            let addr = req.address.as_ref().and_then(parse_address_value);
            let name = req.target_name;
            let offset = req.offset.unwrap_or(0);
            let r = worker.infer_types(addr, name, offset).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }

        // ── Address info ─────────────────────────────────────────────────
        "addr_info" => {
            let req: AddrInfoRequest = parse_params(p)?;
            let addr = req.address.as_ref().and_then(parse_address_value);
            let name = req.target_name;
            let offset = req.offset.unwrap_or(0);
            let r = worker.addr_info(addr, name, offset).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }

        // ── Stack ────────────────────────────────────────────────────────
        "declare_stack" => {
            let req: DeclareStackRequest = parse_params(p)?;
            let addr = req.address.as_ref().and_then(parse_address_value);
            let name = req.target_name;
            let offset = req.offset;
            let var_name = req.var_name;
            let decl = req.decl;
            let relaxed = req.relaxed.unwrap_or(false);
            let r = worker
                .declare_stack(addr, name, offset, var_name, decl, relaxed)
                .await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }
        "delete_stack" => {
            let req: DeleteStackRequest = parse_params(p)?;
            let addr = req.address.as_ref().and_then(parse_address_value);
            let name = req.target_name;
            let offset = req.offset;
            let var_name = req.var_name;
            let r = worker.delete_stack(addr, name, offset, var_name).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }
        "stack_frame" => {
            let req: AddressRequest = parse_params(p)?;
            let addr = parse_address_value(&req.address).unwrap_or(0);
            let r = worker.stack_frame(addr).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }

        // ── Structs ──────────────────────────────────────────────────────
        "structs" => {
            let req: StructsRequest = parse_params(p)?;
            let offset = req.offset.unwrap_or(0);
            let limit = req.limit.unwrap_or(100);
            let filter = req.filter;
            let timeout = req.timeout_secs;
            let r = worker.structs(offset, limit, filter, timeout).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }
        "struct_info" => {
            let req: StructInfoRequest = parse_params(p)?;
            let ordinal = req.ordinal;
            let name = req.name;
            let r = worker.struct_info(ordinal, name).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }
        "read_struct" => {
            let req: ReadStructRequest = parse_params(p)?;
            let addr = parse_address_value(&req.address).unwrap_or(0);
            let ordinal = req.ordinal;
            let name = req.name;
            let r = worker.read_struct(addr, ordinal, name).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }

        // ── Cross-references ─────────────────────────────────────────────
        "xrefs_to" => {
            let req: AddressRequest = parse_params(p)?;
            let addr = parse_address_value(&req.address).unwrap_or(0);
            let r = worker.xrefs_to(addr).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!([])))
        }
        "xrefs_from" => {
            let req: AddressRequest = parse_params(p)?;
            let addr = parse_address_value(&req.address).unwrap_or(0);
            let r = worker.xrefs_from(addr).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!([])))
        }
        "xrefs_to_field" => {
            let req: XrefsToFieldRequest = parse_params(p)?;
            let ordinal = req.ordinal;
            let name = req.name;
            let member_index = req.member_index;
            let member_name = req.member_name;
            let limit = req.limit.unwrap_or(100);
            let r = worker
                .xrefs_to_field(ordinal, name, member_index, member_name, limit)
                .await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }

        // ── Imports / Exports / Entrypoints ──────────────────────────────
        "imports" => {
            let req: PaginatedRequest = parse_params(p)?;
            let offset = req.offset.unwrap_or(0);
            let limit = req.limit.unwrap_or(100);
            let r = worker.imports(offset, limit).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!([])))
        }
        "exports" => {
            let req: PaginatedRequest = parse_params(p)?;
            let offset = req.offset.unwrap_or(0);
            let limit = req.limit.unwrap_or(100);
            let r = worker.exports(offset, limit).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!([])))
        }
        "entrypoints" => {
            let r = worker.entrypoints().await?;
            Ok(serde_json::to_value(r).unwrap_or(json!([])))
        }

        // ── Memory ───────────────────────────────────────────────────────
        "get_bytes" => {
            let req: GetBytesRequest = parse_params(p)?;
            let addr = req.address.as_ref().and_then(parse_address_value);
            let name = req.target_name;
            let offset = req.offset.unwrap_or(0);
            let size = req.size.unwrap_or(16);
            let r = worker.get_bytes(addr, name, offset, size).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!(null)))
        }
        "read_int" => {
            let addr = parse_address_value(&p["address"])
                .or_else(|| parse_address_value(&p["addr"]))
                .unwrap_or(0);
            let size = p["size"].as_u64().unwrap_or(4) as usize;
            worker.read_int(addr, size).await
        }
        "get_string" => {
            let req: GetStringRequest = parse_params(p)?;
            let addr = parse_address_value(&req.address).unwrap_or(0);
            let max_len = req.max_len.unwrap_or(1024);
            worker.get_string(addr, max_len).await
        }
        "get_global_value" => {
            let req: GetGlobalValueRequest = parse_params(p)?;
            let query = match req.query {
                Value::String(s) => s,
                other => serde_json::to_string(&other)
                    .map_err(|e| ToolError::InvalidParams(format!("invalid query: {e}")))?,
            };
            worker.get_global_value(query).await
        }

        // ── Annotations ──────────────────────────────────────────────────
        "set_comments" => {
            let req: SetCommentsRequest = parse_params(p)?;
            let addr = req.address.as_ref().and_then(parse_address_value);
            let name = req.target_name;
            let offset = req.offset.unwrap_or(0);
            let comment = req.comment;
            let repeatable = req.repeatable.unwrap_or(false);
            worker
                .set_comments(addr, name, offset, comment, repeatable)
                .await
        }
        "rename" => {
            let req: RenameRequest = parse_params(p)?;
            let addr = req.address.as_ref().and_then(parse_address_value);
            let current_name = req.current_name;
            let new_name = req.name;
            let flags = req.flags.unwrap_or(0);
            worker.rename(addr, current_name, new_name, flags).await
        }

        "rename_lvar" => {
            let req: RenameLvarRequest = parse_params(p)?;
            let func_addr = parse_address_value(&req.func_address).unwrap_or(0);
            worker
                .rename_lvar(func_addr, req.lvar_name, req.new_name)
                .await
        }

        "set_lvar_type" => {
            let req: SetLvarTypeRequest = parse_params(p)?;
            let func_addr = parse_address_value(&req.func_address).unwrap_or(0);
            worker
                .set_lvar_type(func_addr, req.lvar_name, req.type_str)
                .await
        }

        "set_decompiler_comment" => {
            let req: SetDecompilerCommentRequest = parse_params(p)?;
            let func_addr = parse_address_value(&req.func_address).unwrap_or(0);
            let addr = parse_address_value(&req.address).unwrap_or(0);
            let itp = req.itp.unwrap_or(69);
            worker
                .set_decompiler_comment(func_addr, addr, itp, req.comment)
                .await
        }

        // ── Patching ─────────────────────────────────────────────────────
        "patch_bytes" => {
            let req: PatchRequest = parse_params(p)?;
            let addr = req.address.as_ref().and_then(parse_address_value);
            let name = req.target_name;
            let offset = req.offset.unwrap_or(0);
            let bytes = parse_patch_bytes(&req.bytes);
            worker.patch_bytes(addr, name, offset, bytes).await
        }
        "patch_asm" => {
            let req: PatchAsmRequest = parse_params(p)?;
            let addr = req.address.as_ref().and_then(parse_address_value);
            let name = req.target_name;
            let offset = req.offset.unwrap_or(0);
            let line = req.line;
            worker.patch_asm(addr, name, offset, line).await
        }

        // ── Control / Call flow ──────────────────────────────────────────
        "basic_blocks" => {
            let req: AddressRequest = parse_params(p)?;
            let addr = parse_address_value(&req.address).unwrap_or(0);
            let r = worker.basic_blocks(addr).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!([])))
        }
        "callees" => {
            let req: AddressRequest = parse_params(p)?;
            let addr = parse_address_value(&req.address).unwrap_or(0);
            let r = worker.callees(addr).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!([])))
        }
        "callers" => {
            let req: AddressRequest = parse_params(p)?;
            let addr = parse_address_value(&req.address).unwrap_or(0);
            let r = worker.callers(addr).await?;
            Ok(serde_json::to_value(r).unwrap_or(json!([])))
        }
        "callgraph" => {
            let req: CallGraphRequest = parse_params(p)?;
            let addr = req
                .roots
                .as_array()
                .and_then(|roots| roots.first())
                .and_then(parse_address_value)
                .or_else(|| parse_address_value(&req.roots))
                .unwrap_or(0);
            let max_depth = req.max_depth.unwrap_or(5);
            let max_nodes = req.max_nodes.unwrap_or(100);
            worker.callgraph(addr, max_depth, max_nodes).await
        }
        "find_paths" => {
            let req: FindPathsRequest = parse_params(p)?;
            let start = parse_address_value(&req.start).unwrap_or(0);
            let end = parse_address_value(&req.end).unwrap_or(0);
            let max_paths = req.max_paths.unwrap_or(10);
            let max_depth = req.max_depth.unwrap_or(10);
            worker.find_paths(start, end, max_paths, max_depth).await
        }
        "xref_matrix" => {
            let req: XrefMatrixRequest = parse_params(p)?;
            let addrs = req
                .addrs
                .as_array()
                .map(|a| a.iter().filter_map(parse_address_value).collect())
                .unwrap_or_default();
            worker.xref_matrix(addrs).await
        }

        // ── Metadata ─────────────────────────────────────────────────────
        "idb_meta" => worker.idb_meta().await,

        // ── Globals ──────────────────────────────────────────────────────
        "list_globals" => {
            let req: ListGlobalsRequest = parse_params(p)?;
            let query = req.query;
            let offset = req.offset.unwrap_or(0);
            let limit = req.limit.unwrap_or(100);
            let timeout = req.timeout_secs;
            worker.list_globals(query, offset, limit, timeout).await
        }

        // ── Analysis ─────────────────────────────────────────────────────
        "analyze_funcs" => {
            let req: AnalyzeFuncsRequest = parse_params(p)?;
            let timeout = req.timeout_secs;
            worker.analyze_funcs(timeout).await
        }

        // ── Search ───────────────────────────────────────────────────────
        "find_bytes" => {
            let req: FindBytesRequest = parse_params(p)?;
            let patterns: Vec<String> = if let Some(arr) = req.patterns.as_array() {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            } else if let Some(s) = req.patterns.as_str() {
                // Check if it's a JSON array string
                if s.trim().starts_with('[') {
                    if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str(s) {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    } else {
                        vec![s.to_string()]
                    }
                } else {
                    vec![s.to_string()]
                }
            } else {
                vec![]
            };
            let max_results = req.limit.unwrap_or(100);
            let timeout = req.timeout_secs;

            // Process each pattern and merge results
            if patterns.len() == 1 {
                // Single pattern: return result directly
                worker
                    .find_bytes(patterns.into_iter().next().unwrap(), max_results, timeout)
                    .await
            } else if patterns.is_empty() {
                Ok(json!({"matches": [], "count": 0}))
            } else {
                // Multiple patterns: merge matches
                let mut all_matches = Vec::new();
                for pattern in patterns {
                    if let Ok(result) = worker.find_bytes(pattern, max_results, timeout).await {
                        if let Some(matches) = result.get("matches").and_then(|m| m.as_array()) {
                            all_matches.extend(matches.clone());
                        }
                    }
                }
                let count = all_matches.len();
                Ok(json!({"matches": all_matches, "count": count}))
            }
        }
        "search_text" => {
            let text = p["text"].as_str().unwrap_or("").to_string();
            let max_results = p["max_results"].as_u64().unwrap_or(100) as usize;
            let timeout = p["timeout_secs"].as_u64();
            worker.search_text(text, max_results, timeout).await
        }
        "search_imm" => {
            let imm = p["imm"].as_u64().unwrap_or(0);
            let max_results = p["max_results"].as_u64().unwrap_or(100) as usize;
            let timeout = p["timeout_secs"].as_u64();
            worker.search_imm(imm, max_results, timeout).await
        }
        "find_insns" => {
            let req: FindInsnsRequest = parse_params(p)?;
            let patterns: Vec<String> = if let Some(arr) = req.patterns.as_array() {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            } else if let Some(s) = req.patterns.as_str() {
                if s.trim().starts_with('[') {
                    if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str(s) {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    } else {
                        vec![s.to_string()]
                    }
                } else {
                    vec![s.to_string()]
                }
            } else {
                vec![]
            };
            let max_results = req.limit.unwrap_or(100);
            let case_insensitive = req.case_insensitive.unwrap_or(true);
            let timeout = req.timeout_secs;
            worker
                .find_insns(patterns, max_results, case_insensitive, timeout)
                .await
        }
        "find_insn_operands" => {
            let req: FindInsnOperandsRequest = parse_params(p)?;
            let patterns: Vec<String> = if let Some(arr) = req.patterns.as_array() {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            } else if let Some(s) = req.patterns.as_str() {
                if s.trim().starts_with('[') {
                    if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str(s) {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    } else {
                        vec![s.to_string()]
                    }
                } else {
                    vec![s.to_string()]
                }
            } else {
                vec![]
            };
            let max_results = req.limit.unwrap_or(100);
            let case_insensitive = req.case_insensitive.unwrap_or(true);
            let timeout = req.timeout_secs;
            worker
                .find_insn_operands(patterns, max_results, case_insensitive, timeout)
                .await
        }

        // ── IDAPython script ─────────────────────────────────────────────
        "run_script" => {
            let req: RunScriptRequest = parse_params(p)?;
            let code = req.code.unwrap_or_default();
            let timeout = req.timeout_secs;
            worker.run_script(&code, timeout).await
        }

        "batch_decompile" => {
            let req: BatchDecompileRequest = parse_params(p)?;
            let addrs: Vec<Value> = if let Some(arr) = req.addresses.as_array() {
                arr.clone()
            } else if let Some(s) = req.addresses.as_str() {
                serde_json::from_str(s).unwrap_or_else(|_| vec![req.addresses.clone()])
            } else {
                vec![req.addresses.clone()]
            };

            let mut results = Vec::new();
            for addr_val in &addrs {
                let addr = match crate::rpc_dispatch::parse_address_value(addr_val) {
                    Some(a) => a,
                    None => {
                        results.push(json!({
                            "address": addr_val,
                            "error": "invalid address",
                            "success": false,
                        }));
                        continue;
                    }
                };

                match worker.decompile(addr).await {
                    Ok(code) => results.push(json!({
                        "address": addr_val,
                        "pseudocode": code,
                        "success": true,
                    })),
                    Err(e) => results.push(json!({
                        "address": addr_val,
                        "error": e.to_string(),
                        "success": false,
                    })),
                }
            }
            Ok(json!(results))
        }

        "search_pseudocode" => {
            let req: SearchPseudocodeRequest = parse_params(p)?;
            let pattern = req.pattern;
            let limit = req.limit.unwrap_or(20).min(100);
            let timeout = req.timeout_secs;

            let funcs = worker.list_functions(0, 10000, None, timeout).await?;
            let mut matches = Vec::new();
            for func in &funcs.functions {
                if matches.len() >= limit {
                    break;
                }
                let addr =
                    u64::from_str_radix(func.address.trim_start_matches("0x"), 16).unwrap_or(0);
                if let Ok(result) = worker.decompile(addr).await {
                    if result.contains(&pattern) {
                        matches.push(json!({
                            "address": func.address,
                            "name": func.name,
                            "pseudocode": result,
                        }));
                    }
                }
            }
            Ok(json!({
                "pattern": pattern,
                "matches": matches,
                "total_searched": funcs.functions.len(),
            }))
        }

        "table_scan" => {
            let req: TableScanRequest = parse_params(p)?;
            let base_addr = crate::rpc_dispatch::parse_address_value(&req.base_address)
                .ok_or_else(|| ToolError::InvalidParams("base_address is required".to_string()))?;
            let stride = req.stride.unwrap_or(8).max(1);
            let count = req.count.unwrap_or(16).min(256);

            let mut entries = Vec::new();
            for i in 0..count {
                let offset = (i as u64) * stride;
                let addr = base_addr + offset;
                match worker.get_bytes(Some(addr), None, 0, stride as usize).await {
                    Ok(r) => {
                        entries.push(json!({
                            "index": i,
                            "address": format!("0x{:x}", addr),
                            "bytes": r.bytes,
                        }));
                    }
                    Err(e) => {
                        entries.push(json!({
                            "index": i,
                            "address": format!("0x{:x}", addr),
                            "error": e.to_string(),
                        }));
                        break;
                    }
                }
            }
            Ok(json!({
                "base_address": format!("0x{:x}", base_addr),
                "stride": stride,
                "count": entries.len(),
                "entries": entries,
            }))
        }

        "diff_functions" => {
            let req: DiffFunctionsRequest = parse_params(p)?;
            let addr1 = crate::rpc_dispatch::parse_address_value(&req.addr1)
                .ok_or_else(|| ToolError::InvalidParams("addr1 is required".to_string()))?;
            let addr2 = crate::rpc_dispatch::parse_address_value(&req.addr2)
                .ok_or_else(|| ToolError::InvalidParams("addr2 is required".to_string()))?;

            let result1 = worker.decompile(addr1).await?;
            let result2 = worker.decompile(addr2).await?;

            let lines1: Vec<&str> = result1.lines().collect();
            let lines2: Vec<&str> = result2.lines().collect();

            let mut diff_lines = Vec::new();
            let max_len = lines1.len().max(lines2.len());
            let mut same = 0usize;
            let mut different = 0usize;

            for i in 0..max_len {
                match (lines1.get(i), lines2.get(i)) {
                    (Some(l1), Some(l2)) => {
                        if l1 == l2 {
                            diff_lines.push(format!("  {}", l1));
                            same += 1;
                        } else {
                            diff_lines.push(format!("- {}", l1));
                            diff_lines.push(format!("+ {}", l2));
                            different += 1;
                        }
                    }
                    (Some(l1), None) => {
                        diff_lines.push(format!("- {}", l1));
                        different += 1;
                    }
                    (None, Some(l2)) => {
                        diff_lines.push(format!("+ {}", l2));
                        different += 1;
                    }
                    (None, None) => {}
                }
            }

            let total = same + different;
            let similarity = if total == 0 {
                1.0f64
            } else {
                same as f64 / total as f64
            };

            Ok(json!({
                "function1": result1,
                "function2": result2,
                "similarity_ratio": (similarity * 100.0).round() / 100.0,
                "diff_lines": diff_lines,
            }))
        }

        // ── Unknown method ───────────────────────────────────────────────
        _ => Err(ToolError::InvalidToolName(format!(
            "Unknown method: {}",
            req.method
        ))),
    }
}

#[cfg(test)]
pub mod mock {
    use crate::error::ToolError;
    use crate::ida::types::*;
    use crate::ida::worker_trait::WorkerDispatch;
    use serde_json::{json, Value};
    use std::sync::Mutex;

    /// A test-only worker that records every call and returns default values.
    ///
    /// Each invocation pushes `(method_name, json!({params}))` into `self.calls`.
    pub struct MockWorker {
        pub calls: Mutex<Vec<(String, Value)>>,
    }

    impl MockWorker {
        pub fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn record(&self, method: &str, params: Value) {
            self.calls
                .lock()
                .unwrap()
                .push((method.to_string(), params));
        }
    }

    impl Default for MockWorker {
        fn default() -> Self {
            Self::new()
        }
    }

    // Helper: construct default return values for IDA types that don't derive Default.

    fn default_db_info() -> DbInfo {
        DbInfo {
            path: String::new(),
            file_type: String::new(),
            processor: String::new(),
            bits: 0,
            function_count: 0,
            debug_info: None,
            analysis_status: default_analysis_status(),
        }
    }

    fn default_analysis_status() -> AnalysisStatus {
        AnalysisStatus {
            auto_enabled: false,
            auto_is_ok: true,
            auto_state: String::new(),
            auto_state_id: 0,
            analysis_running: false,
        }
    }

    fn default_function_list() -> FunctionListResult {
        FunctionListResult {
            functions: vec![],
            total: 0,
            next_offset: None,
        }
    }

    fn default_function_info() -> FunctionInfo {
        FunctionInfo {
            address: String::new(),
            name: String::new(),
            size: 0,
        }
    }

    fn default_function_range() -> FunctionRangeInfo {
        FunctionRangeInfo {
            address: String::new(),
            name: String::new(),
            start: String::new(),
            end: String::new(),
            size: 0,
        }
    }

    fn default_string_list() -> StringListResult {
        StringListResult {
            strings: vec![],
            total: 0,
            next_offset: None,
        }
    }

    fn default_string_xrefs() -> StringXrefsResult {
        StringXrefsResult {
            strings: vec![],
            total: 0,
            next_offset: None,
        }
    }

    fn default_local_type_list() -> LocalTypeListResult {
        LocalTypeListResult {
            types: vec![],
            total: 0,
            next_offset: None,
        }
    }

    fn default_guess_type() -> GuessTypeResult {
        GuessTypeResult {
            address: String::new(),
            code: 0,
            status: String::new(),
            decl: String::new(),
            kind: String::new(),
        }
    }

    fn default_address_info() -> AddressInfo {
        AddressInfo {
            address: String::new(),
            segment: None,
            function: None,
            symbol: None,
        }
    }

    fn default_stack_var() -> StackVarResult {
        StackVarResult {
            function: String::new(),
            name: String::new(),
            offset: 0,
            code: 0,
            status: String::new(),
        }
    }

    fn default_frame_info() -> FrameInfo {
        FrameInfo {
            address: String::new(),
            frame_size: 0,
            ret_size: 0,
            frsize: 0,
            frregs: 0,
            argsize: 0,
            fpd: 0,
            args_range: FrameRange {
                start: String::new(),
                end: String::new(),
            },
            retaddr_range: FrameRange {
                start: String::new(),
                end: String::new(),
            },
            savregs_range: FrameRange {
                start: String::new(),
                end: String::new(),
            },
            locals_range: FrameRange {
                start: String::new(),
                end: String::new(),
            },
            member_count: 0,
            members: vec![],
        }
    }

    fn default_struct_list() -> StructListResult {
        StructListResult {
            structs: vec![],
            total: 0,
            next_offset: None,
        }
    }

    fn default_struct_info() -> StructInfo {
        StructInfo {
            ordinal: 0,
            name: String::new(),
            size: 0,
            is_union: false,
            member_count: 0,
            members: vec![],
        }
    }

    fn default_struct_read() -> StructReadResult {
        StructReadResult {
            address: String::new(),
            ordinal: 0,
            name: String::new(),
            size: 0,
            members: vec![],
        }
    }

    fn default_xrefs_to_field() -> XrefsToFieldResult {
        XrefsToFieldResult {
            struct_ordinal: 0,
            struct_name: String::new(),
            member_index: 0,
            member_name: String::new(),
            member_type: String::new(),
            member_offset_bits: 0,
            member_size_bits: 0,
            tid: String::new(),
            xrefs: vec![],
            truncated: false,
        }
    }

    fn default_bytes() -> BytesResult {
        BytesResult {
            address: String::new(),
            bytes: String::new(),
            length: 0,
        }
    }

    impl WorkerDispatch for MockWorker {
        async fn open(
            &self,
            path: &str,
            load_debug_info: bool,
            debug_info_path: Option<String>,
            debug_info_verbose: bool,
            force: bool,
            file_type: Option<String>,
            auto_analyse: bool,
            extra_args: Vec<String>,
        ) -> Result<DbInfo, ToolError> {
            self.record(
                "open",
                json!({
                    "path": path,
                    "load_debug_info": load_debug_info,
                    "debug_info_path": debug_info_path,
                    "debug_info_verbose": debug_info_verbose,
                    "force": force,
                    "file_type": file_type,
                    "auto_analyse": auto_analyse,
                    "extra_args": extra_args,
                }),
            );
            Ok(default_db_info())
        }

        async fn close(&self) -> Result<(), ToolError> {
            self.record("close", json!({}));
            Ok(())
        }

        async fn shutdown(&self) -> Result<(), ToolError> {
            self.record("shutdown", json!({}));
            Ok(())
        }

        async fn load_debug_info(
            &self,
            path: Option<String>,
            verbose: bool,
        ) -> Result<Value, ToolError> {
            self.record("load_debug_info", json!({"path": path, "verbose": verbose}));
            Ok(json!({}))
        }

        async fn analysis_status(&self) -> Result<AnalysisStatus, ToolError> {
            self.record("analysis_status", json!({}));
            Ok(default_analysis_status())
        }

        async fn list_functions(
            &self,
            offset: usize,
            limit: usize,
            filter: Option<String>,
            timeout_secs: Option<u64>,
        ) -> Result<FunctionListResult, ToolError> {
            self.record("list_functions", json!({
                "offset": offset, "limit": limit, "filter": filter, "timeout_secs": timeout_secs,
            }));
            Ok(default_function_list())
        }

        async fn resolve_function(&self, name: &str) -> Result<FunctionInfo, ToolError> {
            self.record("resolve_function", json!({"name": name}));
            Ok(default_function_info())
        }

        async fn function_at(
            &self,
            addr: Option<u64>,
            name: Option<String>,
            offset: u64,
        ) -> Result<FunctionRangeInfo, ToolError> {
            self.record(
                "function_at",
                json!({"addr": addr, "name": name, "offset": offset}),
            );
            Ok(default_function_range())
        }

        async fn lookup_funcs(&self, queries: Vec<String>) -> Result<Value, ToolError> {
            self.record("lookup_funcs", json!({"queries": queries}));
            Ok(json!({}))
        }

        async fn export_funcs(
            &self,
            offset: usize,
            limit: usize,
        ) -> Result<FunctionListResult, ToolError> {
            self.record("export_funcs", json!({"offset": offset, "limit": limit}));
            Ok(default_function_list())
        }

        async fn disasm(&self, addr: u64, count: usize) -> Result<String, ToolError> {
            self.record("disasm", json!({"addr": addr, "count": count}));
            Ok(String::new())
        }

        async fn disasm_by_name(&self, name: &str, count: usize) -> Result<String, ToolError> {
            self.record("disasm_by_name", json!({"name": name, "count": count}));
            Ok(String::new())
        }

        async fn disasm_function_at(
            &self,
            addr: Option<u64>,
            name: Option<String>,
            offset: u64,
            count: usize,
        ) -> Result<String, ToolError> {
            self.record(
                "disasm_function_at",
                json!({
                    "addr": addr, "name": name, "offset": offset, "count": count,
                }),
            );
            Ok(String::new())
        }

        async fn decompile(&self, addr: u64) -> Result<String, ToolError> {
            self.record("decompile", json!({"addr": addr}));
            Ok(String::new())
        }

        async fn pseudocode_at(
            &self,
            addr: u64,
            end_addr: Option<u64>,
        ) -> Result<Value, ToolError> {
            self.record("pseudocode_at", json!({"addr": addr, "end_addr": end_addr}));
            Ok(json!({}))
        }

        async fn segments(&self) -> Result<Vec<SegmentInfo>, ToolError> {
            self.record("segments", json!({}));
            Ok(vec![])
        }

        async fn strings(
            &self,
            offset: usize,
            limit: usize,
            filter: Option<String>,
            timeout_secs: Option<u64>,
        ) -> Result<StringListResult, ToolError> {
            self.record("strings", json!({
                "offset": offset, "limit": limit, "filter": filter, "timeout_secs": timeout_secs,
            }));
            Ok(default_string_list())
        }

        async fn find_string(
            &self,
            query: String,
            exact: bool,
            case_insensitive: bool,
            offset: usize,
            limit: usize,
            timeout_secs: Option<u64>,
        ) -> Result<StringListResult, ToolError> {
            self.record(
                "find_string",
                json!({
                    "query": query, "exact": exact, "case_insensitive": case_insensitive,
                    "offset": offset, "limit": limit, "timeout_secs": timeout_secs,
                }),
            );
            Ok(default_string_list())
        }

        async fn analyze_strings(
            &self,
            query: Option<String>,
            offset: usize,
            limit: usize,
            timeout_secs: Option<u64>,
        ) -> Result<Value, ToolError> {
            self.record(
                "analyze_strings",
                json!({
                    "query": query, "offset": offset, "limit": limit, "timeout_secs": timeout_secs,
                }),
            );
            Ok(json!({}))
        }

        async fn xrefs_to_string(
            &self,
            query: String,
            exact: bool,
            case_insensitive: bool,
            offset: usize,
            limit: usize,
            max_xrefs: usize,
            timeout_secs: Option<u64>,
        ) -> Result<StringXrefsResult, ToolError> {
            self.record(
                "xrefs_to_string",
                json!({
                    "query": query, "exact": exact, "case_insensitive": case_insensitive,
                    "offset": offset, "limit": limit, "max_xrefs": max_xrefs,
                    "timeout_secs": timeout_secs,
                }),
            );
            Ok(default_string_xrefs())
        }

        async fn local_types(
            &self,
            offset: usize,
            limit: usize,
            filter: Option<String>,
            timeout_secs: Option<u64>,
        ) -> Result<LocalTypeListResult, ToolError> {
            self.record("local_types", json!({
                "offset": offset, "limit": limit, "filter": filter, "timeout_secs": timeout_secs,
            }));
            Ok(default_local_type_list())
        }

        async fn declare_type(
            &self,
            decl: String,
            relaxed: bool,
            replace: bool,
            multi: bool,
        ) -> Result<Value, ToolError> {
            self.record(
                "declare_type",
                json!({
                    "decl": decl, "relaxed": relaxed, "replace": replace, "multi": multi,
                }),
            );
            Ok(json!({}))
        }

        async fn apply_types(
            &self,
            addr: Option<u64>,
            name: Option<String>,
            offset: u64,
            stack_offset: Option<i64>,
            stack_name: Option<String>,
            decl: Option<String>,
            type_name: Option<String>,
            relaxed: bool,
            delay: bool,
            strict: bool,
        ) -> Result<Value, ToolError> {
            self.record(
                "apply_types",
                json!({
                    "addr": addr, "name": name, "offset": offset,
                    "stack_offset": stack_offset, "stack_name": stack_name,
                    "decl": decl, "type_name": type_name,
                    "relaxed": relaxed, "delay": delay, "strict": strict,
                }),
            );
            Ok(json!({}))
        }

        async fn infer_types(
            &self,
            addr: Option<u64>,
            name: Option<String>,
            offset: u64,
        ) -> Result<GuessTypeResult, ToolError> {
            self.record(
                "infer_types",
                json!({"addr": addr, "name": name, "offset": offset}),
            );
            Ok(default_guess_type())
        }

        async fn addr_info(
            &self,
            addr: Option<u64>,
            name: Option<String>,
            offset: u64,
        ) -> Result<AddressInfo, ToolError> {
            self.record(
                "addr_info",
                json!({"addr": addr, "name": name, "offset": offset}),
            );
            Ok(default_address_info())
        }

        async fn declare_stack(
            &self,
            addr: Option<u64>,
            name: Option<String>,
            offset: i64,
            var_name: Option<String>,
            decl: String,
            relaxed: bool,
        ) -> Result<StackVarResult, ToolError> {
            self.record(
                "declare_stack",
                json!({
                    "addr": addr, "name": name, "offset": offset,
                    "var_name": var_name, "decl": decl, "relaxed": relaxed,
                }),
            );
            Ok(default_stack_var())
        }

        async fn delete_stack(
            &self,
            addr: Option<u64>,
            name: Option<String>,
            offset: Option<i64>,
            var_name: Option<String>,
        ) -> Result<StackVarResult, ToolError> {
            self.record(
                "delete_stack",
                json!({
                    "addr": addr, "name": name, "offset": offset, "var_name": var_name,
                }),
            );
            Ok(default_stack_var())
        }

        async fn stack_frame(&self, addr: u64) -> Result<FrameInfo, ToolError> {
            self.record("stack_frame", json!({"addr": addr}));
            Ok(default_frame_info())
        }

        async fn structs(
            &self,
            offset: usize,
            limit: usize,
            filter: Option<String>,
            timeout_secs: Option<u64>,
        ) -> Result<StructListResult, ToolError> {
            self.record("structs", json!({
                "offset": offset, "limit": limit, "filter": filter, "timeout_secs": timeout_secs,
            }));
            Ok(default_struct_list())
        }

        async fn struct_info(
            &self,
            ordinal: Option<u32>,
            name: Option<String>,
        ) -> Result<StructInfo, ToolError> {
            self.record("struct_info", json!({"ordinal": ordinal, "name": name}));
            Ok(default_struct_info())
        }

        async fn read_struct(
            &self,
            addr: u64,
            ordinal: Option<u32>,
            name: Option<String>,
        ) -> Result<StructReadResult, ToolError> {
            self.record(
                "read_struct",
                json!({"addr": addr, "ordinal": ordinal, "name": name}),
            );
            Ok(default_struct_read())
        }

        async fn xrefs_to(&self, addr: u64) -> Result<Vec<XRefInfo>, ToolError> {
            self.record("xrefs_to", json!({"addr": addr}));
            Ok(vec![])
        }

        async fn xrefs_from(&self, addr: u64) -> Result<Vec<XRefInfo>, ToolError> {
            self.record("xrefs_from", json!({"addr": addr}));
            Ok(vec![])
        }

        async fn xrefs_to_field(
            &self,
            ordinal: Option<u32>,
            name: Option<String>,
            member_index: Option<u32>,
            member_name: Option<String>,
            limit: usize,
        ) -> Result<XrefsToFieldResult, ToolError> {
            self.record(
                "xrefs_to_field",
                json!({
                    "ordinal": ordinal, "name": name,
                    "member_index": member_index, "member_name": member_name, "limit": limit,
                }),
            );
            Ok(default_xrefs_to_field())
        }

        async fn imports(&self, offset: usize, limit: usize) -> Result<Vec<ImportInfo>, ToolError> {
            self.record("imports", json!({"offset": offset, "limit": limit}));
            Ok(vec![])
        }

        async fn exports(&self, offset: usize, limit: usize) -> Result<Vec<ExportInfo>, ToolError> {
            self.record("exports", json!({"offset": offset, "limit": limit}));
            Ok(vec![])
        }

        async fn entrypoints(&self) -> Result<Vec<String>, ToolError> {
            self.record("entrypoints", json!({}));
            Ok(vec![])
        }

        async fn get_bytes(
            &self,
            addr: Option<u64>,
            name: Option<String>,
            offset: u64,
            size: usize,
        ) -> Result<BytesResult, ToolError> {
            self.record(
                "get_bytes",
                json!({
                    "addr": addr, "name": name, "offset": offset, "size": size,
                }),
            );
            Ok(default_bytes())
        }

        async fn read_int(&self, addr: u64, size: usize) -> Result<Value, ToolError> {
            self.record("read_int", json!({"addr": addr, "size": size}));
            Ok(json!({}))
        }

        async fn get_string(&self, addr: u64, max_len: usize) -> Result<Value, ToolError> {
            self.record("get_string", json!({"addr": addr, "max_len": max_len}));
            Ok(json!({}))
        }

        async fn get_global_value(&self, query: String) -> Result<Value, ToolError> {
            self.record("get_global_value", json!({"query": query}));
            Ok(json!({}))
        }

        async fn set_comments(
            &self,
            addr: Option<u64>,
            name: Option<String>,
            offset: u64,
            comment: String,
            repeatable: bool,
        ) -> Result<Value, ToolError> {
            self.record(
                "set_comments",
                json!({
                    "addr": addr, "name": name, "offset": offset,
                    "comment": comment, "repeatable": repeatable,
                }),
            );
            Ok(json!({}))
        }

        async fn rename(
            &self,
            addr: Option<u64>,
            current_name: Option<String>,
            new_name: String,
            flags: i32,
        ) -> Result<Value, ToolError> {
            self.record("rename", json!({
                "addr": addr, "current_name": current_name, "new_name": new_name, "flags": flags,
            }));
            Ok(json!({}))
        }

        async fn rename_lvar(
            &self,
            func_addr: u64,
            lvar_name: String,
            new_name: String,
        ) -> Result<Value, ToolError> {
            self.record("rename_lvar", json!({
                "func_addr": func_addr, "lvar_name": lvar_name, "new_name": new_name,
            }));
            Ok(json!({}))
        }

        async fn set_lvar_type(
            &self,
            func_addr: u64,
            lvar_name: String,
            type_str: String,
        ) -> Result<Value, ToolError> {
            self.record("set_lvar_type", json!({
                "func_addr": func_addr, "lvar_name": lvar_name, "type_str": type_str,
            }));
            Ok(json!({}))
        }

        async fn set_decompiler_comment(
            &self,
            func_addr: u64,
            addr: u64,
            itp: i32,
            comment: String,
        ) -> Result<Value, ToolError> {
            self.record("set_decompiler_comment", json!({
                "func_addr": func_addr, "addr": addr, "itp": itp, "comment": comment,
            }));
            Ok(json!({}))
        }

        async fn patch_bytes(
            &self,
            addr: Option<u64>,
            name: Option<String>,
            offset: u64,
            bytes: Vec<u8>,
        ) -> Result<Value, ToolError> {
            self.record(
                "patch_bytes",
                json!({
                    "addr": addr, "name": name, "offset": offset, "bytes": bytes,
                }),
            );
            Ok(json!({}))
        }

        async fn patch_asm(
            &self,
            addr: Option<u64>,
            name: Option<String>,
            offset: u64,
            line: String,
        ) -> Result<Value, ToolError> {
            self.record(
                "patch_asm",
                json!({
                    "addr": addr, "name": name, "offset": offset, "line": line,
                }),
            );
            Ok(json!({}))
        }

        async fn basic_blocks(&self, addr: u64) -> Result<Vec<BasicBlockInfo>, ToolError> {
            self.record("basic_blocks", json!({"addr": addr}));
            Ok(vec![])
        }

        async fn callees(&self, addr: u64) -> Result<Vec<FunctionInfo>, ToolError> {
            self.record("callees", json!({"addr": addr}));
            Ok(vec![])
        }

        async fn callers(&self, addr: u64) -> Result<Vec<FunctionInfo>, ToolError> {
            self.record("callers", json!({"addr": addr}));
            Ok(vec![])
        }

        async fn callgraph(
            &self,
            addr: u64,
            max_depth: usize,
            max_nodes: usize,
        ) -> Result<Value, ToolError> {
            self.record(
                "callgraph",
                json!({
                    "addr": addr, "max_depth": max_depth, "max_nodes": max_nodes,
                }),
            );
            Ok(json!({}))
        }

        async fn find_paths(
            &self,
            start: u64,
            end: u64,
            max_paths: usize,
            max_depth: usize,
        ) -> Result<Value, ToolError> {
            self.record(
                "find_paths",
                json!({
                    "start": start, "end": end, "max_paths": max_paths, "max_depth": max_depth,
                }),
            );
            Ok(json!({}))
        }

        async fn xref_matrix(&self, addrs: Vec<u64>) -> Result<Value, ToolError> {
            self.record("xref_matrix", json!({"addrs": addrs}));
            Ok(json!({}))
        }

        async fn idb_meta(&self) -> Result<Value, ToolError> {
            self.record("idb_meta", json!({}));
            Ok(json!({}))
        }

        async fn list_globals(
            &self,
            query: Option<String>,
            offset: usize,
            limit: usize,
            timeout_secs: Option<u64>,
        ) -> Result<Value, ToolError> {
            self.record(
                "list_globals",
                json!({
                    "query": query, "offset": offset, "limit": limit, "timeout_secs": timeout_secs,
                }),
            );
            Ok(json!({}))
        }

        async fn analyze_funcs(&self, timeout_secs: Option<u64>) -> Result<Value, ToolError> {
            self.record("analyze_funcs", json!({"timeout_secs": timeout_secs}));
            Ok(json!({}))
        }

        async fn find_bytes(
            &self,
            pattern: String,
            max_results: usize,
            timeout_secs: Option<u64>,
        ) -> Result<Value, ToolError> {
            self.record(
                "find_bytes",
                json!({
                    "pattern": pattern, "max_results": max_results, "timeout_secs": timeout_secs,
                }),
            );
            Ok(json!({}))
        }

        async fn search_text(
            &self,
            text: String,
            max_results: usize,
            timeout_secs: Option<u64>,
        ) -> Result<Value, ToolError> {
            self.record(
                "search_text",
                json!({
                    "text": text, "max_results": max_results, "timeout_secs": timeout_secs,
                }),
            );
            Ok(json!({}))
        }

        async fn search_imm(
            &self,
            imm: u64,
            max_results: usize,
            timeout_secs: Option<u64>,
        ) -> Result<Value, ToolError> {
            self.record(
                "search_imm",
                json!({
                    "imm": imm, "max_results": max_results, "timeout_secs": timeout_secs,
                }),
            );
            Ok(json!({}))
        }

        async fn find_insns(
            &self,
            patterns: Vec<String>,
            max_results: usize,
            case_insensitive: bool,
            timeout_secs: Option<u64>,
        ) -> Result<Value, ToolError> {
            self.record(
                "find_insns",
                json!({
                    "patterns": patterns, "max_results": max_results,
                    "case_insensitive": case_insensitive, "timeout_secs": timeout_secs,
                }),
            );
            Ok(json!({}))
        }

        async fn find_insn_operands(
            &self,
            patterns: Vec<String>,
            max_results: usize,
            case_insensitive: bool,
            timeout_secs: Option<u64>,
        ) -> Result<Value, ToolError> {
            self.record(
                "find_insn_operands",
                json!({
                    "patterns": patterns, "max_results": max_results,
                    "case_insensitive": case_insensitive, "timeout_secs": timeout_secs,
                }),
            );
            Ok(json!({}))
        }

        async fn run_script(
            &self,
            code: &str,
            timeout_secs: Option<u64>,
        ) -> Result<Value, ToolError> {
            self.record(
                "run_script",
                json!({"code": code, "timeout_secs": timeout_secs}),
            );
            Ok(json!({}))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn mock_records_calls() {
            let mock = MockWorker::new();
            let _ = mock.analysis_status().await;
            let _ = mock.segments().await;
            let _ = mock.disasm(0x1000, 10).await;

            let calls = mock.calls.lock().unwrap();
            assert_eq!(calls.len(), 3);
            assert_eq!(calls[0].0, "analysis_status");
            assert_eq!(calls[1].0, "segments");
            assert_eq!(calls[2].0, "disasm");
            assert_eq!(calls[2].1["addr"], 0x1000);
            assert_eq!(calls[2].1["count"], 10);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_address_value;
    use super::{dispatch_rpc, mock::MockWorker};
    use crate::error::ToolError;
    use crate::router::protocol::RpcRequest;
    use serde_json::json;

    #[test]
    fn test_parse_address_value_hex_string() {
        let v = json!("0x5e8");
        let result = parse_address_value(&v);
        assert_eq!(result, Some(0x5e8));
    }

    #[test]
    fn test_parse_address_value_decimal_string() {
        let v = json!("1512");
        let result = parse_address_value(&v);
        assert_eq!(result, Some(1512));
    }

    #[test]
    fn test_parse_address_value_number() {
        let v = json!(0x5e8u64);
        let result = parse_address_value(&v);
        assert_eq!(result, Some(0x5e8));
    }

    #[test]
    fn test_parse_address_value_invalid() {
        let v = json!("invalid");
        let result = parse_address_value(&v);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_address_value_null() {
        let v = json!(null);
        let result = parse_address_value(&v);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_pattern_array() {
        let patterns_json = json!(["FD 7B", "90 90"]);
        let patterns: Vec<String> = if let Some(arr) = patterns_json.as_array() {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        } else {
            vec![]
        };
        assert_eq!(patterns, vec!["FD 7B", "90 90"]);
    }

    #[test]
    fn test_parse_pattern_string() {
        let patterns_json = json!("FD 7B");
        let patterns: Vec<String> = if let Some(s) = patterns_json.as_str() {
            vec![s.to_string()]
        } else {
            vec![]
        };
        assert_eq!(patterns, vec!["FD 7B"]);
    }

    #[test]
    fn test_parse_pattern_json_array_string() {
        let patterns_json = json!(r#"["FD 7B", "90 90"]"#);
        let patterns: Vec<String> = if let Some(s) = patterns_json.as_str() {
            if s.trim().starts_with('[') {
                if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str(s) {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                } else {
                    vec![s.to_string()]
                }
            } else {
                vec![s.to_string()]
            }
        } else {
            vec![]
        };
        assert_eq!(patterns, vec!["FD 7B", "90 90"]);
    }

    #[test]
    fn test_parse_insn_patterns_string() {
        let patterns_json = json!("bl");
        let patterns: Vec<String> = if let Some(s) = patterns_json.as_str() {
            if s.trim().starts_with('[') {
                if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str(s) {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                } else {
                    vec![s.to_string()]
                }
            } else {
                vec![s.to_string()]
            }
        } else {
            vec![]
        };
        assert_eq!(patterns, vec!["bl"]);
    }

    #[test]
    fn test_parse_insn_patterns_array() {
        let patterns_json = json!(["bl", "mov"]);
        let patterns: Vec<String> = if let Some(arr) = patterns_json.as_array() {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        } else {
            vec![]
        };
        assert_eq!(patterns, vec!["bl", "mov"]);
    }

    #[tokio::test]
    async fn test_dispatch_get_bytes_hex_string_address() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "get_bytes", json!({"addr": "0x5e8", "size": 16}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls[0].0, "get_bytes");
        assert_eq!(calls[0].1["addr"], 0x5e8u64);
    }

    #[tokio::test]
    async fn test_dispatch_get_bytes_decimal_string_address() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "get_bytes", json!({"addr": "1512", "size": 8}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls[0].0, "get_bytes");
        assert_eq!(calls[0].1["addr"], 1512u64);
    }

    #[tokio::test]
    async fn test_dispatch_get_bytes_numeric_address() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "get_bytes", json!({"addr": 0x5e8u64, "size": 4}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls[0].0, "get_bytes");
        assert_eq!(calls[0].1["addr"], 0x5e8u64);
    }

    #[tokio::test]
    async fn test_dispatch_find_bytes_single_string_pattern() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "find_bytes", json!({"pattern": "FD 7B"}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "find_bytes");
        assert_eq!(calls[0].1["pattern"], "FD 7B");
    }

    #[tokio::test]
    async fn test_dispatch_find_bytes_array_pattern() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "find_bytes", json!({"pattern": ["FD 7B", "90 90"]}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "find_bytes");
        assert_eq!(calls[0].1["pattern"], "FD 7B");
        assert_eq!(calls[1].0, "find_bytes");
        assert_eq!(calls[1].1["pattern"], "90 90");
    }

    #[tokio::test]
    async fn test_dispatch_find_bytes_json_array_string_pattern() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "find_bytes", json!({"pattern": r#"["FD 7B"]"#}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "find_bytes");
        assert_eq!(calls[0].1["pattern"], "FD 7B");
    }

    #[tokio::test]
    async fn test_dispatch_find_insns_single_string_pattern() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "find_insns", json!({"patterns": "bl"}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "find_insns");
        assert_eq!(calls[0].1["patterns"], json!(["bl"]));
    }

    #[tokio::test]
    async fn test_dispatch_find_insns_array_pattern() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "find_insns", json!({"patterns": ["bl", "mov"]}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "find_insns");
        assert_eq!(calls[0].1["patterns"], json!(["bl", "mov"]));
    }

    #[tokio::test]
    async fn test_dispatch_find_insns_json_array_string() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "find_insns", json!({"patterns": r#"["bl"]"#}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "find_insns");
        assert_eq!(calls[0].1["patterns"], json!(["bl"]));
    }

    #[tokio::test]
    async fn test_dispatch_decompile_hex_address() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "decompile", json!({"address": "0x100015a98"}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "decompile");
        assert_eq!(calls[0].1["addr"], 0x100015a98u64);
    }

    #[tokio::test]
    async fn test_dispatch_decompile_numeric_address() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "decompile", json!({"address": 4096u64}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "decompile");
        assert_eq!(calls[0].1["addr"], 4096u64);
    }

    /// Regression test: decompile with array of hex-string addresses
    /// Bug: before fix, passing ["0x100000328", "0x100000340"] was parsed as 0x0
    /// because parse_address_value() on a JSON array returns None → unwrap_or(0).
    /// Current code uses parse_address_values() which handles arrays correctly.
    #[tokio::test]
    async fn test_dispatch_decompile_array_hex_addresses() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "decompile",
            json!({"address": ["0x100000328", "0x100000340"]}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        // Both addresses must be dispatched (not collapsed to 0x0)
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "decompile");
        assert_eq!(calls[0].1["addr"], 0x100000328u64);
        assert_eq!(calls[1].0, "decompile");
        assert_eq!(calls[1].1["addr"], 0x100000340u64);
    }

    #[tokio::test]
    async fn test_dispatch_xrefs_to_hex_address() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "xrefs_to", json!({"address": "0x1000"}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "xrefs_to");
        assert_eq!(calls[0].1["addr"], 0x1000u64);
    }

    #[tokio::test]
    async fn test_dispatch_unknown_method_returns_error() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "nonexistent_tool", json!({}));

        let err = dispatch_rpc(&req, &mock).await.unwrap_err();

        match err {
            ToolError::InvalidToolName(msg) => {
                assert!(msg.contains("Unknown method: nonexistent_tool"))
            }
            other => panic!("expected InvalidToolName, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_dispatch_find_bytes_empty_pattern_returns_empty() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "find_bytes", json!({"pattern": null}));

        let result = dispatch_rpc(&req, &mock).await.unwrap();

        assert_eq!(result, json!({"matches": [], "count": 0}));
        let calls = mock.calls.lock().unwrap();
        assert!(calls.is_empty());
    }

    #[tokio::test]
    async fn test_dispatch_get_bytes_default_size() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "get_bytes", json!({"addr": "0x1000"}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "get_bytes");
        assert_eq!(calls[0].1["addr"], 0x1000u64);
        assert_eq!(calls[0].1["size"], 16);
    }

    #[tokio::test]
    async fn test_dispatch_batch_decompile_array_addresses() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "batch_decompile",
            json!({"addresses": ["0x1000", "0x2000"]}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "decompile");
        assert_eq!(calls[0].1["addr"], 0x1000u64);
        assert_eq!(calls[1].0, "decompile");
        assert_eq!(calls[1].1["addr"], 0x2000u64);
    }

    #[tokio::test]
    async fn test_dispatch_batch_decompile_single_address() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "batch_decompile", json!({"addresses": "0x3000"}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "decompile");
        assert_eq!(calls[0].1["addr"], 0x3000u64);
    }

    #[tokio::test]
    async fn test_dispatch_batch_decompile_json_array_string_addresses() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "batch_decompile",
            json!({"addresses": r#"["0x4000","0x5000"]"#}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "decompile");
        assert_eq!(calls[0].1["addr"], 0x4000u64);
        assert_eq!(calls[1].0, "decompile");
        assert_eq!(calls[1].1["addr"], 0x5000u64);
    }

    #[tokio::test]
    async fn test_dispatch_search_pseudocode_pattern() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "search_pseudocode", json!({"pattern": "malloc"}));

        let result = dispatch_rpc(&req, &mock).await.unwrap();

        assert_eq!(result["pattern"], "malloc");
        assert_eq!(result["matches"], json!([]));
        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "list_functions");
    }

    #[tokio::test]
    async fn test_dispatch_search_pseudocode_default_limit() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "search_pseudocode", json!({"pattern": "foo"}));

        let result = dispatch_rpc(&req, &mock).await.unwrap();

        assert_eq!(result["total_searched"], 0);
        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "list_functions");
    }

    #[tokio::test]
    async fn test_dispatch_search_pseudocode_timeout_secs() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "search_pseudocode",
            json!({"pattern": "bar", "timeout_secs": 7u64}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "list_functions");
        assert_eq!(calls[0].1["timeout_secs"], 7u64);
    }

    #[tokio::test]
    async fn test_dispatch_table_scan_hex_base_address() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "table_scan",
            json!({"base_address": "0x1000", "count": 1}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "get_bytes");
        assert_eq!(calls[0].1["addr"], 0x1000u64);
    }

    #[tokio::test]
    async fn test_dispatch_table_scan_default_stride_count() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "table_scan", json!({"base_address": 0x2000u64}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 16);
        assert_eq!(calls[0].0, "get_bytes");
        assert_eq!(calls[0].1["addr"], 0x2000u64);
        assert_eq!(calls[1].1["addr"], 0x2008u64);
        assert_eq!(calls[0].1["size"], 8);
    }

    #[tokio::test]
    async fn test_dispatch_diff_functions_hex_addresses() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "diff_functions",
            json!({"addr1": "0x1000", "addr2": "0x2000"}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "decompile");
        assert_eq!(calls[0].1["addr"], 0x1000u64);
        assert_eq!(calls[1].0, "decompile");
        assert_eq!(calls[1].1["addr"], 0x2000u64);
    }

    #[tokio::test]
    async fn test_dispatch_diff_functions_numeric_addresses() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "diff_functions",
            json!({"addr1": 4096u64, "addr2": 8192u64}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "decompile");
        assert_eq!(calls[0].1["addr"], 4096u64);
        assert_eq!(calls[1].0, "decompile");
        assert_eq!(calls[1].1["addr"], 8192u64);
    }

    #[tokio::test]
    async fn test_dispatch_rename_by_address() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "rename",
            json!({"address": "0x1000", "name": "my_func"}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "rename");
        assert_eq!(calls[0].1["addr"], 0x1000u64);
        assert_eq!(calls[0].1["new_name"], "my_func");
    }

    #[tokio::test]
    async fn test_dispatch_rename_by_current_name() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "rename",
            json!({"current_name": "old_func", "name": "new_func"}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "rename");
        assert_eq!(calls[0].1["current_name"], "old_func");
        assert_eq!(calls[0].1["new_name"], "new_func");
    }

    #[tokio::test]
    async fn test_dispatch_set_comments_by_address() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "set_comments",
            json!({"address": "0x2000", "comment": "hello"}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "set_comments");
        assert_eq!(calls[0].1["addr"], 0x2000u64);
        assert_eq!(calls[0].1["comment"], "hello");
    }

    #[tokio::test]
    async fn test_dispatch_set_comments_by_target_name() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "set_comments",
            json!({"target_name": "my_func", "comment": "world"}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "set_comments");
        assert_eq!(calls[0].1["name"], "my_func");
        assert_eq!(calls[0].1["comment"], "world");
    }

    #[tokio::test]
    async fn test_dispatch_patch_bytes_hex_string() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "patch_bytes",
            json!({"address": "0x3000", "bytes": "90 90 90"}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "patch_bytes");
        assert_eq!(calls[0].1["addr"], 0x3000u64);
        assert_eq!(calls[0].1["bytes"], json!([0x90u8, 0x90u8, 0x90u8]));
    }

    #[tokio::test]
    async fn test_dispatch_patch_bytes_array() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "patch_bytes",
            json!({"address": 0x3000u64, "bytes": [144, 144]}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "patch_bytes");
        assert_eq!(calls[0].1["addr"], 0x3000u64);
        assert_eq!(calls[0].1["bytes"], json!([144u8, 144u8]));
    }

    #[tokio::test]
    async fn test_dispatch_patch_asm_by_address() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "patch_asm",
            json!({"address": "0x4000", "line": "nop"}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "patch_asm");
        assert_eq!(calls[0].1["addr"], 0x4000u64);
        assert_eq!(calls[0].1["line"], "nop");
    }

    #[tokio::test]
    async fn test_dispatch_declare_type() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "declare_type",
            json!({"decl": "typedef int MyInt;", "replace": true}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "declare_type");
        assert_eq!(calls[0].1["decl"], "typedef int MyInt;");
        assert_eq!(calls[0].1["replace"], true);
    }

    #[tokio::test]
    async fn test_dispatch_apply_types_by_address() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "apply_types",
            json!({"address": "0x5000", "decl": "void foo(void);"}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "apply_types");
        assert_eq!(calls[0].1["addr"], 0x5000u64);
        assert_eq!(calls[0].1["decl"], "void foo(void);");
    }

    #[tokio::test]
    async fn test_dispatch_infer_types_by_address() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "infer_types", json!({"address": "0x6000"}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "infer_types");
        assert_eq!(calls[0].1["addr"], 0x6000u64);
    }

    #[tokio::test]
    async fn test_dispatch_declare_stack() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "declare_stack",
            json!({"address": "0x7000", "offset": -8i64, "decl": "int* p;"}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "declare_stack");
        assert_eq!(calls[0].1["addr"], 0x7000u64);
        assert_eq!(calls[0].1["offset"], -8i64);
        assert_eq!(calls[0].1["decl"], "int* p;");
    }

    #[tokio::test]
    async fn test_dispatch_delete_stack() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "delete_stack",
            json!({"address": "0x7000", "offset": -8i64}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "delete_stack");
        assert_eq!(calls[0].1["addr"], 0x7000u64);
        assert_eq!(calls[0].1["offset"], -8i64);
    }

    #[tokio::test]
    async fn test_dispatch_basic_blocks() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "basic_blocks", json!({"address": "0x1000"}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "basic_blocks");
        assert_eq!(calls[0].1["addr"], 0x1000u64);
    }

    #[tokio::test]
    async fn test_dispatch_callers() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "callers", json!({"address": "0x1000"}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "callers");
        assert_eq!(calls[0].1["addr"], 0x1000u64);
    }

    #[tokio::test]
    async fn test_dispatch_callees() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "callees", json!({"address": "0x1000"}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "callees");
        assert_eq!(calls[0].1["addr"], 0x1000u64);
    }

    #[tokio::test]
    async fn test_dispatch_callgraph() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "callgraph",
            json!({"roots": ["0x1000"], "max_depth": 3, "max_nodes": 64}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "callgraph");
        assert_eq!(calls[0].1["addr"], 0x1000u64);
        assert_eq!(calls[0].1["max_depth"], 3u64);
        assert_eq!(calls[0].1["max_nodes"], 64u64);
    }

    #[tokio::test]
    async fn test_dispatch_find_paths() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "find_paths",
            json!({"start": "0x1000", "end": "0x2000"}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "find_paths");
        assert_eq!(calls[0].1["start"], 0x1000u64);
        assert_eq!(calls[0].1["end"], 0x2000u64);
    }

    #[tokio::test]
    async fn test_dispatch_list_globals() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "list_globals",
            json!({"query": "some_global", "limit": 50}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "list_globals");
        assert_eq!(calls[0].1["query"], "some_global");
        assert_eq!(calls[0].1["limit"], 50u64);
    }

    #[tokio::test]
    async fn test_dispatch_find_insn_operands() {
        let mock = MockWorker::new();
        let req = RpcRequest::new("1", "find_insn_operands", json!({"patterns": "rax"}));

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "find_insn_operands");
        assert_eq!(calls[0].1["patterns"], json!(["rax"]));
    }

    #[tokio::test]
    async fn test_dispatch_rename_lvar() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "rename_lvar",
            json!({"func_address": "0x1000", "lvar_name": "v1", "new_name": "counter"}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "rename_lvar");
        assert_eq!(calls[0].1["func_addr"], 0x1000u64);
        assert_eq!(calls[0].1["lvar_name"], "v1");
        assert_eq!(calls[0].1["new_name"], "counter");
    }

    #[tokio::test]
    async fn test_dispatch_set_lvar_type() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "set_lvar_type",
            json!({"func_address": "0x2000", "lvar_name": "v1", "type_str": "unsigned int"}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "set_lvar_type");
        assert_eq!(calls[0].1["func_addr"], 0x2000u64);
        assert_eq!(calls[0].1["lvar_name"], "v1");
        assert_eq!(calls[0].1["type_str"], "unsigned int");
    }

    #[tokio::test]
    async fn test_dispatch_set_decompiler_comment() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "set_decompiler_comment",
            json!({"func_address": "0x3000", "address": "0x3010", "comment": "loop start"}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "set_decompiler_comment");
        assert_eq!(calls[0].1["func_addr"], 0x3000u64);
        assert_eq!(calls[0].1["addr"], 0x3010u64);
        assert_eq!(calls[0].1["itp"], 69);  // default ITP_SEMI
        assert_eq!(calls[0].1["comment"], "loop start");
    }

    #[tokio::test]
    async fn test_dispatch_set_decompiler_comment_custom_itp() {
        let mock = MockWorker::new();
        let req = RpcRequest::new(
            "1",
            "set_decompiler_comment",
            json!({"func_address": "0x3000", "address": "0x3010", "itp": 74, "comment": "block"}),
        );

        dispatch_rpc(&req, &mock).await.unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "set_decompiler_comment");
        assert_eq!(calls[0].1["itp"], 74);  // ITP_BLOCK1
        assert_eq!(calls[0].1["comment"], "block");
    }
}
