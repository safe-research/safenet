#!/usr/bin/env python3
"""Comment out one exact line in a file (asserted). Usage: mutate_line.py <file> <exact stripped line>

Revert with `git checkout -- <file>`.
"""
import pathlib
import sys

f = pathlib.Path(sys.argv[1])
target = sys.argv[2]
lines = f.read_text().split("\n")
hits = [i for i, line in enumerate(lines) if line.strip() == target]
assert len(hits) == 1, f"expected exactly one match for {target!r}, got {len(hits)}"
i = hits[0]
indent = lines[i][: len(lines[i]) - len(lines[i].lstrip())]
lines[i] = indent + "// MUTATION (CA3-FIX): " + lines[i].strip()
f.write_text("\n".join(lines))
assert f.read_text().count("// MUTATION (CA3-FIX): " + target) == 1
print(f"mutated {f}:{i + 1}")
