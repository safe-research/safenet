#!/usr/bin/env python3
"""CA3-FIX end-to-end check of F-SEN-015's fix on the real sentinel binary.

Run inside `integ` (private network namespace):
  python3 -I ca3fix_f_sen_015_e2e.py <worktree> <out-dir> <iterations> <replay-mode> <down-blocks>

replay-mode: what the engine answers after the restart, `down` (HTTP 503, an
engine still starting) or `flip` (a different verdict, R-3.4).
down-blocks: blocks mined while the sentinel is stopped. With the crash right
after processing the commit block C = P + 2 and `max_reorg_depth` 5, 4 puts
the restart head at P + 6, so the new safe block is P + 1: the proposal at P
is replayed inside the warp below the new safe block, and the sentinel's own
commitment at P + 2 arrives in the live tail.

Per iteration: propose a transaction (block P); the engine answers the first
check R-2.1 only after block P + 1 is mined, so the commitment lands at
P + 2; the sentinel is killed (SIGKILL) once it has processed P + 2; blocks are
mined while it is down; the engine is switched to `replay-mode`; the sentinel
is restarted on the same database; blocks are mined one at a time through the
reveal window. Recorded: whether the engine was asked again for the same
request after the restart (the recorded verdict was not used), whether a
`Revealed` event for the sentinel exists, and the onchain commitment state.
"""
import json
import os
import pathlib
import signal
import sqlite3
import subprocess
import sys
import time

WT = pathlib.Path(sys.argv[1])
OUT = pathlib.Path(sys.argv[2])
N = int(sys.argv[3])
REPLAY = sys.argv[4]
DOWN = int(sys.argv[5])
assert REPLAY in ("down", "flip")
RPC = "http://127.0.0.1:8545"
CHAIN_ID = 31337
DEPLOYER = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
DEPLOYER_PK = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
ENGINE_PORT = 5473
COMMIT_WINDOW = 6
REVEAL_WINDOW = 6
REQUEST_FEE = 1000
SAFE = "0x1111111111111111111111111111111111111111"
ZERO = "0x0000000000000000000000000000000000000000"
OUT.mkdir(parents=True, exist_ok=True)
STATE = OUT / "engine"
STATE.mkdir(exist_ok=True)
DB = OUT / "sentinel_a.sqlite"
PROCS = []
REPORT = OUT / "report.txt"


def log(msg):
    line = f"[{time.strftime('%H:%M:%S')}] {msg}"
    print(line, flush=True)
    with REPORT.open("a") as f:
        f.write(line + "\n")


def sh(*args, env=None, check=True):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True, env=env)
    if check and result.returncode != 0:
        raise RuntimeError(f"{args[:3]} failed: {result.stderr[-2000:]}")
    return result.stdout.strip()


def rpc(method, *params):
    return sh("cast", "rpc", "--rpc-url", RPC, method, *params)


def mine(n=1):
    for _ in range(n):
        rpc("evm_mine")


def head():
    return int(sh("cast", "block-number", "--rpc-url", RPC))


def processed():
    if not DB.exists():
        return -1
    try:
        con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True, timeout=5)
        row = con.execute("SELECT MAX(block_number) FROM snapshots").fetchone()
        con.close()
        return -1 if row[0] is None else int(row[0])
    except sqlite3.Error:
        return -1


def wait(pred, timeout, what):
    end = time.time() + timeout
    while time.time() < end:
        if pred():
            return True
        time.sleep(0.05)
    raise RuntimeError(f"timeout waiting for {what}")


def engine_requests(request_id=None, after=0.0):
    path = STATE / "requests.log"
    if not path.exists():
        return []
    out = []
    for line in path.read_text().splitlines():
        entry = json.loads(line)
        if entry.get("event") != "arrived":
            continue
        if entry["t"] >= after and (request_id is None or entry["request_id"].lower() == request_id.lower()):
            out.append(entry)
    return out


def set_engine(mode, release="0"):
    (STATE / "mode").write_text(mode)
    (STATE / "release").write_text(release)


def forge(script, env_extra):
    env = dict(os.environ, FACTORY="2", **env_extra)
    sh("forge", "script", "--root", WT / "contracts", script, "--rpc-url", RPC,
       "--private-key", DEPLOYER_PK, "--broadcast", env=env)


def broadcast_return(script_file, key):
    data = json.loads((WT / "contracts/build/broadcast" / script_file / str(CHAIN_ID) / "run-latest.json").read_text())
    return data["returns"][key]["value"]


def start_sentinel(cfg, mode):
    logf = (OUT / "sentinel_a_logs.txt").open(mode)
    p = subprocess.Popen([str(WT / "target/debug/sentinel"), "--config-file", str(cfg)], stdout=logf, stderr=subprocess.STDOUT)
    PROCS.append(p)
    return p


