"""Insert a test snippet before the final closing brace of a Rust file.

Usage: python3 -I insert_tests.py <target.rs> <snippet.rs>

Asserts the target ends with the closing brace of its trailing `mod tests`
and that the snippet is not already present.
"""

import sys

target, snippet = sys.argv[1], sys.argv[2]
with open(target, encoding="utf-8") as f:
    source = f.read()
with open(snippet, encoding="utf-8") as f:
    addition = f.read()

marker = next(line for line in addition.splitlines() if "Run 3 re-validation" in line)
assert marker not in source, f"snippet already present in {target}"
stripped = source.rstrip()
assert stripped.endswith("}"), f"{target} does not end with a closing brace"
assert "#[cfg(test)]" in source, f"{target} has no test module"
index = stripped.rfind("}")
new_source = stripped[:index] + "\n" + addition + "}\n"
assert new_source.count(marker) == 1
with open(target, "w", encoding="utf-8") as f:
    f.write(new_source)
print(f"inserted {len(addition.splitlines())} lines into {target}")
