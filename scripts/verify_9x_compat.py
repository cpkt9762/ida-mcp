#!/usr/bin/env python3
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "debug" / "ida-cli"
SAMPLE = Path(os.environ.get("IDA_CLI_SAMPLE", "/path/to/example2-devirt.bin"))


def run(cmd, timeout=1200, check=True):
    proc = subprocess.run(
        cmd,
        cwd=ROOT,
        text=True,
        capture_output=True,
        timeout=timeout,
        env=os.environ.copy(),
    )
    if check and proc.returncode != 0:
        raise RuntimeError(
            f"command failed: {' '.join(map(str, cmd))}\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
        )
    return proc


class WorkerClient:
    def __init__(self, backend: str):
        self.proc = subprocess.Popen(
            [str(BIN), "serve-worker", "--backend", backend],
            cwd=ROOT,
            env=os.environ.copy(),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self.seq = 0

    def call(self, method: str, params: dict):
        self.seq += 1
        req = {
            "jsonrpc": "2.0",
            "id": str(self.seq),
            "method": method,
            "params": params,
        }
        assert self.proc.stdin and self.proc.stdout
        self.proc.stdin.write(json.dumps(req) + "\n")
        self.proc.stdin.flush()
        line = self.proc.stdout.readline()
        if not line:
            stderr = self.proc.stderr.read() if self.proc.stderr else ""
            raise RuntimeError(f"worker exited while handling {method}\nstderr:\n{stderr}")
        resp = json.loads(line)
        if resp.get("error"):
            raise RuntimeError(f"{method} failed: {resp['error']['message']}")
        return resp.get("result")

    def close(self):
        try:
            self.call("shutdown", {})
        except Exception:
            pass
        try:
            if self.proc.stdin:
                self.proc.stdin.close()
        except Exception:
            pass
        try:
            self.proc.terminate()
        except Exception:
            pass


def main():
    if not BIN.exists():
        raise SystemExit(f"binary not found: {BIN}")
    if not SAMPLE.exists():
        raise SystemExit(f"sample not found: {SAMPLE}")
    if "IDADIR" not in os.environ:
        raise SystemExit("IDADIR must be set")

    probe = run([str(BIN), "probe-runtime"])
    probe_json = json.loads(probe.stdout)
    if not probe_json.get("supported"):
        raise SystemExit(f"runtime unsupported: {probe.stdout}")
    backend = probe_json["backend"]

    client = WorkerClient(backend)
    try:
        results = {}
        results["probe-runtime"] = probe_json
        results["open"] = client.call(
            "open",
            {
                "path": str(SAMPLE),
                "auto_analyse": True,
                "load_debug_info": False,
                "debug_info_verbose": False,
                "force": False,
                "extra_args": [],
            },
        )
        results["get_analysis_status"] = client.call("get_analysis_status", {})
        results["list_functions"] = client.call(
            "list_functions", {"offset": 0, "limit": 5}
        )
        results["get_function_by_name"] = client.call(
            "get_function_by_name", {"name": "main"}
        )
        results["get_function_at_address"] = client.call(
            "get_function_at_address", {"address": "0x140001000"}
        )
        results["get_address_info"] = client.call(
            "get_address_info", {"address": "0x140001000"}
        )
        results["disassemble"] = client.call(
            "disassemble", {"address": "0x140001000", "count": 10}
        )
        results["disassemble_function"] = client.call(
            "disassemble_function", {"name": "main", "count": 10}
        )
        results["disassemble_function_at"] = client.call(
            "disassemble_function_at", {"address": "0x140001000", "count": 10}
        )
        results["decompile_function"] = client.call(
            "decompile_function", {"address": "0x140001000"}
        )
        results["get_pseudocode_at"] = client.call(
            "get_pseudocode_at", {"address": "0x140001000"}
        )
        results["batch_decompile"] = client.call(
            "batch_decompile",
            {"addresses": ["0x140001000", "0x140001130"]},
        )
        results["search_pseudocode"] = client.call(
            "search_pseudocode", {"pattern": "WriteConsoleA", "limit": 5}
        )
        results["diff_pseudocode"] = client.call(
            "diff_pseudocode",
            {"addr1": "0x140001000", "addr2": "0x140001130"},
        )
        results["list_segments"] = client.call("list_segments", {})
        results["list_strings"] = client.call("list_strings", {"limit": 10})
        results["list_imports"] = client.call(
            "list_imports", {"offset": 0, "limit": 10}
        )
        results["list_exports"] = client.call(
            "list_exports", {"offset": 0, "limit": 10}
        )
        results["list_entry_points"] = client.call("list_entry_points", {})
        results["get_database_info"] = client.call("get_database_info", {})
        results["list_globals"] = client.call("list_globals", {"limit": 10})
        results["read_bytes"] = client.call(
            "read_bytes", {"address": "0x140001000", "size": 16}
        )
        results["read_string"] = client.call(
            "read_string", {"address": "0x140002162", "max_len": 64}
        )
        results["read_int"] = client.call(
            "read_int", {"address": "0x140001000", "size": 4}
        )
        results["search_text"] = client.call(
            "search_text", {"text": "UPPERCASE", "max_results": 5}
        )
        results["search_bytes"] = client.call(
            "search_bytes", {"patterns": "45 6e 74 65 72", "limit": 5}
        )
        results["get_xrefs_to"] = client.call(
            "get_xrefs_to", {"address": "0x140001000"}
        )
        results["get_xrefs_from"] = client.call(
            "get_xrefs_from", {"address": "0x140001000"}
        )
        results["run_script"] = client.call("run_script", {"code": "print(1+2)"})

        out_dir = Path("/tmp/ida-cli-out")
        out_dir.mkdir(parents=True, exist_ok=True)
        report = out_dir / "verify-9x-compat.json"
        report.write_text(json.dumps(results, indent=2))
        print(json.dumps({"ok": True, "backend": backend, "report": str(report)}, indent=2))
    finally:
        client.close()


if __name__ == "__main__":
    main()
