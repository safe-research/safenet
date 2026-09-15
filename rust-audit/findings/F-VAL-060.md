# F-VAL-060 Coordinator and Consensus events are dispatched without checking the emitting contract address

| Field | Value |
| --- | --- |
| Status | QA-done |
| Crate and module | validator, state/mod.rs + service/mod.rs + main.rs |
| Location | crates/validator/src/state/mod.rs:415-462 (related: crates/validator/src/service/mod.rs:102-115, crates/validator/src/main.rs:56-57, crates/core/src/index/events.rs:403-409, crates/validator/src/state/keygen.rs:713-731, crates/validator/src/state/keygen.rs:1195-1224) |
| Severity | High / Medium |
| Certainty | 50% (Critic C-VAL-B; QA may raise) |
| Assumptions involved | A1, A2, A4 |
| Tags | input-validation, crypto, dos |

## Claim

`Transition::apply_transition` routes every `Coordinator::*` and `Consensus::*` event to its handler purely on the decoded `topic0`, never on the address that emitted the log. `log.address` is passed to exactly one handler — `handle_oracle_result` — so the _only_ event bound to its emitter is the one coming from the untrusted, operator-extensible allow-list, while every protocol-critical event from the two trusted contracts is accepted from **any** watched address.

The watched address set is `[consensus, coordinator] ++ config.validator.oracles`, and the log filter is the cross product of every watched address with every watched topic. Any contract whose address the operator adds to `oracles` therefore has full write access to the validator's state machine, not merely the "approve / deny a transaction" authority the config field documents.

The gap is structural, not a forgotten `if`: `state::Transition` is constructed without the coordinator address at all (`service/mod.rs:104-109` gives it `account`, `genesis`, `consensus` (an EIP-712 `ConsensusDomain`, not an address usable for comparison) and `config`), while the coordinator address is handed only to the _action encoder_. A handler could not perform the check today even if it wanted to.

Reachable consequences of one injectable address, all of which need only a log with the right `topic0` and body:

