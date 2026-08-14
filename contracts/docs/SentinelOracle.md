# SentinelOracle Flows

`SentinelOracle` (`contracts/src/SentinelOracle.sol`) resolves a request through a commit/reveal vote among registered sentinels, with the arbitrator as a fallback for split (`FROZEN`) outcomes. This document walks through concrete, worked examples of each flow as sequence diagrams, annotating only the state that matters for understanding what's happening at each step.

Requests move through `SentinelOracleRequest.State`:

```
PENDING -> RESOLVED_APPROVED   (unanimous approve)
PENDING -> RESOLVED_DENIED     (unanimous deny)
PENDING -> FROZEN -> RESOLVED_APPROVED | RESOLVED_DENIED   (split vote, arbitrator rules)
PENDING -> FROZEN -> TIMED_OUT                             (split vote, arbitrator never rules)
PENDING -> TIMED_OUT           (nobody reveals)
```

Each sentinel's individual ballot is tracked separately as a `SentinelOracleCommitment.Commitment` (state machine `NONE -> PENDING -> APPROVED | DENIED`), keyed by `(requestId, sentinel)`.

---

## Flow: Unanimous Approval

Two sentinels — Alice and Bob — both commit and reveal `approve`. Nobody dissents, so `finalize` resolves the request directly to `RESOLVED_APPROVED` with no arbitration step, and every sentinel recovers their full bond plus an equal share of the fee.

**Setup for this example:**

| Parameter | Value |
|---|---|
| `fee` (current governed fee) | 0.40 USDC |
| `bondMultiplier` | 2000 |
| `bondTarget` (= fee × bondMultiplier) | 800 USDC per sentinel |
| `slashAmount` (= fee × slashingMultiplier) | governed separately — unused in this flow, nobody is slashed |
| `daoFeeShare` | 10% (`10_000` / `FEE_SHARE_DENOMINATOR`) |
| sentinels | Alice, Bob (both active) |

Solid arrows (`→`) are contract calls. Dashed arrows (`⇢`) show the resulting ERC20 balance change, directly between the accounts whose balances actually move.

```mermaid
sequenceDiagram
    actor Sponsor
    actor Proposer as PROPOSER (Consensus)
    actor Alice
    actor Bob
    participant Oracle as SentinelOracle
    participant Token as FEE_TOKEN
    actor DAO as protocolFundsReceiver

    Note over Oracle: request[R1] = none

    Proposer->>Oracle: postRequest(R1, Sponsor, ...)
    Oracle->>Token: transferFrom(Sponsor, Oracle, 0.40)
    Sponsor-->>Oracle: 0.40 FEE_TOKEN
    Note over Oracle: request[R1] = { state: PENDING, sponsor: Sponsor,<br/>fee: 0.40, bondTarget: 800, daoFeeShare: 10_000,<br/>committedCount: 0, revealedCount: 0,<br/>approveCount: 0, denyCount: 0 }

    rect rgba(200,220,255,0.3)
        Note over Alice,Token: Commit window
        Alice->>Oracle: commit(R1, hash(approve=true, saltA, reasonA))
        Oracle->>Token: transferFrom(Alice, Oracle, 800)
        Alice-->>Oracle: 800 FEE_TOKEN
        Note over Oracle: commitment[R1][Alice] = { vote: PENDING, bondAmount: 800 }<br/>request[R1].committedCount = 1

        Bob->>Oracle: commit(R1, hash(approve=true, saltB, reasonB))
        Oracle->>Token: transferFrom(Bob, Oracle, 800)
        Bob-->>Oracle: 800 FEE_TOKEN
        Note over Oracle: commitment[R1][Bob] = { vote: PENDING, bondAmount: 800 }<br/>request[R1].committedCount = 2
    end

    Note over Oracle: block.number passes commitDeadline -- reveal window opens

    rect rgba(200,255,200,0.3)
        Note over Alice,Token: Reveal window
        Alice->>Oracle: reveal(R1, approve=true, saltA, reasonA)
        Note over Oracle: commitment[R1][Alice].vote = APPROVED<br/>request[R1] = { revealedCount: 1, approveCount: 1 }

        Bob->>Oracle: reveal(R1, approve=true, saltB, reasonB)
        Note over Oracle: commitment[R1][Bob].vote = APPROVED<br/>request[R1] = { revealedCount: 2, approveCount: 2 }
    end

    Note over Oracle: revealedCount (2) == committedCount (2) -> everyoneRevealed,<br/>so finalize() doesn't need to wait for revealDeadline

    Alice->>Oracle: finalize(R1)
    Note over Oracle: approveCount > 0, denyCount == 0 -> newState = RESOLVED_APPROVED<br/>(no FROZEN step, no unrevealed bond to slash)<br/>daoCut = 0.40 * 10_000 / 100_000 = 0.04<br/>request[R1].fee = 0.40 - 0.04 = 0.36<br/>request[R1].state = RESOLVED_APPROVED
    Oracle->>Token: transfer(protocolFundsReceiver, 0.04)
    Oracle-->>DAO: 0.04 FEE_TOKEN
    Oracle--)Sponsor: emit OracleResult(R1, Sponsor, "", approved=true)

    rect rgba(255,240,200,0.3)
        Note over Alice,Token: Each sentinel claims independently
        Alice->>Oracle: claim(R1)
        Note over Oracle: feeReward = request[R1].fee / approveCount = 0.36 / 2 = 0.18<br/>slash = 0 (denyCount == 0, no losing side)<br/>bondReturn = 800 - 0 = 800<br/>commitment[R1][Alice].claimed = true
        Oracle->>Token: transfer(Alice, 800.18)
        Oracle-->>Alice: 800.18 FEE_TOKEN

        Bob->>Oracle: claim(R1)
        Note over Oracle: feeReward = 0.36 / 2 = 0.18, slash = 0, bondReturn = 800<br/>commitment[R1][Bob].claimed = true
        Oracle->>Token: transfer(Bob, 800.18)
        Oracle-->>Bob: 800.18 FEE_TOKEN
    end
```

**Outcome:** Sponsor paid 0.40 USDC for the request; the protocol kept its 0.04 USDC DAO cut; Alice and Bob each staked 800 USDC and got back 800.18 (800 bond + 0.18 fee share) — a net profit of 0.18 USDC apiece for agreeing on the correct answer. Nobody was slashed because no dissenting side was ever established (`slashAmountFor` only slashes a revealed vote when it's on the *losing* side of a resolved dispute).
