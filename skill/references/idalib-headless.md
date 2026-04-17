# idalib Headless Analysis

Using IDA Pro as an embedded library for headless/automated analysis.

## Overview

idalib (IDA 9.0+) lets you run IDA's analysis engine as a shared library — no GUI, no separate process. Three access layers:

| Layer | Language | Install | Best For |
|-------|----------|---------|----------|
| **ida-domain** | Python | `pip install ida-domain` | Scripting, quick analysis |
| **idapro** | Python | `pip install .` from `$IDADIR/idalib/python` | Full SDK access headlessly |
| **idalib** | C++ | Link against `idalib.so/.dylib/.dll` | Embedding in C++ tools |

---

## Python: idapro Module

### Installation

```bash
# Step 1: Install the module
cd "$IDADIR/idalib/python"
pip install .

# Step 2: Activate (register IDA path)
python "$IDADIR/py-activate-idalib.py"
# Or with explicit path:
python py-activate-idalib.py -d /Applications/IDA\ Pro.app/Contents/MacOS

# Step 3: Set environment
export IDADIR="/Applications/IDA Pro.app/Contents/MacOS"
```

### Basic Usage

```python
# ⚠️ CRITICAL: import idapro must be the FIRST import
import idapro

# Open a new binary for analysis
idapro.open_database("target.exe", auto_analysis=True)

# Use any IDAPython API
import idc
import idautils
import ida_funcs
import ida_bytes
import ida_hexrays

# Wait for auto-analysis to complete
idc.auto_wait()

# Enumerate functions
for ea in idautils.Functions():
    name = idc.get_func_name(ea)
    func = ida_funcs.get_func(ea)
    size = func.end_ea - func.start_ea
    print(f"{name}: {ea:#x} ({size} bytes)")

# Decompile
cfunc = ida_hexrays.decompile(ea)
if cfunc:
    print(str(cfunc))

# Read memory
data = ida_bytes.get_bytes(0x401000, 16)

# Close database (save=True to persist .idb)
idapro.close_database(save=False)
```

### Open Existing .idb / .i64

```python
import idapro

# Open existing database (no re-analysis needed)
idapro.open_database("target.i64", auto_analysis=False)

import idc, idautils

for ea in idautils.Functions():
    print(idc.get_func_name(ea))

idapro.close_database()
```

### Batch Processing Multiple Binaries

```python
import idapro
import idc, idautils, ida_hexrays
import json
from pathlib import Path

def analyze_binary(path):
    """Analyze a single binary and extract metadata."""
    idapro.open_database(str(path), auto_analysis=True)
    idc.auto_wait()

    result = {
        "file": str(path),
        "functions": [],
        "strings": [],
    }

    for ea in idautils.Functions():
        name = idc.get_func_name(ea)
        result["functions"].append({"name": name, "address": f"{ea:#x}"})

    for s in idautils.Strings():
        result["strings"].append({"address": f"{s.ea:#x}", "value": str(s)})

    idapro.close_database(save=False)
    return result

# Process all ELF files in a directory
output = []
for binary in Path("/samples").glob("*"):
    if binary.is_file():
        try:
            print(f"Analyzing: {binary.name}")
            output.append(analyze_binary(binary))
        except Exception as e:
            print(f"Failed: {binary.name}: {e}")

Path("/tmp/batch_results.json").write_text(json.dumps(output, indent=2))
```

### Headless Decompilation to Files

```python
import idapro
import idc, idautils, ida_hexrays
from pathlib import Path

idapro.open_database("firmware.bin", auto_analysis=True)
idc.auto_wait()

output_dir = Path("/tmp/decompiled")
output_dir.mkdir(exist_ok=True)

for ea in idautils.Functions():
    name = idc.get_func_name(ea)
    try:
        cfunc = ida_hexrays.decompile(ea)
        if cfunc:
            out = output_dir / f"{name}.c"
            out.write_text(str(cfunc))
    except ida_hexrays.DecompilationFailure:
        pass

idapro.close_database()
print(f"Decompiled to {output_dir}")
```

---

## Python: ida-domain (High-Level)

ida-domain wraps idapro with a cleaner API. It is the **recommended** approach.

```bash
pip install ida-domain
export IDADIR="/Applications/IDA Pro.app/Contents/MacOS"
```

```python
from ida_domain import Database
from ida_domain.database import IdaCommandOptions

opts = IdaCommandOptions(auto_analysis=True)
with Database.open("binary", opts) as db:
    # Functions
    for func in db.functions:
        name = db.functions.get_name(func)
        pseudocode = db.functions.get_pseudocode(func)

    # Strings
    for s in db.strings:
        print(f"{s.address:#x}: {s}")

    # Cross-references
    for xref in db.xrefs.to_ea(target_ea):
        print(f"From {xref.from_ea:#x}")
# Database auto-closes and optionally saves
```

### When to Use ida-domain vs idapro

| Use **ida-domain** when... | Use **idapro** when... |
|---|---|
| Standard RE tasks (functions, strings, xrefs) | Need struct/enum creation APIs |
| Want clean, Pythonic code | Need `ida_hexrays` AST traversal |
| Quick scripts and automation | Need `ida_struct`, `ida_enum` modules |
| Don't need low-level SDK details | Need processor-specific APIs |

You can **mix both** — ida-domain for high-level, drop to idapro for SDK specifics:

