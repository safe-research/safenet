#!/usr/bin/env python3
"""F3-SEN-003: a committed sentinel whose own reveal misses the window drops the request at
revealDeadline+1 and never claims, and the RevealMissed recovery in handle_oracle_result never
runs.

Route (distinct from the restart recipe and from F3-SEN-004's warp path): the committed sentinel
B is paused with SIGSTOP across its reveal window, an outage with NO process restart, so there is
no snapshot rollback or warp and B's own commit stays counted. When B is resumed after the reveal
deadline its queued Reveal is already expired, so it never reveals onchain. Meanwhile honest
sentinel A reveals and finalizes, slashing B's governed slashAmount (0.40 USDC.e-equivalent) and
leaving the rest of B's 800 bond claimable by B alone. B then drops the request at
NewBlock(revealDeadline+1) before the OracleResult log of that block, so no Claim is ever queued.
Fee token is MyToken so the Direct-mode approve succeeds (F3-SEN-001 is a separate finding).
"""
import signal
import sys
import time

sys.path.insert(0, "/home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3/scratch/QA3-2B")
import h  # noqa: E402

BINARY = sys.argv[1] if len(sys.argv) > 1 else f"{h.SCRATCH}/bin/sentinel-361aaf3"
LABEL = sys.argv[2] if len(sys.argv) > 2 else "repro"
TRACE = "trace,hyper=info,reqwest=info,alloy_transport=info,alloy_rpc_client=info"

run_dir = f"{h.SCRATCH}/runs/sen003-{LABEL}-{int(time.time())}"
anvil = h.Anvil(port=31700)
a = bsn = None
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
    bsn = h.Sentinel("B", b_pk, dep, 31704, 31706, BINARY, run_dir, log_filter=TRACE)
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

    # Wait until B's own commit has landed onchain, then pause B (outage, no restart).
    for _ in range(60):
        if dep.get_commitment(rid, b_addr)["vote"] == "PENDING" and dep.get_request(rid)["committed"] >= 2:
            break
        time.sleep(0.5)
    h.log(f"both committed: {dep.get_request(rid)}; B commitment {dep.get_commitment(rid, b_addr)}")
    bsn.send_signal(signal.SIGSTOP)
    h.log("B paused (SIGSTOP) for the whole reveal window")

    # A reveals and, after the reveal deadline, finalizes.
    miner.wait_block(req["revealDeadline"] + 3)
    time.sleep(2)
    h.log(f"after A finalizes: {dep.get_request(rid)}")

    # Resume B well past the reveal deadline; its replayed Reveal is already expired.
    bsn.send_signal(signal.SIGCONT)
    h.log("B resumed (SIGCONT) past the reveal deadline")
    miner.wait_block(h.block_number() + 8)
    time.sleep(4)

    final = dep.get_request(rid)
    h.log(f"FINAL request: {final}")
    for who, name in [(a_addr, "A"), (b_addr, "B")]:
        h.log(f"commitment[{name}] = {dep.get_commitment(rid, who)}")
    h.log(f"A balance change covers fee+bond; B balance = {dep.balance(b_addr)}")
    h.log("--- B transactions (expect commit only; NO reveal, NO claim) ---")
    b_txs = dep.txs_from(b_addr, p, h.block_number())
    for t in b_txs:
        h.log(f"  B: {t}")
    h.log("--- B log: reveal expiry and the untracked oracle result ---")
    for ln in bsn.lines():
        f = ln.get("fields", {})
        msg = str(f.get("message", ""))
        if any(k in msg for k in ("expir", "untracked", "Reveal", "reveal", "drop", "oracle result",
                                  "RevealMissed", "reveal_missed", "finalize")):
            print("    B|", ln.get("level"), msg, {k: v for k, v in f.items() if k != "message"})
    h.log("--- B metrics requests_resolved_total ---")
    for ln in bsn.metric("safenet_sentinel_requests_resolved_total"):
        print("    B metric:", ln)
    print("=== verdict ===")
    bc = dep.get_commitment(rid, b_addr)
    print("request_resolved_without_B =", final["state"] == "RESOLVED_APPROVED")
    print("B_vote_still_PENDING =", bc["vote"] == "PENDING")
    print("B_never_claimed =", bc["claimed"] is False)
    print("B_sent_no_reveal =", not any(t["fn"] == "reveal" for t in b_txs))
    print("B_sent_no_claim =", not any(t["fn"] == "claim" for t in b_txs))
    rm = bsn.metric("safenet_sentinel_requests_resolved_total")
    print("B_no_reveal_missed_metric =",
          not any("reveal_missed" in ln and ln.strip().split()[-1] not in ("0", "0.0") for ln in rm))
finally:
    for obj in (a, bsn):
        try:
            if obj and obj.proc:
                if obj.proc.poll() is None:
                    try:
                        obj.send_signal(signal.SIGCONT)
                    except Exception:
                        pass
                    obj.stop(signal.SIGKILL)
        except Exception:
            pass
    anvil.stop()