- `KeyGenComplained{gid, plaintiff, accused}` — `handle_key_gen_complained` counts complaints per accused with no address, plaintiff-membership or per-plaintiff-uniqueness check, and at `complaint.total >= threshold` restarts the DKG excluding the accused (`keygen.rs:713-731`). During the **genesis** ceremony `restart_key_gen_excluding` cannot restart and falls through to `rollover_failure`, which sets `RolloverState::Halted` — documented in `state/mod.rs:94-98` as unrecoverable. `threshold` injected logs in a single transaction permanently halt the validator.
- The same handler, when `accused == self.account`, queues `Action::KeyGenComplaintResponse{secret_share}` carrying the plaintext scalar `f_me(plaintiff)` (`keygen.rs:733-745`). The transaction reverts on the real coordinator, but the queue signs and broadcasts without simulating or estimating (`core/src/tx/mod.rs:265` calls `send_raw_transaction` directly), so the plaintext evaluation reaches the RPC and the public mempool. The reveal itself is bounded: `frost::keygen::reveal_secret_share` only answers for a peer already in `peer_packages` (`frost/keygen.rs:420-428`), and the threshold restart caps the count at `threshold - 1` distinct points of a degree-`threshold-1` polynomial.
- `Sign`, `SignRevealedNonces`, `KeyGenComplaintResponded`, `TransactionProposed`, `EpochStaged` are dispatched by the same unguarded `match` arms and drive nonce-sequence consumption, ceremony restarts and session bookkeeping.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1a | Dispatch is on the decoded event only; the emitting address is not consulted for Coordinator or Consensus events | E2 | `crates/validator/src/state/mod.rs:415-419` | `            Message::Event(log) => match log.data {`<br>`                Event::Coordinator(Coordinator::CoordinatorEvents::KeyGen(event)) => {`<br>`                    self.handle_genesis_key_gen(state, &event)`<br>`                }`<br>`                Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenCommitted(event)) => {` |
| 1b | `log.address` is passed to exactly one handler, `handle_oracle_result` | E2 | `crates/validator/src/state/mod.rs:458-462` | `                Event::Oracle(Oracle::OracleEvents::OracleResult(event)) => {`<br>`                    self.handle_oracle_result(state, log.block, log.address, &event)`<br>`                }`<br>`                // The remaining events are wired in as their handlers land.`<br>`                _ => (state, Vec::new),` |
| 2 | The transition is never given the coordinator address, so it could not check one; only the action encoder receives it | E2 | `crates/validator/src/service/mod.rs:102-115` | `        let consensus_address = config.consensus;`<br>`        (`<br>`            state::Transition {`<br>`                account,`<br>`                genesis,`<br>`                consensus,`<br>`                config,`<br>`            },`<br>`            effect::Handler::new(account, secrets),`<br>`            action::Encoder {`<br>`                coordinator,`<br>`                consensus: consensus_address,`<br>`            },`<br>`        )` |
| 3 | Configured oracle contracts join the same watched address set as the two trusted contracts | E2 | `crates/validator/src/main.rs:56-57` | `    let mut watched = vec![consensus, coordinator];`<br>`    watched.extend(config.validator.oracles.iter.copied);` |
| 4 | The log filter is the cross product of every watched address with every watched topic | E2 | `crates/core/src/index/events.rs:402-409` | `    async fn fetch_logs(&self, fetch: Fetch) -> Result<Vec<EventLog<E>>, Error> {`<br>`        let logs = match fetch {`<br>`            Fetch::SingleQuery(blocks) => {`<br>`                let filter = blocks`<br>`                    .into_filter`<br>`                    .address(self.addresses.clone)`<br>`                    .event_signature(self.topics.clone);`<br>`                let logs = self.provider.get_logs(&filter).await?;` |
| 5 | Decoding ignores the address; it is only carried alongside the decoded data | E2 | `crates/core/src/index/events.rs:491-503` | `fn decode_and_sort<E>(logs: &[Log]) -> Result<Vec<EventLog<E>>, Error>`<br>`where`<br>`    E: Events,`<br>`{`<br>`    let mut logs = logs`<br>`        .iter`<br>`        .map(\|log\| {`<br>`            E::decode_log(log.topics, &log.data.data)`<br>`                .and_then(\|data\| {`<br>`                    Some(EventLog {`<br>`                        block: log.block_number?,`<br>`                        index: log.log_index?,`<br>`                        address: log.inner.address,` |
| 6a | Complaints are counted per accused with no address, plaintiff-membership or per-plaintiff-uniqueness check | E2 | `crates/validator/src/state/keygen.rs:714-722` | `        let complaint = complaints.entry(event.accused).or_default;`<br>`        complaint.total += 1;`<br>`        complaint.unresponded += 1;`<br>``<br>`        // If we ever get threshold complaints, the keygen is done. This is`<br>`        // because it would reveal sufficient public information to compute`<br>`        // secret key shares from one or more participants.`<br>`        let (_, threshold) = group.size;`<br>`        if complaint.total >= threshold {` |
| 6b | Reaching `threshold` restarts the ceremony excluding the accused | E2 | `crates/validator/src/state/keygen.rs:728-730` | `            let excluded = group.also_exclude(iter::once(event.accused));`<br>``<br>`            return self.restart_key_gen_excluding(state, next_epoch, excluded, restart_deadline);` |
| 6c | When the accused is this validator, a plaintext scalar is queued for onchain publication | E2 | `crates/validator/src/state/keygen.rs:733-744` | `        let mut commands = Vec::new;`<br>`        if let KeyGenParticipation::Participating(sharing_state) = participation`<br>`            && event.accused == self.account`<br>`        {`<br>`            match frost::keygen::reveal_secret_share(sharing_state, event.plaintiff) {`<br>`                Ok(secret_share) => {`<br>`                    commands.push(Command::Action(Action::KeyGenComplaintResponse {`<br>`                        group_id: group.id,`<br>`                        plaintiff: event.plaintiff,`<br>`                        secret_share,`<br>`                        expires_at: response_expires_at,`<br>`                    }));` |
| 7a | A restart during genesis cannot re-form a participant set | E2 | `crates/validator/src/state/keygen.rs:1204-1210` | `        } else {`<br>`            // In case we need to restart keygen during genesis - halt! The`<br>`            // The genesis keygen is special in that it cannot be restart`<br>`            // since the group ID has special authorization, and any restart`<br>`            // would issue a new and different group ID`<br>`            None`<br>`        };` |
| 7b | and therefore falls through to `rollover_failure` | E2 | `crates/validator/src/state/keygen.rs:1214-1219` | `            None => (`<br>`                State {`<br>`                    rollover: rollover_failure(`<br>`                        next_epoch,`<br>`                        "could not form new participant set to restart keygen",`<br>`                    ),` |
| 7c | which is permanent for genesis | E2 | `crates/validator/src/state/keygen.rs:1434-1438` | `    } else {`<br>`        tracing::error!(`<br>`            %err,`<br>`            "failed to advance genesis key generation, permanently halted"`<br>`        );` |
| 8 | Queued transactions are broadcast with no simulation or gas estimation, so a reverting call still publishes its calldata | E2 | `crates/core/src/tx/mod.rs:264-266` | `        );`<br>`        match self.provider.send_raw_transaction(signed.as_raw).await {`<br>`            Ok(_) => self.storage.record_submission(submission).await?,` |
| 9 | The plaintext reveal is bounded to addresses that are actual DKG peers of the group | E2 | `crates/validator/src/frost/keygen.rs:420-428` | `pub fn reveal_secret_share(sharing_state: &SharingState, peer: Address) -> Result<U256, Error> {`<br>`    let identifier = participants::identifier(peer);`<br>`    sharing_state`<br>`        .peer_packages`<br>`        .get(&identifier)`<br>`        .map(\|package\| marshal::solidity_scalar(&package.signing_share.to_scalar))`<br>`        .ok_or(frost_secp256k1::Error::UnknownIdentifier)`<br>`        .err_unexpected`<br>`}` |
| 10 | `Consensus._COORDINATOR` is immutable, so the address resolved at startup cannot go stale mid-run | E2 | `contracts/src/Consensus.sol:60 and :119-120` | `    constructor(address coordinator, FROSTGroupId.T groupId) {`<br>`        _COORDINATOR = FROSTCoordinator(coordinator);` |