```python
from ida_domain import Database

with Database.open("binary") as db:
    # High-level: ida-domain
    func = db.functions.get_function_by_name("main")
    pseudocode = db.functions.get_pseudocode(func)

    # Low-level: drop to IDAPython SDK
    import ida_hexrays
    cfunc = ida_hexrays.decompile(func.start_ea)
    # Traverse the AST...
    for item in cfunc.treeitems:
        if item.op == ida_hexrays.cot_call:
            print(f"Call at {item.ea:#x}")
```

---

## C++ Integration

### Setup

```cpp
#include <idalib/idalib.hpp>

int main(int argc, char *argv[]) {
    // Initialize idalib
    if (!idalib::init()) {
        fprintf(stderr, "Failed to initialize idalib\n");
        return 1;
    }

    // Open database
    idalib::database_t db;
    if (!db.open("target.exe", /*auto_analysis=*/true)) {
        fprintf(stderr, "Failed to open database\n");
        return 1;
    }

    // Use standard IDA C++ SDK APIs
    // e.g., iterate functions, read bytes, etc.

    // Close
    db.close(/*save=*/false);
    idalib::term();
    return 0;
}
```

### CMake Build (IDA 9.2+)

```cmake
cmake_minimum_required(VERSION 3.20)
project(my_analyzer)

# Set IDA SDK path
set(IDA_SDK_DIR "/path/to/ida-sdk")

find_package(idalib REQUIRED PATHS ${IDA_SDK_DIR}/cmake)

add_executable(my_analyzer main.cpp)
target_link_libraries(my_analyzer PRIVATE idalib::idalib)
```

### Linking (Manual)

```bash
# macOS
clang++ -std=c++17 main.cpp \
    -I$IDADIR/idalib/include \
    -L$IDADIR/idalib/lib \
    -lidalib -o my_analyzer

# Linux
g++ -std=c++17 main.cpp \
    -I$IDADIR/idalib/include \
    -L$IDADIR/idalib/lib \
    -lidalib -Wl,-rpath,$IDADIR/idalib/lib -o my_analyzer
```

---

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Binary Analysis
on: [push]

jobs:
  analyze:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup IDA
        run: |
          # Assumes IDA is pre-installed or available as artifact
          export IDADIR="/opt/ida"
          cd $IDADIR/idalib/python && pip install .
          python $IDADIR/py-activate-idalib.py

      - name: Run analysis
        env:
          IDADIR: /opt/ida
        run: python scripts/analyze.py firmware.bin
```

### Docker

```dockerfile
FROM python:3.11-slim

# Copy IDA Pro installation (requires valid license)
COPY ida-pro /opt/ida
ENV IDADIR=/opt/ida

# Install idalib Python module
RUN cd /opt/ida/idalib/python && pip install . && \
    python /opt/ida/py-activate-idalib.py

# Install ida-domain
RUN pip install ida-domain

COPY analyze.py /app/
WORKDIR /app
ENTRYPOINT ["python", "analyze.py"]
```

---

## idalib vs idat -A Comparison

| Feature | idalib (in-process) | idat -A (separate process) |
|---------|--------------------|-----------------------------|
| **Startup** | Fast (library load) | Slow (process spawn + init) |
| **Memory** | Shared with host | Isolated process |
| **API access** | Full, native | Script-only via `-S` flag |
| **IDE support** | Autocomplete, debug | None |
| **Batch processing** | One process, many DBs | One process per binary |
| **Error handling** | Try/except in Python | Exit codes + log parsing |
| **GUI plugins** | Not available | Available (may fail silently) |
| **Licensing** | IDA Pro 9.0+ required | Any IDA version |
| **OEM use** | Requires OEM license | Standard license OK |

### When to Use idat -A Instead

```bash
# Quick one-off analysis
idat -A -S"script.py" binary.exe

# With script arguments
idat -A -S"script.py arg1 arg2" binary.exe

# Auto-analysis only (create .idb and exit)
idat -A -B binary.exe
```

---

## Community Projects

| Project | Description |
|---------|-------------|
| [Rhabdomancer](https://github.com/0xdea/rhabdomancer) | Rust-based vulnerability discovery using idalib |
| [Haruspex](https://github.com/0xdea/haruspex) | Headless decompilation, exports pseudocode to files |
| [Augur](https://github.com/0xdea/augur) | String extraction with xref-to-pseudocode mapping |
| [headless-ida](https://pypi.org/project/headless-ida/) | Python wrapper for batch processing with idalib backend |
| [headless-ida-mcp-server](https://github.com/MxIris-Reverse-Engineering/headless-ida-mcp-server) | Headless automation server for AI-assisted reverse engineering via idalib |

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `ModuleNotFoundError: idapro` | Run `py-activate-idalib.py` and ensure `IDADIR` is set |
| `import idapro` must be first | Move `import idapro` to the **very first line** before any other imports |
| GUI plugin crashes in headless | Guard with `if ida_kernwin.is_idaq(): ...` |
| `License check failed` | Ensure IDA Pro license is valid; free IDA lacks idalib |
| Database lock error | Close any running IDA instance using same .idb file |
| Python version mismatch | Use the exact Python version bundled with your IDA (check `$IDADIR/python3`) |
| OEM license required | Commercial embedding requires separate OEM license from Hex-Rays |
