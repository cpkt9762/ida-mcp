# CLI Tool Quick Reference

This page is the quick reference for `ida-cli`. Use it when you need a command pattern, a common flag combination, or a reminder about how the CLI maps to analysis tasks.

## Open and Inspect a Binary

```bash
ida-cli --path <file> list-functions --limit 20
ida-cli --path <file> get-function-by-name --name main
ida-cli --path <file> get-function-at-address --address 0x1000
ida-cli --path <file> get-address-info --address 0x1000
```

## Decompilation and Disassembly

```bash
ida-cli --path <file> decompile-function --address 0x1000
ida-cli --path <file> disassemble --address 0x1000 --count 20
ida-cli --path <file> disassemble-function --name func_name --count 50
ida-cli --path <file> disassemble-function-at --address 0x1000 --count 200
ida-cli --path <file> batch-decompile --addresses "0x1000,0x2000,0x3000"
```

## Call Graph and Cross-References

```bash
ida-cli --path <file> get-callees --address 0x1000
ida-cli --path <file> get-callers --address 0x1000
ida-cli --path <file> build-callgraph --roots 0x1000 --max-depth 3
ida-cli --path <file> get-xrefs-to --address 0x1000
ida-cli --path <file> get-xrefs-from --address 0x1000
```

## Search

```bash
ida-cli --path <file> search-text --targets 110 --kind imm
ida-cli --path <file> search-text --targets password --kind text
ida-cli --path <file> search-bytes --pattern "6E 00 00 00"
ida-cli --path <file> search-pseudocode --pattern "amount"
ida-cli --path <file> search-instructions --patterns "MUL,UDIV"
ida-cli --path <file> search-instruction-operands --patterns "#0x6E"
ida-cli --path <file> list-strings --query error --limit 20
```

## Metadata and Layout

```bash
ida-cli --path <file> list-segments
ida-cli --path <file> list-imports
ida-cli --path <file> list-exports
ida-cli --path <file> list-entry-points
ida-cli --path <file> get-database-info
ida-cli --path <file> get-analysis-status
```

## Reads and Struct Access

```bash
ida-cli --path <file> read-bytes --address 0x1000 --size 32
ida-cli --path <file> read-string --address 0x1000
ida-cli --path <file> read-int --address 0x1000
ida-cli --path <file> read-struct-at-address --address 0x1000 --name Config
ida-cli --path <file> get-stack-frame --address 0x1000
```

## Renaming, Comments, and Types

```bash
ida-cli --path <file> rename-symbol --address 0x1000 --new-name parse_header
ida-cli --path <file> set-comment --address 0x1000 --comment "header parse gate"
ida-cli --path <file> set-function-comment --address 0x1000 --comment "top-level dispatcher"
ida-cli --path <file> set-function-prototype --address 0x1000 --prototype "int64_t __fastcall parse_header(Config *cfg)"
ida-cli --path <file> rename-stack-variable --func-address 0x1000 --var-name v1 --new-name amount_in
ida-cli --path <file> set-stack-variable-type --func-address 0x1000 --var-name amount_in --type-str uint64_t
ida-cli --path <file> declare-c-type --decl "struct Config { int magic; char key[32]; };"
ida-cli --path <file> apply-type --name parse_header --decl "int64_t __fastcall parse_header(Config *cfg)"
```

## Scripting

```bash
ida-cli --path <file> run-script --code 'import idautils; print(len(list(idautils.Functions())))'
```

## Output Modes

```bash
ida-cli --json --path <file> list-functions --limit 5
ida-cli --compact --path <file> list-functions --limit 5
```

## Runtime and Cache

```bash
ida-cli probe-runtime
ida-cli prewarm --path <file>
ida-cli prewarm-many samples.txt --jobs 8
ida-cli status
ida-cli close --path <file>
```

## Service Operations

Use these only when you actually need a long-lived service:

```bash
ida-cli serve
ida-cli serve-http --bind 127.0.0.1:8765
ida-cli shutdown
```

## Raw Mode

Use raw JSON only when the regular subcommand does not cover what you need:

```bash
ida-cli --path <file> raw '{"method":"get_xrefs_to","params":{"address":"0x1000"}}'
ida-cli --json pipe <<'EOF'
{"method":"status"}
{"method":"list_functions","params":{"path":"<file>","limit":5}}
EOF
```

## Debugging

The CLI also exposes the debugger command family for process start, attach, breakpoints, stepping, registers, and memory inspection. Use the `dbg-*` subcommands only when you actually need live debugging.

## Notes

- `.i64` / `.idb` inputs reopen the existing IDA database.
- Raw PE / ELF / Mach-O inputs are analyzed and cached automatically.
- Little-endian byte order matters for byte-pattern searches.
- Prefer ordinary subcommands over raw JSON whenever possible.
