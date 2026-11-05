#!/usr/bin/env python3
"""CA3 (2026-10-09): F3-SEN-002 variant (a), honest sentinels only (PC3's settling run).

Two honest registered sentinels, real `sentinel` binaries, real Consensus + SentinelOracle with the
Aegis oracle parameters (fee 0.40, bond 800, slash 0.40, commit 6, reveal 6), 5 s blocks mined by
the harness. Engine A answers at once; engine S answers SLOW_S seconds after it receives the request
(just over one Gnosis block). No attacker. Recorded: whether S logged "ignoring unexpected commitment"
for A's commit, the order of the two reveals, and whether S claimed its bond. MyToken is the fee
token so the Direct-mode approve fits (F3-SEN-001 is a separate finding).
Usage: python3 -I sen002a.py <sentinel-binary> <label> [slow_seconds]
"""
import signal
import sys
import threading
import time

sys.path.insert(0, "/home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3/scratch/CA3/qa2b")
import h  # noqa: E402

BINARY = sys.argv[1]
LABEL = sys.argv[2] if len(sys.argv) > 2 else "honest"
SLOW_S = float(sys.argv[3]) if len(sys.argv) > 3 else 7.0

run_dir = f"{h.SCRATCH}/runs/sen002a-{LABEL}-{int(time.time())}"
anvil = h.Anvil(port=31500)
a = s = None
try:
    # Optional 4th argument "swap": give the fast role to the account with the lower address,
    # to test whether the in-block order of the two reveals follows the sender address.
    fast_i, slow_i = (2, 1) if (len(sys.argv) > 4 and sys.argv[4] == "swap") else (1, 2)
    a_addr, a_pk = h.ACCT[fast_i]
    s_addr, s_pk = h.ACCT[slow_i]
    h.log(f"fast sentinel A = {a_addr}, slow sentinel S = {s_addr}")
    dep = h.Deployment([(a_addr, a_pk), (s_addr, s_pk)])
    h.rpc("evm_setAutomine", [False])
    miner = h.Miner()
    miner.start()
    eng_a = h.Engine(31503, "A")
    eng_s = h.Engine(31504, "S")
    a = h.Sentinel("A", a_pk, dep, 31503, 31505, BINARY, run_dir)
    s = h.Sentinel("S", s_pk, dep, 31504, 31506, BINARY, run_dir)
    a.start()
    s.start()
    a.wait_log("starting sentinel service", 30)
    s.wait_log("starting sentinel service", 30)
    time.sleep(6)

    # S's engine holds each answer for SLOW_S seconds after it receives the request.
    eng_s.hold.set()

    def release_s():
        while not eng_s.requests:
            time.sleep(0.05)
        time.sleep(SLOW_S)
        eng_s.hold.clear()
        h.log(f"engine S released {SLOW_S}s after its request")

    threading.Thread(target=release_s, daemon=True).start()
    rid = dep.request_id(0)
    dep.propose(wait=False)
    miner.wait_block(h.block_number() + 1)
    p = h.block_number()
    h.log(f"proposal mined in block {p}, request id {rid}")
    req = dep.get_request(rid)
    miner.wait_block(req["revealDeadline"] + 20)
    time.sleep(3)
    final = dep.get_request(rid)
    h.log(f"commitDeadline={req['commitDeadline']} revealDeadline={req['revealDeadline']}")
    h.log(f"FINAL onchain request: {final}")
    ca = dep.get_commitment(rid, a_addr)
    cs = dep.get_commitment(rid, s_addr)
    h.log(f"commitment[A] = {ca}")
    h.log(f"commitment[S] = {cs}")
    for who, name in [(a_addr, "A"), (s_addr, "S")]:
        for t in dep.txs_from(who, p, h.block_number()):
            h.log(f"  {name}: {t}")
    events = dep.oracle_events(p, h.block_number())
    for e in events:
        h.log(f"  event {e}")
    reveals = [e for e in events if e[2] == "Revealed"]
    first = reveals[0][3].lower() if reveals else ""
    s_ignored = bool(s.find("ignoring unexpected commitment"))
    s_claim_tx = any(t["fn"] == "claim" for t in dep.txs_from(s_addr, p, h.block_number()))
    print("=== verdict ===")
    print("S_ignored_A_commit =", s_ignored)
    print("first_reveal =", "A" if first == a_addr.lower() else ("S" if first == s_addr.lower() else first))
    print("request_state =", final["state"])
    print("A_claimed =", ca["claimed"])
    print("S_claimed =", cs["claimed"])
    print("S_sent_claim_tx =", s_claim_tx)
finally:
    for obj in (a, s):
        try:
            if obj and obj.proc and obj.proc.poll() is None:
                obj.stop(signal.SIGKILL)
        except Exception:
            pass
    anvil.stop()
