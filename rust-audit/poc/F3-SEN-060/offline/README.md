# F3-SEN-060 contract half, offline

The fork PoC that LT3 ran first (taken out of this folder, kept outside the repository) called `vm.createSelectFork(vm.rpcUrl("gnosis"), 48635154)` with `rpc_endpoints.gnosis = "https://rpc.gnosischain.com"`, so it contacted a public RPC. This folder re-derives the same result with no network: no fork, no `rpc_endpoints`, `offline = true`, and every command ran under `integ`, a network namespace with only a loopback device (`run-offline.txt` section 0).

## Result

Reproduced. The sponsor of a FROZEN request is blacklisted on USDC.e:

- `resolveDispute`, `markOutOfScope` and, at `arbitrationDeadline + 1` (block 49,170,842), each sentinel's `timeoutArbitration` revert with `Blacklistable: account is blacklisted`. The real FiatTokenV2_2 code raises it on the fee refund to the sponsor.
- Each sentinel's `claim` in the same block reverts `RequestNotResolved()`.
- The request stays FROZEN. The oracle keeps 2,400,400,000 raw units (2,400.40 USDC.e: three 800 USDC.e bonds plus the 0.40 fee), and each sentinel holds 200 of its 1,000.
- After `unBlacklist`, a third party's `timeoutArbitration` succeeds (`ArbitrationTimedOut`, 0.40 back to the sponsor), every `claim` returns the full 800, and the oracle ends at 0.
- Control without the blacklist: at the same block the first sentinel's `timeoutArbitration` succeeds, the other two revert `RequestNotFrozen()`, and every `claim` succeeds.

The fork run (its output was taken out of this folder with the fork PoC) gave the same numbers: 2,400,400,000 locked, `DisputeTriggered` deadline 49,170,841, and the same gas for each of the 29 state-changing oracle and token calls before the blacklist and for each of its calls after it.

## What runs

- `SentinelOracle` from the audit commit `361aaf3`, built with the production settings (solc 0.8.30, optimizer 1,000,000 runs, via IR, prague) and deployed with every oracle value in `contracts/.env.aegis.sample`. Apart from the CBOR metadata, its runtime bytecode equals the `contracts/build/out` artifact (section 4).
- USDC.e runtime code and storage from the Foundry RPC cache `~/.foundry/cache/rpc/xdai/48635154` (Gnosis block 48,635,154): FiatTokenProxy `0x2a22f9c3b484c3629090FeED35F17Ff8F88f76F0` (1,852 bytes), FiatTokenV2_2 `0x107CF7fb73EA48D1D200989b156Ce1894d7AfEC7` (23,464 bytes) and the 14 cached proxy slots, installed with `vm.etch` and `vm.store`. `scripts/extract_usdc.py` writes `src/UsdcCode.sol` from the cache; the test checks the code hashes against it. Every proxy slot the tests read is a cached slot (section 5).
- Cross-check from the 2026-10-07 Aegis env review, read through two RPCs at block 48,635,100: the same proxy code hash, implementation, admin, blacklister and `paused = false`, and a 23,464-byte implementation.
- The test starts at the cache block, so the timeline matches the fork run: sentinels active at 48,686,994, frozen at 48,687,001.

## Differences from the fork PoC

- A `consensus` address stands in for the oracle's PROPOSER and calls `postRequest(id, sponsor, "")`. `Consensus.proposeTransaction` makes the same call with the proposer as the sponsor (`contracts/src/Consensus.sol:266`), and the oracle only checks the caller.
- The sentinels send `timeoutArbitration` and then `claim` from their own addresses, in the order the dispute-timeout arm queues them. The fork PoC sent one `timeoutArbitration` from the test contract.

## Privileged steps

- Blacklisting the sponsor is the precondition of the finding (a Circle compliance action), not a shortcut around the defect, so taking the blacklister role is allowed here. The test calls the token's own `blacklist` and `unBlacklist` as its blacklister `0x458cD345B4C05e8DF39d0A07220feb4Ec19F5e6f` (cached slot 2). A `vm.store` of the blacklist bit would be acceptable for the same reason.
- `vm.etch` and `vm.store` only install the cached chain code and storage.
- `deal` funds the sponsor (10 USDC.e) and each sentinel (1,000). Funding is not part of the route.
- `vm.prank` as GOVERNANCE registers the sentinels (normal setup). `vm.prank` as the arbitrator shows that it cannot rule or decline either.

## Mutation checks

In `mutations.txt`. With the sponsor refund removed from `timeoutArbitration`, the main test fails at the sentinel's `timeoutArbitration`. With `claim` accepting FROZEN, it fails at the sentinel's `claim`. Both mutants were scratch copies.

## Re-run

From this folder in a checkout of `361aaf3` with `contracts/lib` populated and solc 0.8.30 installed: `forge test -vv --offline`. No cache file is needed; `src/UsdcCode.sol` carries the code and storage. To regenerate it: `python3 -I scripts/extract_usdc.py <cache file for block 48635154> src/UsdcCode.sol`.