## Trigger

Precondition: at least one address in `config.validator.oracles` belongs to a contract that can be made to emit a log whose `topic0` equals a `FROSTCoordinator` or `Consensus` event selector, with a body that decodes as that event. None of the three reference oracle implementations in this repo can (see _Considered and rejected_), so this requires a third-party, upgradeable, or later-compromised oracle contract.

Given that address, the concrete sequence for the strongest outcome:

1. Wait for the `KeyGen` event of the genesis ceremony (public, indexed by `gid` and `context`).
2. In one transaction, emit `threshold` logs with `topic0 = keccak256("KeyGenComplained(bytes32,address,address,bool)") = 0xfacda0c1a23c91046de84f88c9fb4f3cd4360b4ae1b821b127968fa5d9db5fb9`, `topics[1] = gid`, and bodies `(plaintiff_i, accused = <any honest member>, false)` with `threshold` distinct plaintiffs (or the same one — nothing dedupes).
3. `handle_key_gen_complained` reaches `complaint.total >= threshold` while `rollover` is `CollectingShares`/`CollectingConfirmations` for that `gid`, calls `restart_key_gen_excluding` with `next_epoch = EpochId::Genesis`, gets `None`, and sets `RolloverState::Halted`. The validator never publishes another commitment or share for genesis; recovery needs manual deletion of the snapshot table.

For the plaintext-reveal variant, set `accused` to the target validator's own account and use `threshold - 1` distinct peer plaintiffs; each yields one `keyGenComplaintResponse` broadcast carrying `f_me(plaintiff)` in cleartext calldata.

## Considered and rejected

