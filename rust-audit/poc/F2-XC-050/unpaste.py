#!/usr/bin/env python3
"""Remove exactly the block paste.py inserted (from the PoC's first line to the line before the final brace),
leaving any other hunk in the file untouched. usage: unpaste.py <tracked-file> <poc-file>"""
import sys
tracked, poc = sys.argv[1], sys.argv[2]
src = open(tracked).read()
add = open(poc).read().rstrip("\n") + "\n"
needle = "\n" + add + "}\n"
assert src.endswith(needle), "the pasted block is not at the end of the file as inserted"
open(tracked, "w").write(src[: -len(needle)] + "}\n")
print(f"removed {len(add.splitlines())} lines from {tracked}")
