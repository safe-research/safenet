"""Trim the repetitive post-teardown "connection refused / reset" driver spam
from a sentinel log, keeping every other line plus a one-line count of what was
dropped. Overwrites the file in place. Usage: python3 -I trim_logs.py <log>..."""

import sys

for path in sys.argv[1:]:
    kept, dropped = [], 0
    for line in open(path):
        if ("failed to get next blockchain update" in line) or (
            "ConnectionRefused" in line or "ConnectionReset" in line
        ):
            dropped += 1
            continue
        kept.append(line)
    kept.append(f'[trimmed] dropped {dropped} repeated post-teardown connection-refused/reset lines\n')
    open(path, "w").writelines(kept)
    print(f"{path}: kept {len(kept)} lines, dropped {dropped}")