- **"The core watcher filters by address, so only the coordinator's logs arrive."** It does filter by address, but the address set includes the oracles (`main.rs:56-57`) and the topic set is the union of all three ABIs' selectors (`core/index/events.rs:403-409`, `watcher_events!` at `core/index/events.rs:568-591`); the filter is the cross product, not a per-address topic list.
- **"Topic collisions across the three contracts could explain it away / could be the real bug."** Checked mechanically: I computed `keccak256` of the canonical signature of all 18 event definitions in `bindings.rs` and of all 18 function signatures, expanding structs to tuples and the `FROSTGroupId.T` / `FROSTSignatureId.T` / `SafeId.T` user-defined value types to `bytes32` and `Operation` to `uint8`. There are **no** `topic0` collisions and **no** 4-byte selector collisions, and every signature matches the Solidity in `contracts/src/FROSTCoordinator.sol`, `contracts/src/Consensus.sol`, `contracts/src/interfaces/IConsensus.sol` and `contracts/src/interfaces/IOracle.sol`. So there is no accidental cross-decode between the two trusted contracts; the exposure comes only from the extra watched addresses. (Script and output: see the coverage log.)
- **"The reference oracles make this unreachable."** `SentinelOracle.sol`, `SimpleOracle.sol` and `AlwaysApproveOracle.sol` declare only `OracleResult` plus dispute/bond events, none of which collide with a Coordinator or Consensus selector. With those exact contracts the injection is not reachable today. It stays a finding because (a) `oracles` is an operator-extensible allow-list whose documented trust is narrow — "The oracle contracts whose results the validator honors when signing oracle transactions" (`config.rs:66-67`) — and (b) the missing check is not a policy choice but a structural gap: `Transition` has no coordinator address to compare against.
- **"The coordinator address could change and re-introduce the problem."** Refuted: `Consensus._COORDINATOR` is `immutable` (`contracts/src/Consensus.sol:60`, `:119-120`) with no setter, so the address read once at `main.rs:49-52` cannot go stale while the process runs.
- **"The handlers' own guards are enough."** They are partial. `handle_key_gen_complained` guards on `group.id == event.gid` and on the complaint deadline (`keygen.rs:669`, `:692-695`), both of which an injecting address satisfies trivially since `gid` is public. `handle_oracle_result` _does_ bind the address (`state/sign.rs:189`, `:226`) — that is precisely the asymmetry this finding is about.
- **"The reverting transaction never leaves the process."** Refuted: the queue neither simulates nor estimates; it signs and calls `send_raw_transaction` (`core/tx/mod.rs:265`).

## Remediation options

