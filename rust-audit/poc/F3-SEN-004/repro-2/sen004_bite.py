#!/usr/bin/env python3
"""F3-SEN-004 bite check with controlled timing.

The warp still replays A's own commit, but the chain is paused at commitDeadline before A restarts,
so the engine check resumes (reaching commit_vote) before NewBlock(commitDeadline+1) is observed,
the ordering the finding's row 7 models. With remediation option 1 the replayed own commit is now
tallied (self_committed carried into CollectingCommitments), so A reveals at commitDeadline+1 and
claims instead of dropping. Pass the pristine binary to confirm it still drops under this timing.
"""
import signal
import sys
import time

sys.path.insert(0, "/home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3/scratch/QA3-2B")
import h  # noqa: E402

BINARY = sys.argv[1] if len(sys.argv) > 1 else f"{h.SCRATCH}/bin/sentinel-sen004fix"
LABEL = sys.argv[2] if len(sys.argv) > 2 else "bite"

run_dir = f"{h.SCRATCH}/runs/sen004b-{LABEL}-{int(time.time())}"
anvil = h.Anvil(port=31700)
a = a2 = bsn = None
try:
    a_addr, a_pk = h.ACCT[1]
    b_addr, b_pk = h.ACCT[2]
    dep = h.Deployment([(a_addr, a_pk), (b_addr, b_pk)])
    h.rpc("evm_setAutomine", [False])
    miner = h.Miner()
    miner.start()
    eng_a = h.Engine(31703, "A")
    eng_b = h.Engine(31704, "B")
    a = h.Sentinel("A", a_pk, dep, 31703, 31705, BINARY, run_dir)
    bsn = h.Sentinel("B", b_pk, dep, 31704, 31706, BINARY, run_dir)
    a.start()
    bsn.start()
    a.wait_log("starting sentinel service", 30)
    bsn.wait_log("starting sentinel service", 30)
    time.sleep(6)

    rid = dep.request_id(0)
    dep.propose(wait=False)
    miner.wait_block(h.block_number() + 1)
    p = h.block_number()
    req = dep.get_request(rid)
    cd, rd = req["commitDeadline"], req["revealDeadline"]
    h.log(f"proposal in block {p}; commitDeadline={cd} revealDeadline={rd}")
    for _ in range(60):
        if dep.get_commitment(rid, a_addr)["vote"] == "PENDING" and dep.get_request(rid)["committed"] >= 2:
            break
        time.sleep(0.5)
    h.log(f"both committed: {dep.get_request(rid)}")

    a.stop(signal.SIGTERM)
    h.log(f"A stopped (clean) at block {h.block_number()}")
    # Advance to exactly commitDeadline, then pause the chain so the engine resume can complete.
    miner.wait_block(cd)
    miner.running.clear()
    h.log(f"miner paused at head {h.block_number()} (== commitDeadline {cd})")
    n_eng = len(eng_a.requests)
    a2 = h.Sentinel("A", a_pk, dep, 31703, 31707, BINARY, run_dir)
    a2.dir, a2.db, a2.config = a.dir, a.db, a.config
    a2.config.write_text(a.config.read_text().replace("127.0.0.1:31705", "127.0.0.1:31707"))
    a2.runs = 1
    a2.start()
    h.log("A restarted with the chain paused at commitDeadline")
    # Wait for A's engine check to resume (fresh call, since the pruned verdict was dropped).
    for _ in range(40):
        if len(eng_a.requests) > n_eng:
            break
        time.sleep(0.5)
    time.sleep(4)  # let the resume reach commit_vote
    h.log(f"engine calls on restart: {len(eng_a.requests) - n_eng}; resuming the chain")
    miner.running.set()

    miner.wait_block(rd + 20)
    time.sleep(4)
    final = dep.get_request(rid)
    h.log(f"FINAL request: {final}")
    for who, name in [(a_addr, "A"), (b_addr, "B")]:
        h.log(f"commitment[{name}] = {dep.get_commitment(rid, who)}")
    h.log(f"A balance = {dep.balance(a_addr)}")
    a_txs = dep.txs_from(a_addr, p, h.block_number())
    for t in a_txs:
        h.log(f"  A: {t}")
    print("=== verdict ===")
    ac = dep.get_commitment(rid, a_addr)
    print("A_revealed_onchain =", any(t["fn"] == "reveal" and t["status"] == 1 for t in a_txs))
    print("A_claimed =", ac["claimed"])
    print("A_bond_recovered =", dep.balance(a_addr) > 79200000000)
finally:
    for obj in (a, a2, bsn):
        try:
            if obj and obj.proc and obj.proc.poll() is None:
                obj.stop(signal.SIGKILL)
        except Exception:
            pass
    anvil.stop()