def cleanup():
    for p in PROCS:
        if p.poll() is None:
            p.kill()
            p.wait()


def words(data_hex):
    data = data_hex[2:] if data_hex.startswith("0x") else data_hex
    return [int(data[i:i + 64], 16) for i in range(0, len(data), 64)]


def main():
    (REPORT).write_text("")
    log(f"F-SEN-015 E2E: iterations={N} replay-mode={REPLAY} down-blocks={DOWN}")
    anvil_log = (OUT / "anvil_logs.txt").open("w")
    PROCS.append(subprocess.Popen(["anvil", "--port", "8545"], stdout=anvil_log, stderr=subprocess.STDOUT))
    wait(lambda: subprocess.run(["cast", "block-number", "--rpc-url", RPC], capture_output=True).returncode == 0, 20, "anvil")

    forge("DeployERC20Script", {})
    fee_token = broadcast_return("DeployERC20.s.sol", "erc20")
    forge("DeployTestConsensusScript", {})
    consensus = broadcast_return("DeployTestConsensus.s.sol", "consensus")
    forge("DeploySentinelOracleScript", {
        "SENTINEL_ARBITRATOR": DEPLOYER, "SENTINEL_GOVERNANCE": DEPLOYER,
        "SENTINEL_PROTOCOL_FUNDS_RECEIVER": DEPLOYER, "SENTINEL_CONSENSUS": consensus,
        "SENTINEL_FEE_TOKEN": fee_token, "SENTINEL_REQUEST_FEE": str(REQUEST_FEE),
        "SENTINEL_COMMIT_WINDOW": str(COMMIT_WINDOW), "SENTINEL_REVEAL_WINDOW": str(REVEAL_WINDOW),
        "SENTINEL_GOVERNANCE_DELAY": "0", "SENTINEL_BOND_MULTIPLIER": "4",
        "SENTINEL_INITIAL_SLASHING_MULTIPLIER": "2", "SENTINEL_INITIAL_DAO_FEE_SHARE": "0",
        "SENTINEL_CHARTER_ENS": "safenet-charter.safe.eth", "SENTINEL_ARBITRATION_TIMEOUT": "20",
    })
    oracle = broadcast_return("DeploySentinelOracle.s.sol", "sentinelOracle")
    log(f"fee token {fee_token}, consensus {consensus}, oracle {oracle}")

    wallets = json.loads(sh("cast", "wallet", "new", "--json", "--number", "2"))
    sentinel, sentinel_pk = wallets[0]["address"], wallets[0]["private_key"]
    sponsor, sponsor_pk = wallets[1]["address"], wallets[1]["private_key"]
    for addr in (sentinel, sponsor):
        sh("cast", "send", "--rpc-url", RPC, "--private-key", DEPLOYER_PK, "--value", "10ether", addr)
        sh("cast", "send", "--rpc-url", RPC, "--private-key", DEPLOYER_PK, fee_token,
           "transfer(address,uint256)", addr, "100000000")
    sh("cast", "send", "--rpc-url", RPC, "--private-key", DEPLOYER_PK, oracle, "addSentinel(address)", sentinel)
    sh("cast", "send", "--rpc-url", RPC, "--private-key", sponsor_pk, fee_token,
       "approve(address,uint256)", oracle, "100000000")
    log(f"sentinel {sentinel}, sponsor {sponsor}")

    engine_log = (OUT / "engine_logs.txt").open("w")
    set_engine("gated", "0")
    PROCS.append(subprocess.Popen([sys.executable, "-I", str(pathlib.Path(__file__).parent / "ca3fix_engine.py"),
                                   str(ENGINE_PORT), str(STATE)], stdout=engine_log, stderr=subprocess.STDOUT))

    cfg = OUT / "sentinel_a.toml"
    cfg.write_text(
        f'rpc = "{RPC}"\nsigner = "{sentinel_pk}"\ndatabase = "sqlite://{DB}?mode=rwc"\n'
        f'oracle = "{oracle}"\nconsensus = "{consensus}"\n\n'
        f'[sentinel]\nfee_token = "{fee_token}"\nvoting_window = {COMMIT_WINDOW + REVEAL_WINDOW}\n'
        f'engine = "http://127.0.0.1:{ENGINE_PORT}"\n\n'
        f'[observability]\nlog_filter = "info,safenet_core=debug,sentinel=debug"\n\n'
        f'[index]\nblock_time = 1000\nblock_propagation_delay = 0\nblock_retry_delays = []\n'
    )
    rpc("evm_setAutomine", "false")
    proc = start_sentinel(cfg, "w")
    for _ in range(8):
        mine(1)
        time.sleep(0.3)
    wait(lambda: processed() >= head(), 30, "initial sync")
    log(f"sentinel synced at block {processed()}")

    results = []
    for i in range(N):
        set_engine("gated", "0")
        h0 = head()
        sh("cast", "send", "--async", "--rpc-url", RPC, "--private-key", sponsor_pk, consensus,
           "proposeTransaction(address,bytes,(uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256))",
           oracle, "0x", f"({CHAIN_ID},{SAFE},{SAFE},0,0x,0,0,0,0,{ZERO},{ZERO},{1000 + i})")
        time.sleep(0.2)
        mine(1)
        p_block = h0 + 1
        logs = json.loads(sh("cast", "logs", "--json", "--rpc-url", RPC, "--from-block", str(p_block),
                             "--to-block", str(p_block), "--address", oracle,
                             "NewRequest(bytes32,address,uint96,uint96,uint24,uint96,uint64,uint64)"))
        assert len(logs) == 1, f"expected one NewRequest at {p_block}: {logs}"
        rid = logs[0]["topics"][1]
        w = words(logs[0]["data"])
        commit_deadline, reveal_deadline = w[4], w[5]
        wait(lambda: len(engine_requests(rid)) >= 1, 15, "first engine check")
        mine(1)
        wait(lambda: processed() >= p_block + 1, 15, "processing P + 1")
        set_engine("gated", "1")
        latest_nonce = int(sh("cast", "nonce", "--rpc-url", RPC, sentinel))
        wait(lambda: int(sh("cast", "nonce", "--rpc-url", RPC, "--block", "pending", sentinel)) > latest_nonce, 15, "commit submission")
        time.sleep(0.8)
        mine(1)
        c_block = p_block + 2
        committed = json.loads(sh("cast", "logs", "--json", "--rpc-url", RPC, "--from-block", str(c_block),
                                  "--to-block", str(c_block), "--address", oracle,
                                  "Committed(bytes32,address,uint96)", rid))
        if not committed:
            log(f"iter {i}: no Committed at {c_block}; skipping")
            results.append({"iter": i, "skipped": "no commit at P+2"})
            continue
        wait(lambda: processed() >= c_block, 15, "processing C")
        proc.send_signal(signal.SIGKILL)
        proc.wait()
        mine(DOWN)
        set_engine(REPLAY, "0")
        t_restart = time.time()
        proc = start_sentinel(cfg, "a")
        restart_head = head()
        wait(lambda: processed() >= restart_head, 30, "replay after restart")
        time.sleep(1.5)
        while head() < reveal_deadline + 2:
            mine(1)
            target = head()
            wait(lambda: processed() >= target, 15, f"processing {target}")
            time.sleep(0.4)
        again = engine_requests(rid, after=t_restart)
        revealed = json.loads(sh("cast", "logs", "--json", "--rpc-url", RPC, "--from-block", str(p_block),
                                 "--to-block", "latest", "--address", oracle,
                                 "Revealed(bytes32,address,bool,uint96,string)", rid))
        reason = None
        for ev in revealed:
            if ev["topics"][2][-40:].lower() == sentinel[2:].lower():
                data = ev["data"][2:]
                length = int(data[192:256], 16)
                reason = bytes.fromhex(data[256:256 + 2 * length]).decode()
        commitment = sh("cast", "call", "--rpc-url", RPC, oracle,
                        "getCommitment(bytes32,address)((bytes32,uint96,uint8,bool))", rid, sentinel)
        outcome = {
            "iter": i, "P": p_block, "C": c_block, "restart_head": restart_head,
            "commit_deadline": commit_deadline, "reveal_deadline": reveal_deadline,
            "engine_asked_again": len(again), "replay_modes": [e["mode"] for e in again],
            "revealed_reason": reason, "commitment": commitment,
        }
        results.append(outcome)
        log(f"iter {i}: P={p_block} C={c_block} H={restart_head} engine asked again={len(again)} "
            f"revealed={reason!r} commitment={commitment}")

    done = [r for r in results if "skipped" not in r]
    lost = [r for r in done if r["engine_asked_again"] > 0]
    ok = [r for r in done if r["revealed_reason"] == "R-2.1"]
    log(f"SUMMARY replay-mode={REPLAY} down-blocks={DOWN}: {len(done)} runs, engine asked again after the restart in {len(lost)}, "
        f"revealed the committed reason in {len(ok)}, no or wrong reveal in {len(done) - len(ok)}")
    (OUT / "results.json").write_text(json.dumps(results, indent=1))


try:
    main()
finally:
    cleanup()
