"""Emit the real Gnosis USDC.e proxy + implementation runtime code and the
proxy's 14 storage slots from Foundry's RPC cache (Gnosis block 48635154), so a
local Anvil can carry the exact deployed token. Offline; reads the cache only.

Writes, into the directory given as argv[1]:
  proxy.code   hex runtime code of FiatTokenProxy  0x2a22...76F0
  impl.code    hex runtime code of FiatTokenV2_2   0x107C...EC7
  slots.txt    one "<slot> <value>" pair per line, the proxy's cached storage
It prints the code hashes to stderr for cross-checking against the finding.
"""

import json
import os
import sys

CACHE = os.path.expanduser("~/.foundry/cache/rpc/xdai/48635154")
PROXY = "0x2a22f9c3b484c3629090feed35f17ff8f88f76f0"
IMPL = "0x107cf7fb73ea48d1d200989b156ce1894d7afec7"

out_dir = sys.argv[1]
d = json.load(open(CACHE))


def runtime(addr):
    la = d["accounts"][addr]["code"]["LegacyAnalyzed"]
    bc, n = la["bytecode"], la["original_len"]
    body = bc[2 : 2 + 2 * n] if bc.startswith("0x") else bc[: 2 * n]
    return "0x" + body, d["accounts"][addr]["code_hash"], n


for name, addr in (("proxy", PROXY), ("impl", IMPL)):
    code, code_hash, n = runtime(addr)
    open(os.path.join(out_dir, f"{name}.code"), "w").write(code)
    print(f"{name} {addr} len={n} code_hash={code_hash}", file=sys.stderr)

slots = d["storage"][PROXY]
with open(os.path.join(out_dir, "slots.txt"), "w") as f:
    for slot, val in slots.items():
        s = "0x" + format(int(slot, 16), "064x")
        v = "0x" + format(int(val, 16), "064x")
        f.write(f"{s} {v}\n")
print(f"slots={len(slots)}", file=sys.stderr)
print("block", int(d["meta"]["block_env"]["number"], 16), file=sys.stderr)
