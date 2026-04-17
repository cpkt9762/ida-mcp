use crate::ida::runtime::WorkerBackendKind;

const IDAT_COMPAT_METHODS: &[&str] = &[
    "open",
    "close",
    "shutdown",
    "get_analysis_status",
    "get_database_info",
    "list_functions",
    "get_function_by_name",
    "get_function_at_address",
    "get_address_info",
    "disassemble",
    "disassemble_function",
    "disassemble_function_at",
    "decompile_function",
    "get_pseudocode_at",
    "batch_decompile",
    "search_pseudocode",
    "diff_pseudocode",
    "list_segments",
    "list_strings",
    "list_imports",
    "list_exports",
    "list_entry_points",
    "list_globals",
    "read_bytes",
    "read_string",
    "read_int",
    "search_text",
    "search_bytes",
    "get_xrefs_to",
    "get_xrefs_from",
    "run_script",
];

pub fn supported_methods_for(backend: WorkerBackendKind) -> &'static [&'static str] {
    match backend {
        WorkerBackendKind::IdatCompat => IDAT_COMPAT_METHODS,
        WorkerBackendKind::NativeLinked => &[],
    }
}
