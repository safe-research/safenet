#!/usr/bin/env python3
"""F3-CORE-001: a rate-limited RPC stalls the indexer for good.

Route (real sentinel binary behind a JSON-RPC proxy, distinct from the suggested unit test): the
proxy serves everything normally during warmup, then a short eth_getLogs blackout forces three
single-query failures on a new block so the event watcher escalates to one eth_getLogs per watched
event (11 for the sentinel). From then on the proxy caps concurrent eth_getLogs below 11 (a limit
that would serve a single query but cannot admit 11 at once), so no per-event attempt ever sees all
11 succeed. The driver retries every 100 ms forever, /health keeps returning OK, and the indexer's
seen block never advances. Gnosis timings (chain 100) throughout.
"""
import collections
import signal
import sys
import threading
import time

sys.path.insert(0, "/home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3/scratch/QA3-2B")
import h  # noqa: E402

BINARY = sys.argv[1] if len(sys.argv) > 1 else f"{h.SCRATCH}/bin/sentinel-361aaf3"
LABEL = sys.argv[2] if len(sys.argv) > 2 else "repro"

WARMUP_S = 12.0
BLACKOUT_S = 12.0     # spans at least two 5 s blocks so a new block's single query fails >=3 times
CONCURRENCY_CAP = 4   # below the 11 watched events, so a per-event attempt never fully succeeds

run_dir = f"{h.SCRATCH}/runs/core001-{LABEL}-{int(time.time())}"
anvil = h.Anvil(port=31700)
proxy = None
a = None
try:
    s_addr, s_pk = h.ACCT[1]
    dep = h.Deployment([(s_addr, s_pk)])
    h.rpc("evm_setAutomine", [False])
    miner = h.Miner()
    miner.start()

    t_begin = time.time()
    recent = collections.deque()
    lock = threading.Lock()

    def policy(method, params, raw):
        if method != "eth_getLogs":
            return "forward"
        now = time.time()
        phase = now - t_begin
        if phase < WARMUP_S:
            return "forward"
        if phase < WARMUP_S + BLACKOUT_S:
            return ("http", 429)  # blackout: forces entry into per-event mode
        with lock:
            while recent and now - recent[0] > 0.08:
                recent.popleft()
            inflight = len(recent)
            recent.append(now)
        if inflight >= CONCURRENCY_CAP:
            return ("http", 429)  # concurrency cap below 11: per-event never fully succeeds
        return "forward"

    proxy = h.JsonRpcProxy(31702, anvil.url, policy)
    eng = h.Engine(31703, "A")
    a = h.Sentinel("A", s_pk, dep, 31703, 31705, BINARY, run_dir, rpc_url=proxy.url)
    a.start()
    a.wait_log("starting sentinel service", 30)

    # Warmup: confirm the indexer advances while the proxy serves logs.
    time.sleep(WARMUP_S - 2)
    warm_seen = a.metric("safenet_core_block_number")
    h.log(f"after warmup, block metrics: {warm_seen}; /health={h.health(31705)}")

    # Through the blackout and into the capped phase; let the stall run.
    time.sleep(BLACKOUT_S + 40)
    stall_seen = a.metric("safenet_core_block_number")
    status, body = h.health(31705)
    h.log(f"during stall: block metrics={stall_seen}; /health=({status}, {body!r}); anvil head={h.block_number()}")

    # Observe a little longer to show the seen block does not move while requests keep flowing.
    t_mark = time.time() - h.T0
    n_before = sum(1 for t, m in proxy.calls if m == "eth_getLogs" and t < t_mark)
    time.sleep(12)
    stall_seen2 = a.metric("safenet_core_block_number")
    n_after = sum(1 for t, m in proxy.calls if m == "eth_getLogs")
    rate = (n_after - n_before) / 12.0
    h.log(f"12 s later: block metrics={stall_seen2}; eth_getLogs rate ~{rate:.0f}/s; alive={a.proc.poll() is None}")

    bursts = proxy.getlogs_bursts()
    h.log(f"last eth_getLogs bursts (t, count) during stall: {bursts[-8:]}")
    per_event = [c for _, c in bursts if c >= 8]
    h.log(f"number of >=8-wide (per-event) bursts: {len(per_event)}; sizes seen: {sorted(set(c for _, c in bursts))}")
    warns = [ln for ln in a.lines() if 'failed to get next blockchain update' in str(ln.get('fields', {}).get('message', ''))]
    h.log(f"'failed to get next blockchain update; retrying after delay' warn count: {len(warns)}")
    print("=== verdict ===")
    def seen(metrics):
        for ln in metrics:
            if 'status="seen"' in ln:
                return float(ln.split()[-1])
        return None
    ws, ss, ss2 = seen(warm_seen), seen(stall_seen), seen(stall_seen2)
    print(f"seen_block warmup={ws} stall={ss} stall+12s={ss2}")
    print("indexer_stalled =", ss is not None and ss2 is not None and ss == ss2 and h.block_number() > ss + 2)
    print("health_ok_while_stalled =", (status, body) == (200, "OK"))
    print("process_alive =", a.proc.poll() is None)
    print("escalated_to_per_event_11 =", any(c >= 11 for _, c in bursts))
    print("high_request_rate =", rate > 40)
finally:
    try:
        if a and a.proc and a.proc.poll() is None:
            a.stop(signal.SIGKILL)
    except Exception:
        pass
    if proxy:
        proxy.stop()
    anvil.stop()
