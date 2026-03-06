"""IDA Debugger Headless Feasibility Test — 4 levels: Import → API → ReadOnly → Load."""

import json
import platform
import sys

results = {
    "level_1_import": {"status": "skip", "details": {}},
    "level_2_api_check": {"status": "skip", "details": {}},
    "level_3_readonly_calls": {"status": "skip", "details": {}},
    "level_4_debugger_load": {"status": "skip", "details": {}},
}

# === Level 1: Module Import ===
try:
    import ida_dbg
    import ida_idd
    import ida_idp
    import idaapi

    results["level_1_import"] = {
        "status": "pass",
        "details": {
            "ida_dbg": True,
            "ida_idd": True,
            "ida_idp": True,
            "idaapi": True,
            "ida_dbg_dir_sample": [x for x in dir(ida_dbg) if not x.startswith("_")][
                :30
            ],
        },
    }
except ImportError as e:
    results["level_1_import"] = {"status": "fail", "details": {"error": str(e)}}
    print(json.dumps(results, indent=2, default=str))
    sys.exit(0)

# === Level 2: API Existence Check ===
critical_apis = {
    "start_process": hasattr(ida_dbg, "start_process"),
    "exit_process": hasattr(ida_dbg, "exit_process"),
    "suspend_process": hasattr(ida_dbg, "suspend_process"),
    "continue_process": hasattr(ida_dbg, "continue_process"),
    "attach_process": hasattr(ida_dbg, "attach_process"),
    "detach_process": hasattr(ida_dbg, "detach_process"),
    "get_process_state": hasattr(ida_dbg, "get_process_state"),
    "step_into": hasattr(ida_dbg, "step_into"),
    "step_over": hasattr(ida_dbg, "step_over"),
    "step_until_ret": hasattr(ida_dbg, "step_until_ret"),
    "run_to": hasattr(ida_dbg, "run_to"),
    "get_reg_val": hasattr(ida_dbg, "get_reg_val"),
    "set_reg_val": hasattr(ida_dbg, "set_reg_val"),
    "get_ip_val": hasattr(ida_dbg, "get_ip_val"),
    "get_sp_val": hasattr(ida_dbg, "get_sp_val"),
    "add_bpt": hasattr(ida_dbg, "add_bpt"),
    "del_bpt": hasattr(ida_dbg, "del_bpt"),
    "enable_bpt": hasattr(ida_dbg, "enable_bpt"),
    "get_bpt_qty": hasattr(ida_dbg, "get_bpt_qty"),
    "check_bpt": hasattr(ida_dbg, "check_bpt"),
    "get_thread_qty": hasattr(ida_dbg, "get_thread_qty"),
    "get_current_thread": hasattr(ida_dbg, "get_current_thread"),
    "select_thread": hasattr(ida_dbg, "select_thread"),
    "read_dbg_memory": hasattr(ida_dbg, "read_dbg_memory"),
    "write_dbg_memory": hasattr(ida_dbg, "write_dbg_memory"),
    "wait_for_next_event": hasattr(ida_dbg, "wait_for_next_event"),
    "get_debug_event": hasattr(ida_dbg, "get_debug_event"),
}

missing = [k for k, v in critical_apis.items() if not v]
results["level_2_api_check"] = {
    "status": "pass" if not missing else "partial",
    "details": {
        "total_checked": len(critical_apis),
        "available": sum(1 for v in critical_apis.values() if v),
        "missing": missing,
        "all_apis": critical_apis,
    },
}

# === Level 3: Read-Only Calls (no state mutation) ===
readonly_results = {}

try:
    state = ida_dbg.get_process_state()
    state_names = {-1: "DSTATE_SUSP", 0: "DSTATE_NOTASK", 1: "DSTATE_RUN"}
    readonly_results["get_process_state"] = {
        "ok": True,
        "value": state,
        "meaning": state_names.get(state, "unknown"),
    }
except Exception as e:
    readonly_results["get_process_state"] = {"ok": False, "error": str(e)}

try:
    readonly_results["get_bpt_qty"] = {"ok": True, "value": ida_dbg.get_bpt_qty()}
except Exception as e:
    readonly_results["get_bpt_qty"] = {"ok": False, "error": str(e)}

try:
    readonly_results["is_debugger_on"] = {"ok": True, "value": ida_dbg.is_debugger_on()}
except Exception as e:
    readonly_results["is_debugger_on"] = {"ok": False, "error": str(e)}

try:
    readonly_results["get_thread_qty"] = {"ok": True, "value": ida_dbg.get_thread_qty()}
except Exception as e:
    readonly_results["get_thread_qty"] = {"ok": False, "error": str(e)}

try:
    name = ida_dbg.dbg_get_name() if hasattr(ida_dbg, "dbg_get_name") else "N/A"
    readonly_results["dbg_get_name"] = {"ok": True, "value": name}
except Exception as e:
    readonly_results["dbg_get_name"] = {"ok": False, "error": str(e)}

