#!/usr/bin/env python3
"""Insert a PoC module file before the final closing brace of a tracked file's `mod tests` block.
usage: paste.py <tracked-file> <poc-file>   (idempotent: refuses if a marker is already present)"""
import sys
tracked, poc = sys.argv[1], sys.argv[2]
src = open(tracked).read()
add = open(poc).read()
marker = add.strip().splitlines()[0]
assert marker not in src, "already pasted"
assert src.endswith("}\n"), "tracked file must end with the tests block's closing brace"
open(tracked, "w").write(src[:-2] + "\n" + add.rstrip("\n") + "\n}\n")
print(f"pasted {len(add.splitlines())} lines into {tracked}")