1. Give `state::Transition` the coordinator address (it is already resolved in `main.rs:49-52` and handed to `action::Encoder`), and gate the `match` in `apply_transition` on the source: `Event::Coordinator(_)` only when `log.address == self.coordinator`, `Event::Consensus(_)` only when `log.address == self.config.consensus`, `Event::Oracle(_)` only when `self.config.oracles.contains(&log.address)`. One `if` at the top of `Message::Event`, with a `warn!` on rejection. Cheapest and most complete; no protocol change.
2. Fix it in `safenet-core` instead, so every service benefits: let `watcher_events!` associate each variant with an address supplied at construction, and have `EventWatcher` build one filter per `(address, topic-set)` pair rather than one cross-product filter. Larger change, more `eth_getLogs` calls, but removes the whole class (this is the shared root with CORE-H4 and is R1's half).
3. Defence in depth regardless of 1 or 2: reject `config.validator.oracles` entries equal to `consensus` or `coordinator` at startup, and bound the complaint flow by plaintiff as well as by accused so a single source cannot reach `threshold` on its own.

Tests to add: a `state/mod.rs` unit test that feeds `apply_transition` a `KeyGenComplained` `EventLog` whose `address` is an oracle address and asserts no `Action::KeyGenComplaintResponse` and no rollover change; the same for `Sign` asserting `next_sequence` is unchanged. Neither exists today — `state/` has zero tests and 0.0% line coverage (`codebase-map.md` Section 2).

## Trail

- Reviewer R6: drafted from lead VAL-H2 / CORE-H4. Re-opened every cited line in this checkout. Self-estimate 80% that the missing binding and the state-machine consequences are exactly as described; ~25% that the precondition (an injectable watched address) is reachable in the current deployment, which is why the impact section is written conditionally.

## Cross-reference (Critic C-CORE-A, covering R1's `core` findings)

Not a critique of this finding — its assigned Critic owns that. Recorded here so the pairing is visible from both sides, per the Critic brief's rule on the same defect appearing twice.

**F-CORE-006 is the `core`-side half of this defect and this file should be canonical.** F-CORE-006 describes the same root cause from inside `crates/core/src/index/events.rs`: the `eth_getLogs` filter is the cross product of `addresses` × `topics` (`events.rs:404-411`, `412-421`, `458-465`), and `decode_and_sort` chooses the decode from `topics`/`data` alone, copying `log.inner.address` into `EventLog` without consulting it (`events.rs:495-506`). I verified all of that independently.

I critiqued F-CORE-006 as **Plausible, 55%**, and **corrected its severity from Medium to Low**, because on `core` evidence alone what remains is a missing API affordance — `EventWatcher::new` (`events.rs:213-222`) offers no way to express per-address topic scoping, so a consumer cannot state the restriction even if it wants to — and the exploitable consequence lives here, behind the precondition this finding states honestly (an injectable watched oracle address, which R6 puts at about 25% and which your mechanical `keccak256` sweep shows is not reachable with the three oracle implementations in `contracts/src`). I did not raise `core` severity on that precondition because I have not verified it; equally, I did not lower anything here.

Also relevant to this file: **F-CORE-004** (Critic verdict Confirmed, severity corrected High → Medium) shares the same precondition for its _primary_ trigger — a watched address emitting a raw `LOG1` with a watched selector, which stalls the indexer permanently instead of driving the state machine. If the team establishes that a real deployment's `oracles` list contains a third-party or upgradeable contract, **both** F-VAL-060 and F-CORE-004 escalate together, and F-CORE-004's variant is the cheaper attack (one malformed log, no valid encoding required, network-wide and simultaneous). That should be stated wherever the precondition is finally resolved.

## Critic (C-VAL-B)

The brief asked me to settle the precondition and to spot-check the reviewer's digests independently. I did both, and I re-derived the dispatch path from `state/mod.rs:402-500`, `service/mod.rs:71-117`, `main.rs:45-79` and `core/index/events.rs` before reading the Claim.

### Independent verification of the digests, with my own Keccak-256

R6 verified the ABI with a self-written pure-Python Keccak. I did not take that on trust. I wrote my own (self-tested against `keccak256("") = c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470` and `keccak256("abc") = 4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45`) and recomputed **every** event `topic0` and **every** function selector in `bindings.rs`, expanding structs to tuples, the `FROSTGroupId.T`/`FROSTSignatureId.T`/`SafeId.T` UDVTs to `bytes32` and `Operation` to `uint8`, after checking each struct's field order against `libraries/Secp256k1.sol:18-21`, `libraries/FROST.sol:36-51`, `libraries/SafeTransaction.sol:30-43` and `FROSTCoordinator.sol:97-154`. Results:

- **No `topic0` collisions and no 4-byte selector collisions.** H-R6-1 and H-R6-2 are correctly refuted; the cross-decode explanation for this finding is genuinely dead.
- The one digest the Trigger quotes reproduces exactly: `keccak256("KeyGenComplained(bytes32,address,address,bool)") = 0xfacda0c1a23c91046de84f88c9fb4f3cd4360b4ae1b821b127968fa5d9db5fb9`.
- Spot values for the other injection targets, for QA's benefit: `Sign(address,bytes32,bytes32,bytes32,uint64)` → `b48d242879f9f3df555c800db966f65cba128c7213198748fa202ed54e092691`; `Preprocess(bytes32,address,uint64,bytes32)` → `38107eecb8be72b1b829bce317d7b161fe99c4ac90b58abda7c5ce969f196c6c`; `EpochStaged(uint64,uint64,uint64,bytes32,(uint256,uint256),bytes32,((uint256,uint256),uint256))` → `d22757d0334b80219cf27dedaf82211008f84fc412a29a27e3000dc1c6160b86`.

**One factual correction to the reviewer, not affecting the conclusion.** The Claim and `../state/coverage-logs.md#r6` both say "all 18 events". There are **17**: five on `Consensus` (`EpochProposed`, `EpochStaged`, `TransactionProposed`, `TransactionAttested`, `ValidatorStakerSet`), eleven on `Coordinator`, one on `Oracle` (`bindings.rs:113-246`). The function count of 18 is right. I recomputed all 17 and all 18; the collision-freedom claim stands, so this is a miscount in the prose, not an `H` on the substance.

### Per-claim verdicts

All twelve basis rows **Supported**. Re-opened and matched: `state/mod.rs:415-419` and `:458-462` (`log.address` reaches only `handle_oracle_result`); `service/mod.rs:102-115` (the `Transition` is built with `account`, `genesis`, an EIP-712 `ConsensusDomain` and `config` — no coordinator address, so the check is structurally impossible today, which is the sharpest observation in the finding); `main.rs:56-57`; `core/index/events.rs:402-409` (`.address(self.addresses.clone).event_signature(self.topics.clone)` — a cross product, confirmed) and `:491-503`; `keygen.rs:714-722`, `:728-730`, `:733-744`; `keygen.rs:1204-1210`, `:1214-1219`, `:1434-1438`; `frost/keygen.rs:420-428`; `core/tx/mod.rs:264-266`. Basis 10 quotes only the constructor for a claim cited at `Consensus.sol:60 and :119-120`; I checked line 60 itself — `FROSTCoordinator private immutable _COORDINATOR;` — and grepped every `_COORDINATOR` reference in the file (`:60, 120, 134, 187, 220, 223, 237, 239, 265, 284, 299`): assignment happens only in the constructor and there is no setter. **H-R6-3 is correctly refuted; the coordinator address cannot change mid-run.** No `H` claims anywhere in this finding.

### Settling the precondition — the part the brief asked for

The mechanism is not in doubt and I reached it independently. The question is whether an operator-configured oracle can emit a log the validator decodes as a `Consensus`/`FROSTCoordinator` event. My answer, from the validator side:

1. **Structurally, yes, and with no residual guard.** `oracles` is a plain `BTreeSet<Address>` from TOML (`config.rs:66-69`) with no validation of any kind — not against `consensus`, not against `coordinator`, not for contract-ness. `main.rs:57` appends the set to `watched` and the filter is the cross product, so every watched address is offered every watched `topic0`. Once decoded, `apply_transition` branches on the payload alone. Nothing downstream re-derives provenance: the handlers' own guards (`group.id == event.gid`, deadlines) are satisfied trivially because `gid` is public. So _given_ such an address, the injection works and the consequences the Claim lists are real — I re-walked the genesis-halt chain in `keygen.rs` and it lands where the Claim says.
2. **Today, with the contracts in this repository, no.** I re-read `SentinelOracle.sol`, `SimpleOracle.sol` and `AlwaysApproveOracle.sol`: their declared events are `OracleResult` plus dispute/bond/governance events, and none of those digests appears in the 17 above. So there is no in-repo configuration that reaches the mechanism. H-R6-16 is correctly refuted.
3. **The gap between 1 and 2 is an operator adding a third-party, upgradeable or later-compromised oracle** — which is precisely what the `oracles` allow-list exists to permit, since a Safenet oracle is by design a contract the validator did not write. A1 buys an honest _operator_, not an honest _oracle contract_; nothing in the assumption set makes the contents of that list trusted code. But it is also not something an attacker chooses unilaterally, and the repository ships no such oracle.

So the precondition is genuinely conditional and stays conditional: it is neither refuted nor reachable in this checkout.

### Finding verdict

**Plausible — 50%.** The brief is right that an 80% mechanism and a ~25% precondition cannot both survive into one number. The mechanism is `E2` and, unusually, _structurally_ verified — the `Transition` has no coordinator address to compare against, so this is a design gap rather than a missing `if`. The trigger requires an operator-supplied contract that does not exist in this repository. 50% is the honest product: certain mechanism, unproven and operator-gated precondition.

**Severity: High → Medium.** This is where I most disagree with the reviewer. PROMPT.md §8's High band is for liveness loss "under attacker-controlled input", and no attacker controls membership of `config.validator.oracles`; an honest operator (A1) does. What is actually established here is _missing validation at a trust boundary with contained impact_ — Medium — where the containment is supplied by the operator's own allow-list rather than by the code, which is exactly why it deserves to be fixed. If the team's deployment model is that operators will allow-list third-party oracles they have not audited, the severity is High and I would say so; that is a question about intended operations that the audit cannot settle, and I have recorded it here rather than resolving it by assertion.

**Remediation.** Option 1 is the right fix and is genuinely one `if` plus threading one `Address` into `Transition`; option 3's "reject `oracles` entries equal to `consensus` or `coordinator` at startup" should be taken regardless, since it costs nothing and closes the one variant an operator could reach by typo. Option 2 (per-address topic sets in `watcher_events!`) is the structural fix and belongs with the core-side twin **F-CORE-006**, which C-CORE-A owns; that file should be canonical for the shared root, and this one for the validator-side consequences enumerated in the Claim.

**Cross-references.** F-VAL-036 inherits this precondition wholesale and I have capped it at 40% accordingly. The `keygen.rs` complaint counter that the Claim exercises has a second, _unconditional_ defect that neither reviewer filed — `complaint.total` is monotonic while the contract's `accusations` is netted by `respond` — which I have promoted as **F-VAL-067**.

### Addendum (C-VAL-B) — disposition of R4's dangling observations O7 and O8

The Coverage Critic reports that R4's observations **O7** and **O8** (`../state/coverage-logs.md#r4`) were parked pending this finding and have no owner. Both are in scope here because both are explicitly "conditional on R6's finding". Judged against the precondition I settled above:

- **O7 — injected `KeyGenComplained` naming a non-member.** The mechanism is real and I verified its two halves: `handle_key_gen_complained` inserts `event.accused` with no `group.participants.contains(...)` check (`state/keygen.rs:714`), and `also_exclude` over an address that is not in the set leaves the set unchanged, so `participants_set` re-derives the **same** group id and `start_key_gen` re-enters `CollectingCommitments` with an empty `commitments` map for a group the contract will never re-emit `KeyGenCommitted` for. On the honest chain it is unreachable: `FROSTParticipantMap.complain` requires `accusedState.status != ParticipantStatus.NONE` (`contracts/src/libraries/FROSTParticipantMap.sol:187`), so the contract only ever emits an accused that is a registered member.
- **O8 — `handle_epoch_staged`'s `WaitingForGenesis` recovery trusting `event.proposedEpoch` (`state/keygen.rs:613-635`).** Same shape: authenticated by the contract, unauthenticated locally.

**Neither is promoted, and here is why.** Both are _additional consequences of this finding's single precondition_, not independent defects: each becomes reachable exactly when an injectable watched address exists, and neither adds a defect that survives if the address binding in remediation option 1 is applied. Filing them separately would multiply one root cause across three files and inflate the finding count without adding a fix. They are recorded here so the consequence list is complete, and they strengthen the case for option 1 rather than standing alone. The same reasoning places them beside **F-XC-050** (the Coverage Critic's own promotion of R4's O9), which took the strongest member of that family — an injected `KeyGenConfirmed` closing the confirmation round early and finalising genesis with no key share — and made it a finding in its own right. That was the right one to promote; O7 and O8 are weaker instances of it and belong in this consequence list.

**What I did promote instead** is the defect in the same complaint machinery that is _not_ conditional on any injection: `complaint.total` is monotonic while the contract's `accusations` counter is decremented by `respond`, so the Rust abort test and the Solidity `compromised` test diverge on the honest chain. That is **F-VAL-067**, filed in my range.

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain).** Certainty unchanged at **50%**; severity High / Medium unchanged — I agree with C-VAL-B's downgrade and with the reason, and I would add that the finding should carry C-VAL-B's caveat into the report verbatim, since "does the deployment model expect operators to allow-list third-party oracles" is a question the team can answer in one sentence and the audit cannot answer at all.

