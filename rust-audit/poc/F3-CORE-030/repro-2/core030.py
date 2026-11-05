#!/usr/bin/env python3
"""F3-CORE-030: a restart against an RPC node behind the oldest retained snapshot exits with
BadUpdate (and status 0), repeating on every restart until the node catches up.

Route (second reproduction, real sentinel binary + two real Anvils): run the real `sentinel`
against Anvil1 until it has processed and snapshotted to block L, stop it, then restart it
against a second Anvil forked far behind L. No mocks, no vm.store: the stale head at startup is
the whole trigger.
"""
import signal
import sys
import time

sys.path.insert(0, "/home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3/scratch/QA3-2B")
import h  # noqa: E402

BINARY = sys.argv[1] if len(sys.argv) > 1 else f"{h.SCRATCH}/bin/sentinel-361aaf3"
LABEL = sys.argv[2] if len(sys.argv) > 2 else "repro"
FORK_AT = 12          # second Anvil forks here (far below the oldest retained snapshot)
L_TARGET = 30         # run the first node to at least this block before stopping

run_dir = f"{h.SCRATCH}/runs/core030-{LABEL}-{int(time.time())}"
anvil1 = h.Anvil(port=31700)
anvil2 = None
a = None
try:
    s_addr, s_pk = h.ACCT[1]
    dep = h.Deployment([(s_addr, s_pk)])
    h.rpc("evm_setAutomine", [False])
    miner = h.Miner(url=anvil1.url)
    miner.start()
    eng = h.Engine(31703, "A")
    a = h.Sentinel("A", s_pk, dep, 31703, 31705, BINARY, run_dir, rpc_url=anvil1.url)
    a.start()
    a.wait_log("starting sentinel service", 30)
    # Let it follow the chain and snapshot every block up to L.
    miner.wait_block(L_TARGET + 2)
    time.sleep(4)
    seen = a.metric("safenet_core_block_number")
    h.log(f"before stop: anvil1 head={h.block_number(anvil1.url)}, sentinel block metrics={seen}")
    a.stop(signal.SIGTERM)
    h.log(f"run 1 exit status: {a.proc.returncode}")

    # Second node: same history, forked far behind L, mining slowly forward.
    anvil2 = h.Anvil(port=31701, extra=["--fork-url", anvil1.url, "--fork-block-number", str(FORK_AT),
                                        "--block-time", "1"])
    h.log(f"anvil2 forked at {FORK_AT}, head now {h.block_number(anvil2.url)}")
    a2 = h.Sentinel("A", s_pk, dep, 31703, 31706, BINARY, run_dir, rpc_url=anvil2.url)
    a2.config = a.config  # reuse A's database directory; only the rpc differs
    a2.db = a.db
    a2.dir = a.dir
    a2.config.write_text(a.config.read_text().replace(anvil1.url, anvil2.url)
                         .replace("127.0.0.1:31705", "127.0.0.1:31706"))
    a2.start()
    t_start = time.time()
    rc = None
    for _ in range(120):
        if a2.proc.poll() is not None:
            rc = a2.proc.returncode
            break
        time.sleep(0.5)
    took = time.time() - t_start
    if rc is None:
        a2.stop(signal.SIGKILL)
        h.log(f"restart did NOT exit within {took:.0f}s (head={h.block_number(anvil2.url)})")
    else:
        h.log(f"restart against the behind node EXITED after {took:.1f}s, status {rc}")
    print("=== run 2 (restart against the behind node) log tail ===")
    for ln in h.summarize(a2):
        print("   ", ln)
finally:
    for obj in (a,):
        try:
            if obj and obj.proc and obj.proc.poll() is None:
                obj.stop(signal.SIGKILL)
        except Exception:
            pass
    try:
        anvil1.stop()
    except Exception:
        pass
    if anvil2:
        try:
            anvil2.stop()
        except Exception:
            pass
