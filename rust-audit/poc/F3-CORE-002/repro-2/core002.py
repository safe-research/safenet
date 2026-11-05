#!/usr/bin/env python3
"""F3-CORE-002: with use_client_filtering, the integrity check gives up after three attempts and
accepts the empty answer it was meant to catch, losing a block's watched events for good.

Route (real sentinel binary with use_client_filtering=true, behind a proxy): the proxy returns an
empty eth_getLogs for the proposal block's hash (header bloom is non-zero, it holds NewRequest and
TransactionProposed), for longer than the three client-filtered attempts. After three bloom
mismatches the watcher falls back to per-event node-filtered queries, whose empty answer is
accepted, so the sentinel commits the proposal block with no events, never runs an engine check,
and never votes on the request. Gnosis timings; the proposal block's logs are the only ones
suppressed.
"""
import signal
import sys
import threading
import time

sys.path.insert(0, "/home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3/scratch/QA3-2B")
import h  # noqa: E402

BINARY = sys.argv[1] if len(sys.argv) > 1 else f"{h.SCRATCH}/bin/sentinel-361aaf3"
LABEL = sys.argv[2] if len(sys.argv) > 2 else "repro"
SUPPRESS_S = float(sys.argv[3]) if len(sys.argv) > 3 else 40.0  # > the ~0.3 s the 3 attempts cover

run_dir = f"{h.SCRATCH}/runs/core002-{LABEL}-{int(time.time())}"
anvil = h.Anvil(port=31700)
proxy = None
a = None
try:
    s_addr, s_pk = h.ACCT[1]
    dep = h.Deployment([(s_addr, s_pk)])
    h.rpc("evm_setAutomine", [False])
    miner = h.Miner()
    miner.start()

    state = {"target_hash": None, "suppress_until": 0.0}
    lock = threading.Lock()

    def policy(method, params, raw):
        if method != "eth_getLogs":
            return "forward"
        with lock:
            th, until = state["target_hash"], state["suppress_until"]
        if th and params and isinstance(params[0], dict) and params[0].get("blockHash", "").lower() == th:
            if time.time() < until:
                return ("result", [])  # node returns an empty log set for this block's hash
        return "forward"

    proxy = h.JsonRpcProxy(31702, anvil.url, policy)
    eng = h.Engine(31703, "A")
    a = h.Sentinel("A", s_pk, dep, 31703, 31705, BINARY, run_dir, rpc_url=proxy.url,
                   index_extra="use_client_filtering = true")
    a.start()
    a.wait_log("starting sentinel service", 30)
    time.sleep(6)

    # Propose; capture the proposal block and its hash, then suppress that block's logs.
    rid = dep.request_id(0)
    dep.propose(wait=False)
    miner.wait_block(h.block_number() + 1)
    p = h.block_number()
    blk = h.rpc("eth_getBlockByNumber", [hex(p), False])
    with lock:
        state["target_hash"] = blk["hash"].lower()
        state["suppress_until"] = time.time() + SUPPRESS_S
    h.log(f"proposal in block {p} hash={blk['hash']} logsBloom_nonzero={int(blk['logsBloom'],16)!=0}; suppressing its logs for {SUPPRESS_S}s")

    # Sample well inside the suppression window. Pristine: block p is committed empty and the
    # indexer advances past it. Fixed (option 2): the watcher treats the empty answer as suspect
    # and stalls on block p, so seen stays below p.
    time.sleep(18)
    seen = a.metric("safenet_core_block_number")
    req = dep.get_request(rid)
    def seen_val(metrics, status):
        for ln in metrics:
            if f'status="{status}"' in ln:
                return float(ln.split()[-1])
        return None
    during_seen = seen_val(seen, "seen")
    h.log(f"while suppressed (anvil head {h.block_number()}): seen={during_seen} metrics={seen}")
    h.log(f"  onchain committed={req['committed']} state={req['state']}; engine checks={len(eng.requests)}")

    # Wait for suppression to end, then give the watcher time to read block p and vote.
    while time.time() < state["suppress_until"] + 2:
        time.sleep(1)
    time.sleep(30)
    req2 = dep.get_request(rid)
    after_seen = seen_val(a.metric("safenet_core_block_number"), "seen")
    h.log(f"after suppression ended: seen={after_seen}")
    h.log(f"onchain request committed={req2['committed']} state={req2['state']}; engine checks={len(eng.requests)}")
    mism = [ln for ln in a.lines() if 'bloom filter mismatch' in str(ln.get('fields', {}).get('message', ''))]
    h.log(f"'incomplete logs served for block, bloom filter mismatch' warn count: {len(mism)}")
    s_txs = dep.txs_from(s_addr, p, h.block_number())
    h.log(f"sentinel onchain txns (expect none for the request): {s_txs}")
    print("=== verdict ===")
    print("proposal_block_has_watched_bloom =", int(blk["logsBloom"], 16) != 0)
    print(f"seen_during_suppression={during_seen} (proposal block p={p})")
    print("advanced_past_proposal_while_suppressed =", during_seen is not None and during_seen > p)
    print("stalled_on_proposal_while_suppressed =", during_seen is not None and during_seen < p)
    print("bloom_mismatch_count =", len(mism))
    print("engine_ran_eventually =", len(eng.requests) > 0)
    print("request_committed_after =", req2["committed"] >= 1)
    print("sentinel_lost_the_event =", len(eng.requests) == 0 and not s_txs and req2["committed"] == 0)
finally:
    try:
        if a and a.proc and a.proc.poll() is None:
            a.stop(signal.SIGKILL)
    except Exception:
        pass
    if proxy:
        proxy.stop()
    anvil.stop()