No PoC directory — not in my assigned set. The two tests the finding proposes are pure transition tests and the harness for them exists: [`poc/F-VAL-004/genesis_stall.rs`](../poc/F-VAL-004/genesis_stall.rs) builds a `Transition` and synthetic `EventLog`s with a settable `address` field, which is the only thing these tests need.

### What would be run, and what it would show

```rust
// using the F-VAL-004 harness, with `log.address` set to an oracle address
let log = EventLog { block: 100, index: 0, address: ORACLE, data: Event::Coordinator(
    Coordinator::CoordinatorEvents::KeyGenComplained(/* plaintiff, accused = ME */)) };
let (state, commands) = transition.apply_transition(state, Message::Event(log));
assert!(commands.is_empty());   // FAILS TODAY
```

and the `Sign` variant asserting `next_sequence` is unchanged. Both fail today and both are `E1` for the mechanism in a dozen lines each. They would **not** move the certainty, and it is worth being explicit about why: what is uncertain here is the _precondition_ — that an operator has allow-listed a contract that emits a colliding topic — not the dispatch. C-VAL-B's "certain mechanism, unproven and operator-gated precondition" is the correct reading and a green test does not change it.

C-VAL-B's observation that this is _structurally_ verified is the strongest thing in the file and should survive into the report: `state::Transition` has no coordinator address field at all (`state/mod.rs:390-401`), so there is nothing to compare against. It is a design gap, not a missing `if`, and that is why option 1 involves threading a value rather than adding a condition.

