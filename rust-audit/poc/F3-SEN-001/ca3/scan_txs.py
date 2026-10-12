"""Print every transaction sent by one account across a block range, with its
receipt status and gas used, decoding the target and the 4-byte selector.
Localhost JSON-RPC only (offline). Usage:

  python3 -I scan_txs.py <rpc> <from_addr> <start_block> <end_block> <usdc> <oracle>
"""

import json
import sys
import urllib.request

rpc, who, start, end, usdc, oracle = (
    sys.argv[1],
    sys.argv[2].lower(),
    int(sys.argv[3]),
    int(sys.argv[4]),
    sys.argv[5].lower(),
    sys.argv[6].lower(),
)

# SentinelOracle / ERC20 selectors we care about.
NAMES = {
    "0x095ea7b3": "ERC20.approve",
    "0xe3ce094d": "oracle.commit",
    "0x386a15bc": "oracle.reveal",
    "0x92584d80": "oracle.finalize",
    "0xbd66528a": "oracle.claim",
}


def call(method, params):
    req = urllib.request.Request(
        rpc,
        data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req) as r:
        return json.load(r)["result"]


def h2i(x):
    return int(x, 16) if isinstance(x, str) else x


n = 0
for b in range(start, end + 1):
    blk = call("eth_getBlockByNumber", [hex(b), True])
    if not blk:
        continue
    for tx in blk["transactions"]:
        if (tx["from"] or "").lower() != who:
            continue
        rc = call("eth_getTransactionReceipt", [tx["hash"]])
        to = (tx.get("to") or "").lower()
        sel = (tx.get("input") or "0x")[:10]
        tgt = "USDC.e" if to == usdc else ("oracle" if to == oracle else to)
        name = NAMES.get(sel, sel)
        status = h2i(rc["status"])
        print(
            f"   block {b}  nonce {h2i(tx['nonce'])}  -> {tgt} {name}"
            f"  status={status} ({'OK' if status else 'FAILED'})"
            f"  gasLimit={h2i(tx['gas'])}  gasUsed={h2i(rc['gasUsed'])}"
        )
        n += 1
if n == 0:
    print("   (no transactions from this account in range)")
