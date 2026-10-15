#!/usr/bin/env python3
"""F3-SEN-004: after a restart with a few blocks of downtime, the warp replay re-ignores the
sentinel's own Committed, so a bonded request is dropped without a reveal.

Route (restart + real warp, the path F3-SEN-003 deliberately avoids): sentinel A commits, is
cleanly stopped for ~7 blocks (a container restart / upgrade), and restarts. Its block watcher
rolls back to the oldest retained snapshot and replays the missed range as a warp page that
includes the proposal, the NewRequest and A's OWN Committed in a single update, applied before
the engine check resumes. A's own commit is logged "ignoring unexpected commitment", the recorded
verdict resumes with self_committed=false, A re-sends a Commit that reverts AlreadyCommitted, and
A drops the request at commitDeadline+1 with no reveal. Sentinel B stays up and reveals, so the
request resolves, A is slashed the governed amount and A's 799.60 remainder is stranded unclaimed.
"""
import signal
import sys
import time

sys.path.insert(0, "/home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3/scratch/QA3-2B")
import h  # noqa: E402

BINARY = sys.argv[1] if len(sys.argv) > 1 else f"{h.SCRATCH}/bin/sentinel-361aaf3"
LABEL = sys.argv[2] if len(sys.argv) > 2 else "repro"

run_dir = f"{h.SCRATCH}/runs/sen004-{LABEL}-{int(time.time())}"
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
    h.log(f"proposal in block {p}; commitDeadline={req['commitDeadline']} revealDeadline={req['revealDeadline']}")
    for _ in range(60):
        if dep.get_commitment(rid, a_addr)["vote"] == "PENDING" and dep.get_request(rid)["committed"] >= 2:
            break
        time.sleep(0.5)
    h.log(f"both committed: {dep.get_request(rid)}; A commitment {dep.get_commitment(rid, a_addr)}")

    # Clean restart of A with downtime (B keeps running and will reveal).
    a.stop(signal.SIGTERM)
    h.log(f"A stopped (clean), exit {a.proc.returncode}; downtime begins at block {h.block_number()}")
    miner.wait_block(p + 8)  # ~7 blocks of downtime so the warp page spans P..P+commit
    a2 = h.Sentinel("A", a_pk, dep, 31703, 31707, BINARY, run_dir)
    a2.dir = a.dir
    a2.db = a.db
    a2.config = a.config
    a2.config.write_text(a.config.read_text().replace("127.0.0.1:31705", "127.0.0.1:31707"))
    a2.runs = 1
    a2.start()
    h.log(f"A restarted at head {h.block_number()} after downtime")

    miner.wait_block(req["revealDeadline"] + 20)
    time.sleep(4)
    final = dep.get_request(rid)
    h.log(f"FINAL request: {final}")
    for who, name in [(a_addr, "A"), (b_addr, "B")]:
        h.log(f"commitment[{name}] = {dep.get_commitment(rid, who)}")
    h.log(f"A balance = {dep.balance(a_addr)} (100*bond - stranded)")
    a_txs = dep.txs_from(a_addr, p, h.block_number())
    h.log("--- A transactions across both runs ---")
    for t in a_txs:
        h.log(f"  A: {t}")
    h.log("--- A restart log: warp + own commit ignored ---")
    for ln in a2.lines():
        f = ln.get("fields", {})
        msg = str(f.get("message", ""))
        if any(k in msg for k in ("initializing block watcher", "ignoring unexpected commitment",
                                  "reusing recorded", "ignoring oracle result", "unexpected")):
            print("    A2|", ln.get("level"), msg, {k: v for k, v in f.items() if k != "message"})
    print("=== verdict ===")
    ac = dep.get_commitment(rid, a_addr)
    print("request_resolved_without_A =", final["state"] == "RESOLVED_APPROVED")
    print("A_vote_still_PENDING =", ac["vote"] == "PENDING")
    print("A_never_claimed =", ac["claimed"] is False)
    print("A_no_successful_reveal =", not any(t["fn"] == "reveal" and t["status"] == 1 for t in a_txs))
    print("A_no_claim_tx =", not any(t["fn"] == "claim" for t in a_txs))
    print("A_own_commit_ignored_on_replay =",
          any(str(ln.get("fields", {}).get("message", "")) == "ignoring unexpected commitment"
              and str(ln.get("fields", {}).get("state")) == "waiting_for_engine_check"
              for ln in a2.lines()))
finally:
    for obj in (a, a2, bsn):
        try:
            if obj and obj.proc and obj.proc.poll() is None:
                obj.stop(signal.SIGKILL)
        except Exception:
            pass
    anvil.stop()