### Remediation check

**Option 1 (gate the `Message::Event` match on `log.address`) is sound and is the fix.** It is genuinely one `if` plus one `Address` on `Transition`, and the value is already resolved in `main.rs:49-52` and handed to `action::Encoder` (`service/mod.rs:106-110`), so no new plumbing to the outside world is needed. It keeps the transition pure. One caution the option does not state: the rejection branch must be a `warn!` and a `(state, Vec::new)`, **not** a panic or an error — `crates/core/src/state/mod.rs:75-77` documents that transitions are non-fallible and must gracefully recover from unexpected events, and a rejected log is exactly that case.

**Option 2 (per-address topic sets in `watcher_events!`) is the structural fix and is correctly deferred to the core-side twin F-CORE-006.** Two notes for whoever schedules it: it increases `eth_getLogs` calls, which under A4 is a real cost because a rate-limited or truncated RPC response is in scope; and it does not remove the need for option 1, because a defence at the dispatch boundary is worth having even when the filter is correct. Take option 1 now and option 2 on its own timeline.

**Option 3 is two separate things and they should be separated.** "Reject `oracles` entries equal to `consensus` or `coordinator` at startup" is sound, free, closes the one variant an operator reaches by typo, and should ship with option 1 — it also belongs in **F-VAL-063**'s `ValidatorConfig::validate`. "Bound the complaint flow by plaintiff as well as by accused" is **F-VAL-003**'s remediation option 2 and should be tracked there, not here; leaving it in this file risks it being implemented twice or not at all.

