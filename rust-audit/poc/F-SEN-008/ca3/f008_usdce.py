"""Measure the Aegis fee token's `approve` against the sentinel's hard-coded 55,000 gas limit.

Loads the USDC.e proxy and implementation code plus the proxy's storage slots from a local
Foundry fork cache (Gnosis block 48,635,154, written by an earlier run) into a loopback Anvil,
then sends the sentinel's exact call, `approve(SentinelOracle, bondTarget)`, as a direct
transaction with gas limit 55,000 (Direct mode, the default), and once more with a high limit
to read the gas it really uses. No live RPC is contacted.

usage: f008_usdce.py <foundry-cache-json> <anvil-port>
"""
import json
import subprocess
import sys
import urllib.request

CACHE, PORT = sys.argv[1], sys.argv[2]
RPC = f"http://127.0.0.1:{PORT}"
TOKEN = "0x2a22f9c3b484c3629090feed35f17ff8f88f76f0"  # USDC.e, contracts/.env.aegis.sample:26
IMPL = "0x107cf7fb73ea48d1d200989b156ce1894d7afec7"  # FiatTokenV2_2 behind the proxy
ORACLE = "0x4F61B8832978e83b80D69551AEf07557DBE41d03"  # Aegis SentinelOracle (overview)
BOND = 800_000_000  # bondTarget = 0.40 USDC.e x 2000 (6 decimals)
# Anvil's well-known development keys (local chain only).
KEYS = [
    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
    "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a",
]


def rpc(method, params):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(RPC, data=body, headers={"content-type": "application/json"})
    with urllib.request.urlopen(req) as r:
        out = json.loads(r.read())
    if "error" in out:
        raise RuntimeError(out["error"])
    return out["result"]


def cast(*args):
    return subprocess.run(["cast", *args, "--rpc-url", RPC], capture_output=True, text=True)


def cast_local(*args):
    return subprocess.run(["cast", *args], capture_output=True, text=True)


cache = json.load(open(CACHE))
print("fork cache meta:", json.dumps(cache["meta"].get("block_env", {}))[:200])
for addr in (TOKEN, IMPL):
    analyzed = cache["accounts"][addr]["code"]["LegacyAnalyzed"]
    # revm pads analyzed bytecode; keep the original length.
    code = analyzed["bytecode"][: 2 + 2 * analyzed["original_len"]]
    rpc("anvil_setCode", [addr, code])
    print(addr, "code bytes", (len(code) - 2) // 2)
for slot, value in cache["storage"][TOKEN].items():
    slot_hex = "0x" + int(slot, 0).to_bytes(32, "big").hex()
    value_hex = "0x" + int(value, 0).to_bytes(32, "big").hex()
    rpc("anvil_setStorageAt", [TOKEN, slot_hex, value_hex])
print("proxy slots loaded:", len(cache["storage"][TOKEN]))
ZOS_IMPL = "0x7050c9e0f4ca769c69bd3a8ef740bc37934f8e2c036e5a723fd8ee048ed3f8c3"
ZOS_ADMIN = "0x10d6a54a4754c8869d6886b5f5d7fbfa5b4522237ea5c60d11bc4e7a1ff9390b"
print("impl slot:", rpc("eth_getStorageAt", [TOKEN, ZOS_IMPL, "latest"]))
print("admin slot:", rpc("eth_getStorageAt", [TOKEN, ZOS_ADMIN, "latest"]))
print("symbol:", cast("call", TOKEN, "symbol()(string)").stdout.strip())
print("decimals:", cast("call", TOKEN, "decimals()(uint8)").stdout.strip())
print("paused:", cast("call", TOKEN, "paused()(bool)").stdout.strip())

data = cast_local("calldata", "approve(address,uint256)", ORACLE, str(BOND)).stdout.strip()
print("approve calldata:", data)


def send(key, gas):
    r = cast("send", "--private-key", key, "--gas-limit", str(gas), TOKEN, data, "--json")
    if r.returncode != 0:
        print("send failed:", r.stderr.strip()[:400])
        return None
    rec = json.loads(r.stdout)
    return int(rec["status"], 16), int(rec["gasUsed"], 16)


# 1. The sentinel's transaction as encoded at crates/sentinel/src/service.rs:867-877: gas 55,000.
status, used = send(KEYS[0], 55_000)
print(f"[F-SEN-008 run3] approve 0 -> {BOND} with gas limit 55000: status={status} gasUsed={used}")
print("allowance after:", cast("call", TOKEN, "allowance(address,address)(uint256)",
      cast_local("wallet", "address", "--private-key", KEYS[0]).stdout.strip(), ORACLE).stdout.strip())
# 2. The same call with room to spare, from a fresh account: what the call really needs.
status, used = send(KEYS[1], 200_000)
print(f"[F-SEN-008 run3] approve 0 -> {BOND} with gas limit 200000: status={status} gasUsed={used}")
# 3. Non-zero to non-zero (an allowance left by an earlier approve that no commit consumed).
status, used = send(KEYS[1], 55_000)
print(f"[F-SEN-008 run3] approve {BOND} -> {BOND} (non-zero slot) with gas limit 55000: status={status} gasUsed={used}")