try:
    debugger_names = []
    if hasattr(idaapi, "get_debugger_plugins"):
        for p in idaapi.get_debugger_plugins():
            debugger_names.append(str(p))
    else:
        debugger_names.append("(enumeration API not available)")
    readonly_results["debugger_plugins"] = {"ok": True, "value": debugger_names}
except Exception as e:
    readonly_results["debugger_plugins"] = {"ok": False, "error": str(e)}

try:
    import ida_kernwin

    batch = (
        ida_kernwin.cvar.batch
        if hasattr(ida_kernwin, "cvar") and hasattr(ida_kernwin.cvar, "batch")
        else "unknown"
    )
    readonly_results["batch_mode"] = {"ok": True, "value": batch}
except Exception as e:
    readonly_results["batch_mode"] = {"ok": False, "error": str(e)}

try:
    ip = ida_dbg.get_ip_val()
    readonly_results["get_ip_val_no_process"] = {
        "ok": True,
        "value": hex(ip) if ip is not None else None,
    }
except Exception as e:
    readonly_results["get_ip_val_no_process"] = {
        "ok": False,
        "error": str(e),
        "implication": "callui likely not routed in headless mode",
    }

ok_count = sum(1 for v in readonly_results.values() if v.get("ok"))
results["level_3_readonly_calls"] = {
    "status": "pass" if ok_count == len(readonly_results) else "partial",
    "details": {
        "total": len(readonly_results),
        "passed": ok_count,
        "failed": len(readonly_results) - ok_count,
        "tests": readonly_results,
    },
}

# === Level 4: Debugger Module Loading ===
debugger_load_results = {}

try:
    proc_name = ida_idp.get_idp_name()
    debugger_load_results["processor"] = {"ok": True, "value": proc_name}
except Exception as e:
    debugger_load_results["processor"] = {"ok": False, "error": str(e)}

try:
    dbg_info = {}
    if hasattr(ida_dbg, "dbg_is_loaded"):
        dbg_info["is_loaded"] = ida_dbg.dbg_is_loaded()
    if hasattr(idaapi, "get_dbg"):
        dbg_obj = idaapi.get_dbg()
        if dbg_obj:
            dbg_info["name"] = getattr(dbg_obj, "name", "?")
            dbg_info["id"] = getattr(dbg_obj, "id", "?")
        else:
            dbg_info["get_dbg"] = None
    debugger_load_results["debugger_info"] = {"ok": True, "value": dbg_info}
except Exception as e:
    debugger_load_results["debugger_info"] = {"ok": False, "error": str(e)}

dbg_name_to_try = {"Darwin": "mac", "Linux": "linux", "Windows": "win32"}.get(
    platform.system(), "gdb"
)
try:
    if hasattr(ida_dbg, "load_debugger"):
        ok = ida_dbg.load_debugger(dbg_name_to_try, False)
        debugger_load_results["load_debugger"] = {
            "ok": True,
            "loaded": ok,
            "debugger": dbg_name_to_try,
        }
    else:
        debugger_load_results["load_debugger"] = {
            "ok": False,
            "error": "load_debugger not found in ida_dbg",
        }
except Exception as e:
    debugger_load_results["load_debugger"] = {
        "ok": False,
        "error": str(e),
        "debugger": dbg_name_to_try,
    }

try:
    import idc

    entry = idc.get_inf_attr(idc.INF_START_EA)
    if entry and entry != idaapi.BADADDR:
        add_ok = ida_dbg.add_bpt(entry, 0, ida_idd.BPT_SOFT)
        check_result = ida_dbg.check_bpt(entry)
        del_ok = ida_dbg.del_bpt(entry)
        debugger_load_results["bpt_add_del"] = {
            "ok": True,
            "entry": hex(entry),
            "add_result": add_ok,
            "check_result": check_result,
            "del_result": del_ok,
        }
    else:
        debugger_load_results["bpt_add_del"] = {
            "ok": True,
            "skip": "no valid entrypoint",
        }
except Exception as e:
    debugger_load_results["bpt_add_del"] = {"ok": False, "error": str(e)}

ok_count_4 = sum(1 for v in debugger_load_results.values() if v.get("ok"))
results["level_4_debugger_load"] = {
    "status": "pass" if ok_count_4 == len(debugger_load_results) else "partial",
    "details": {
        "total": len(debugger_load_results),
        "passed": ok_count_4,
        "tests": debugger_load_results,
    },
}

# === Summary ===
level_keys = [
    "level_1_import",
    "level_2_api_check",
    "level_3_readonly_calls",
    "level_4_debugger_load",
]
all_levels = [results[k]["status"] for k in level_keys]
results["summary"] = {
    "feasible": all(s in ("pass", "partial") for s in all_levels),
    "levels": {
        "L1_import": all_levels[0],
        "L2_api_exists": all_levels[1],
        "L3_readonly_works": all_levels[2],
        "L4_debugger_loads": all_levels[3],
    },
    "verdict": (
        "FULLY_FEASIBLE"
        if all(s == "pass" for s in all_levels)
        else "PARTIALLY_FEASIBLE"
        if any(s == "pass" for s in all_levels)
        else "NOT_FEASIBLE"
    ),
}

print(json.dumps(results, indent=2, default=str))