**One thing no option addresses.** Even with option 1, an oracle contract in the allow-list can still emit an `OracleResult` the validator honours — that is by design and correct — so the trust placed in `config.validator.oracles` remains total. The finding is about _unintended_ trust in those addresses; the _intended_ trust is unbounded and undocumented. A line in `crates/validator/validator.sample.toml` saying that an entry in `oracles` is fully trusted for transaction attestation would cost nothing and is the kind of thing F-VAL-063 item 3 is about.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID, with one input to the mechanical sweep refreshed.** Certainty and severity unchanged. Merge commit `a7f3915`.

The finding's core — `Transition` carries no coordinator address to compare against, so an operator-configured `oracles` entry can inject events the validator will decode — is Rust-side and `crates/validator` is untouched by the merge.

**Moved citations.** Two of this file's structural-layout references shifted: `contracts/src/libraries/Secp256k1.sol:18-21`→**`:19-22`** (`struct Point`, body unchanged) and `contracts/src/libraries/FROST.sol:36-51`→**`:38-53`** (`struct Signature` through `struct SignatureShare`, bodies unchanged — only surrounding NatSpec grew). `FROSTCoordinator.sol:97-154` is **unchanged**, sitting below the merge's first doc hunk.

**One event signature in the sweep changed, and I re-checked it.** The "18 event definitions" pass covered `FROSTCoordinator`, `Consensus`, `IConsensus` and `IOracle` — all unchanged — but the follow-on paragraph about the reference oracles cited `SentinelOracle.sol`, and the merge changed one of its events:

|  | Signature | `topic0` |
| --- | --- | --- |
| Before | `DisputeTriggered(bytes32)` | `0xb4347bd4df09c261016ca4bc94b9740dfeb948175596ca4c68dea749056be5d9` |
| After | `DisputeTriggered(bytes32,uint64)` | `0x86e8b85731e4787f033d85108356db1e068dea243be32d422e6dc5681ff49cc1` |

Computed with `cast keccak`. The new topic collides with nothing in the Coordinator or Consensus sets, so the paragraph's conclusion — "with those exact contracts the injection is not reachable today" — holds unchanged. `crates/sentinel/src/bindings.rs:42` was updated in the same merge to the new signature, so there is no decode break either. Recorded because the sweep was presented as mechanical over a fixed set, and that set has now moved once; a re-run is cheap and should be repeated on any future contract merge.
