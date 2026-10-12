# OFF3 offline re-derivation of the F3-SEN-060 contract half (run 3)

The contract half of F3-SEN-060 came from a fork test (`poc/F3-SEN-060/contracts/test/AegisParams.t.sol`) that called `vm.createSelectFork(vm.rpcUrl("gnosis"), 48635154)` against `https://rpc.gnosischain.com`. Brief: get the same result with no network at all. Audit commit `361aaf3`, worktree `wt/lt3`, env `env/lt3.sh`, every forge command under `integ`. No Anvil started.

## Log

| Step | Command (all under `integ`, in `wt/lt3/rust-audit/poc/F3-SEN-060/offline`) | Result |
| --- | --- | --- |
| namespace | `integ bash -c 'ip -o link show; ip route show'` | only `lo`, no route |
| extract | `integ python3 -I scripts/extract_usdc.py ~/.foundry/cache/rpc/xdai/48635154 src/UsdcCode.sol` | block 48,635,154; FiatTokenProxy 1,852 bytes, FiatTokenV2_2 23,464 bytes, 14 proxy slots; cache sha256 `6842f62b…`; a second run gives the same file (sha256 `c1d7d701…`) |
| build | `integ forge build --force` | 37 files, solc 0.8.30, optimizer 1,000,000 runs, via IR |
| test | `integ forge test -vv` | 3 passed: main, control, etched-code check |
| trace | `integ forge test --match-test test_blacklistedSponsorLocksFrozenBondsUntilUnblacklisted -vvvv` | `Blacklistable: account is blacklisted` raised inside FiatTokenV2_2 for `resolveDispute`, `markOutOfScope` and each sentinel's `timeoutArbitration` at block 49,170,842; each `claim` reverts `RequestNotResolved()`; 2,400,400,000 stays in the oracle; after `unBlacklist` the timeout succeeds, each claim returns 800,000,000, the oracle ends at 0 |
| production bytecode | `integ python3 -I cmp_bytecode.py out/... ../../../../contracts/build/out/...` | runtime identical to the `forge build` artifact without the CBOR metadata (16,759 bytes) |
| slot coverage | scratch copy with `vm.record()` in `setUp`, reads from `vm.accesses(PROXY)` | 214 reads (main) and 158 (control), 0 outside the cached slots; the cached receiver balance is read 0 times |
| gas parity | Python over this trace and `poc/F3-SEN-060/contract-blacklist-offline.txt` | 29 of 29 state-changing oracle and token calls before the blacklist use the same gas as the fork run; its calls after the blacklist appear in order with the same gas |
| M1 | scratch mutant: no refund push in `timeoutArbitration` | main test fails at sentinel0's `timeoutArbitration`; control fails (sponsor 9.6, not 10) |
| M2 | scratch mutant: `requireResolved` accepts FROZEN | main test fails at sentinel0's `claim`; control passes |
| format | `forge fmt --check`; Prettier 3.9.6 `--check`; the style check | clean |

## Verdict

Reproduced with no network. With the sponsor of a FROZEN request blacklisted on USDC.e, `timeoutArbitration` reverts with the token's `Blacklistable` error and `claim` reverts `RequestNotResolved()`, the request stays FROZEN with all three 800 USDC.e bonds and the 0.40 fee in the oracle, and after `unBlacklist` the timeout and every claim succeed. Same numbers and same gas as LT3's fork run. Output: `poc/F3-SEN-060/offline/` (`README.md`, `run-offline.txt`, `mutations.txt`).

The blacklisting is done by calling the token's own `blacklist` as its real blacklister (`0x458cD345…`, cached slot 2). That role is the precondition of the finding, not a shortcut around the defect. `vm.etch` and `vm.store` only install the cached chain state; `deal` only funds accounts. A `consensus` address stands in for the PROPOSER (`contracts/src/Consensus.sol:266` makes the same `postRequest` call).

## Notes

- Provenance cross-check without new network access: a local review of the Aegis configuration (2026-10-07) read the token through two RPCs at block 48,635,100 (its `onchain/g_publicnode.txt` and `g_gnosischain.txt`): same proxy code hash `0xad30d819…`, implementation, admin, blacklister, `paused = false`, and a 23,464-byte implementation. The cache's `code_hash` values are computed by Foundry from the fetched bytes, so the test's hash check confirms the trimming, not provenance by itself.
- Wording in the finding, for the head: Claim paragraph 2 and Basis row 3 say each exit pushes the refund "before changing state". The code writes `TIMED_OUT` first (`SentinelOracleRequests.sol:226`, `:237`) and pushes after it (`SentinelOracle.sol:347-348`); the failed push reverts both, so the effect is the same.
- Residue: the run folder `wt/lt3/rust-audit/poc/F3-SEN-060/` (untracked in the worktree, 2.3 MB with `out/` and `cache/`) is a cleanup candidate. Tracked files in `wt/lt3` are untouched. The Foundry cache file is unchanged (same sha256 and mtime).
- Not covered: USDC.e `pause()` (PC3's optional second route), the route through a real Consensus, and the sentinel half (LT3 and CA3 ran it).
