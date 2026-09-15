# F-VAL-037 Merkle trees pad with `B256::ZERO` and have no leaf/internal domain separation, so `B256::ZERO` is a provable leaf of most trees - safe today only by accident of what the consumers hash

| Field | Value |
| --- | --- |
| Status | QA-done |
| Crate and module | validator, merkle.rs |
| Location | crates/validator/src/merkle.rs:13-29, 47-59, 85-93 (related: crates/validator/src/consensus/group.rs:243-246, contracts/src/libraries/FROSTParticipantMap.sol:144-151) |
| Severity | Informational / Informational |
| Certainty | 60% (Critic C-VAL-B; QA may raise) |
| Assumptions involved | A7 |
| Tags | crypto |

## Claim

`MerkleTree` uses OpenZeppelin-style commutative pair hashing with no domain tag distinguishing a leaf from an internal node, and it materialises missing siblings as `B256::ZERO` at every odd level - in `build` and, symmetrically, in `proof`. Because `B256::ZERO` sorts below every other value it is always placed left, and because it is a _value in the tree_ rather than a hash of anything, a proof asserting `B256::ZERO` as a leaf verifies against the root of any tree that has an odd node count at some level.

For two of the three trees this is unreachable: nonce-set leaves are `keccak256` over 160 bytes and selection leaves `keccak256` over 192 bytes, so producing a leaf equal to `B256::ZERO` is a keccak preimage problem. The third is different. Participant leaves are the _unhashed_ left-padded address word, and the coordinator recomputes the leaf the same way, so `B256::ZERO` is precisely the leaf of `address(0)`. Any participants tree with an odd node count at any level therefore contains a valid membership proof for the zero address. It is not exploitable in this deployment because the only consumer authenticates `msg.sender`, which the EVM never sets to `address(0)` - a property of the caller, not of the scheme.

This is filed as Informational, and deliberately: the property is real, the safety margin is entirely circumstantial, and it is the kind of thing that breaks silently when a future consumer verifies a proof for an address supplied as an argument instead of taken from `msg.sender`, or when a leaf type is added whose preimage an attacker can drive to zero.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | `build` substitutes `B256::ZERO` for a missing right sibling at every level. | E2 | crates/validator/src/merkle.rs:13-29 | excerpt 1 |
| 2 | `proof` substitutes the same value for an out-of-range sibling, so the padding is consistent and a proof over it verifies. | E2 | crates/validator/src/merkle.rs:47-59 | excerpt 2 |
| 3 | Pair hashing is commutative with no leaf/internal domain tag. | E2 | crates/validator/src/merkle.rs:85-93 | excerpt 3 |
| 4 | Participant leaves are raw address words, not digests. | E2 | crates/validator/src/consensus/group.rs:243-246 | excerpt 4 |
| 5 | The contract recomputes the participant leaf identically and verifies it with OpenZeppelin's commutative `MerkleProof`. | E2 | contracts/src/libraries/FROSTParticipantMap.sol:139-151 | excerpt 5 |
| 6 | The other two leaf constructions are digests over fixed-size preimages, which is what makes them safe. | E2 | crates/validator/src/frost/preprocess.rs:162-172 | excerpt 6 |
| 7 | The repository's own tests already exercise zero padding as an ordinary leaf value, showing the padding is indistinguishable from real data. | E2 | crates/validator/src/merkle.rs:123-141 | excerpt 7 |

### Excerpts

**`crates/validator/src/merkle.rs:13-29`**

```rust
    pub fn build(leaves: Vec<B256>) -> Self {
        let mut tree = vec![leaves];
        while let Some(level) = tree.last
            && level.len() > 1
        {
            let pairs = level.len().div_ceil(2);
            let next = (0..pairs)
                .map(|i| {
                    let a = level.get(i * 2).copied.unwrap_or(B256::ZERO);
                    let b = level.get(i * 2 + 1).copied.unwrap_or(B256::ZERO);
                    hash_pair(a, b)
                })
                .collect;
            tree.push(next);
        }
        Self(tree)
    }
```

**`crates/validator/src/merkle.rs:47-59`**

```rust
    /// Generates a Merkle inclusion proof for the leaf at `index`: the sibling
    /// hashes from the leaf up to (but excluding) the root.
    pub fn proof(&self, index: usize) -> Vec<B256> {
        let len = self.height().saturating_sub(1);
        let mut current = index;
        let mut proof = Vec::with_capacity(len);
        for level in self.0.iter.take(len) {
            let sibling = current ^ 1;
            proof.push(level.get(sibling).copied.unwrap_or(B256::ZERO));
            current >>= 1;
        }
        proof
    }
```

**`crates/validator/src/merkle.rs:85-93`**

```rust
/// Hashes a pair of nodes with canonical (ascending) ordering, matching
/// OpenZeppelin's commutative `MerkleProof` hashing.
fn hash_pair(a: B256, b: B256) -> B256 {
    let (left, right) = if a <= b { (a, b) } else { (b, a) };
    let mut data = [0u8; 64];
    data[..32].copy_from_slice(left.as_slice);
    data[32..].copy_from_slice(right.as_slice);
    keccak256(data)
}
```

**`crates/validator/src/consensus/group.rs:243-246`**

```rust
/// its left-padded 32-byte address leaf.
fn participants_tree(participants: &BTreeSet<Address>) -> MerkleTree {
    let leaves = participants.iter.map(|p| p.into_word).collect;
    MerkleTree::build(leaves)
```

**`contracts/src/libraries/FROSTParticipantMap.sol:139-151`**

```solidity

    /**
     * @notice Registers a participant to the merkle tree.
     * @param self The storage struct.
     * @param participant The participant's address.
     * @param poap The Merkle proof of participation.
     */
    function register(T storage self, address participant, bytes32[] calldata poap) internal {
        ParticipantState memory state = self.states[participant];
        require(state.status == ParticipantStatus.NONE, InvalidParticipant);
        bytes32 leaf = bytes32(uint256(uint160(participant)));
        require(MerkleProof.verifyCalldata(poap, self.root, leaf), NotParticipating);
        state.status = ParticipantStatus.REGISTERED;
```

**`crates/validator/src/frost/preprocess.rs:162-172`**

```rust
/// The leaf hash for a nonce commitment at `offset` in the nonce tree. Defined
/// as `keccak256(abi.encode(offset, d.x, d.y, e.x, e.y))`.
pub(super) fn nonces_leaf(offset: u64, nonces: &bindings::SignNonces) -> B256 {
    let mut buf = [0u8; 160];
    buf[24..32].copy_from_slice(&offset.to_be_bytes);
    buf[32..64].copy_from_slice(&nonces.d.x.to_be_bytes::<32>);
    buf[64..96].copy_from_slice(&nonces.d.y.to_be_bytes::<32>);
    buf[96..128].copy_from_slice(&nonces.e.x.to_be_bytes::<32>);
    buf[128..160].copy_from_slice(&nonces.e.y.to_be_bytes::<32>);
    keccak256(buf)
}
```

**`crates/validator/src/merkle.rs:123-141`**

```rust
    #[test]
    fn generates_the_expected_merkle_root {
        let root = MerkleTree::build(vec![
            b256!("0000000000000000000000000000000000000000000000000000000000000001"),
            b256!("0000000000000000000000000000000000000000000000000000000000000002"),
            b256!("0000000000000000000000000000000000000000000000000000000000000003"),
            b256!("0000000000000000000000000000000000000000000000000000000000000004"),
            b256!("0000000000000000000000000000000000000000000000000000000000000005"),
            B256::ZERO,
            B256::ZERO,
            B256::ZERO,
        ])
        .root();

        assert_eq!(
            root,
            b256!("37e58bc84afff4e1afade4140135583af3d6d3523a435e60cec5dc75ae3d7e8b")
        );
    }
```

## Trigger

None identified against the current deployment.

The mechanism, for completeness: take a participants tree whose level 0 has an odd length `N`. `build` computes the last parent as `hash_pair(leaf[N-1], B256::ZERO)`. A verifier presented with `leaf = B256::ZERO` and the proof `[leaf[N-1], ...remaining siblings]` recomputes `hash_pair(B256::ZERO, leaf[N-1])`, which is the same value because `hash_pair` sorts its arguments, and then follows the identical path to the root. `FROSTParticipantMap.register` would accept it - it computes `leaf = bytes32(uint256(uint160(participant)))` and calls `MerkleProof.verifyCalldata` - but `participant` is `msg.sender` at its only call site, and no EVM transaction originates from `address(0)`. The same holds at every higher odd level, which yields shorter proofs for the same leaf; the participant map does not constrain proof length.

## Considered and rejected

- **Leaf/internal confusion in the nonce and selection trees.** Rejected. Nonce leaves are `keccak256` of exactly 160 bytes (basis 6, matching `contracts/src/libraries/FROSTNonceCommitmentSet.sol:147-158`) and selection leaves `keccak256` of exactly 192 bytes (`crates/validator/src/frost/sign.rs:165-179`, matching `contracts/src/libraries/FROSTSignatureShares.sol:112-129`), while internal nodes are `keccak256` of 64 bytes. Passing one off as the other needs a cross-length keccak collision. The nonce set additionally pins `proof.length == 10` (`contracts/src/libraries/FROSTNonceCommitmentSet.sol:131`), which forbids the short-proof variant entirely.
- **A non-zero internal node masquerading as an address leaf.** Rejected as negligible: an internal node is a keccak digest, so it is below `2^160` with probability about `2^-96`. This is the argument the prior analysis makes, and it is correct - but it does not cover `B256::ZERO`, which is in the tree with probability 1, and that gap is what this finding records.
- **Pad-value or ordering mismatch with Solidity.** Rejected - both sides use the same commutative sorted-pair hash (basis 3 versus `MerkleProof.processProof`), and the Rust side is only ever the prover, so its padding choice is self-consistent by construction. The vector test at basis 7 pins a tree built with explicit `B256::ZERO` leaves against a fixed root.
- **`proof(index)` returning a wrong proof for an out-of-range index (lead M4's second half).** Rejected as unreachable. I checked all three call sites: `crates/validator/src/frost/sign.rs:135` derives the index from `signers.range(..identifier).count` after `129-132` has already established the identifier is in the map; `crates/validator/src/frost/preprocess.rs:136-139` enumerates the same vector the tree was built from; `crates/validator/src/consensus/group.rs:135-136` derives it from `addresses.range(..address).count` behind a `contains` guard at `129-131`. No caller can pass `index >= leaves.len`.
- **Empty or single-leaf trees reaching a verifier.** Rejected: a single-leaf tree's root _is_ the leaf (`crates/validator/src/merkle.rs:105-109`), which would make a zero-length proof verify, but every production tree has at least two leaves - selection trees have at least `group_threshold >= 2` (`crates/validator/src/state/sign.rs:304`, `538`), nonce trees exactly 1024 (`crates/validator/src/frost/preprocess.rs:24`), participants trees at least `min_participants >= 2` (`crates/validator/src/consensus/group.rs:227-240`). The contract also rejects a zero root at `FROSTParticipantMap.init` (`contracts/src/libraries/FROSTParticipantMap.sol:124-128`).
- **Filing this against `consensus/group.rs` instead.** The unhashed leaf is R4's file; the padding and the missing domain separation are `merkle.rs`, which is this reviewer's scope and is where a fix belongs. Flagged for the Critic to cross-reference with R4's coverage.

## Remediation options

1. Domain-separate the levels: hash leaves as `keccak256(0x00 || leaf)` and internal nodes as `keccak256(0x01 || left || right)`. This is the standard fix but breaks compatibility with the three Solidity verifiers, all of which use OpenZeppelin's commutative `processProof`, so under A7 it is a protocol change, not a Rust change.
2. Cheaper and compatible: require every leaf to be a digest. `participants_tree` would hash the address word (`keccak256(p.into_word)`) instead of using it raw. This also breaks Solidity compatibility, but only in one library.
3. Cheapest and compatible: leave the scheme alone and add the invariant as a comment plus a test in `merkle.rs` stating that every leaf construction must be a `keccak256` digest over a fixed-length preimage, with `participants_tree` documented as the exception that relies on `msg.sender != address(0)`. This is documentation of an assumption that is currently implicit and load-bearing.
4. Independently, consider padding by duplicating the last node instead of using `B256::ZERO`; it removes the "value that is not a digest" from the tree entirely while remaining compatible with commutative verification.

Tests to add: a `merkle.rs` test asserting `MerkleTree::build(leaves).root.verify(B256::ZERO, &tree.proof(leaves.len))` is `true` for an odd leaf count - so that the property is recorded and any future change to the padding is caught deliberately rather than silently.

## Trail

- Reviewer R5: drafted, self-estimate 80% that the property holds as described, with impact deliberately rated Informational because the only reachable consumer authenticates `msg.sender`. Resolves lead M4: the `proof(index)` half is refuted, the domain-separation half is recorded here.

## Critic (C-VAL-B)

The brief singled out seeded lead M4 for re-checking, so I re-derived `merkle.rs` and both consumer sides from scratch, and reproduced the repository's own vectors with an independent Keccak-256 implementation rather than trusting either the code or the reviewer.

### Independent verification with a self-written Keccak-256

I wrote a pure-Python Keccak-256 (self-tested against the empty-string vector `c5d2...5a470` and `keccak256("abc") = 4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45`) and re-computed the two hard-coded vectors in this file's scope:

- `merkle.rs:124-141`'s expected root over `[1,2,3,4,5,ZERO,ZERO,ZERO]` reproduces exactly as `37e58bc84afff4e1afade4140135583af3d6d3523a435e60cec5dc75ae3d7e8b` under sorted-pair hashing.
- `frost/sign.rs:186-203`'s `signer_leaf` vector reproduces exactly as `da01939303f39ff14b730b8023a7902cfe7dc335052430e8b1efb957a909bf34` over the 192-byte preimage.

So `hash_pair`'s commutative ordering and the leaf layouts are what the code says they are, verified without reference to the reviewer.

### Per-claim verdicts

All seven basis rows **Supported**. Re-opened: `merkle.rs:13-29` (`unwrap_or(B256::ZERO)` for both `a` and `b`), `:47-59`, `:85-93`, `:123-141`; `consensus/group.rs:243-246` (`participants.iter.map(|p| p.into_word)` - unhashed); `frost/preprocess.rs:162-172`. Basis 5 cites `FROSTParticipantMap.sol:139-151`: `register` computes `bytes32 leaf = bytes32(uint256(uint160(participant)))` at `:149` and calls `MerkleProof.verifyCalldata(poap, self.root, leaf)` at `:150`, which is OpenZeppelin's commutative sorted-pair verifier - identical construction on both sides. No `H` claims.

### Two corrections to the reviewer's disposal of M4

**`proof(index)` cannot go out of bounds at all, so the caller enumeration was not needed.** R5's coverage log refutes the M4 bounds half by enumerating three callers and showing each passes an in-range index. That enumeration is correct (`frost/sign.rs:129-136`, `frost/preprocess.rs:136-139`, `consensus/group.rs:129-136`) and I re-checked it and found it complete - `grep -rn "\.proof("` over `crates/validator/src` returns exactly those three plus the two test sites. But the enumeration is belt-and-braces: `MerkleTree::proof` indexes only through `level.get(sibling).copied.unwrap_or(B256::ZERO)` (`merkle.rs:55`) and `self.0.iter.take(len)` (`:53`), both total. An out-of-range index yields a garbage proof, never a panic. The refutation is therefore _stronger_ than R5 claimed, and does not depend on the caller list staying complete as the code evolves.

**The leaf/internal confusion is bounded by work, not only by circumstance.** The finding is right that participant leaves are raw address words and that `B256::ZERO` is exactly `address(0)`'s leaf, and right that `msg.sender` can never be zero, so it is unreachable. I would add the quantitative half, because it is what makes the _other_ direction safe: passing an internal node off as a participant leaf needs a 64-byte preimage whose Keccak-256 has 12 leading zero bytes, i.e. ~2^96 work. The scheme is not one preimage away from breaking; it is one _consumer change_ away, which is precisely the reviewer's point.

### Finding verdict

**Confirmed as a property - 60%.** The mechanism is `E2` and I re-derived and numerically reproduced it; the _non_-exploitability is also `E2` (the only consumer authenticates `msg.sender`). The Trigger correctly says "none identified", so there is nothing further to prove and nothing to raise the number on: 60 records "the property is certain, the impact today is nil".

**Severity: Informational (unchanged).** Correct, and I want to record that explicitly because it is the discipline the brief asks for. A commutative Merkle tree with unhashed leaves and zero padding _looks_ like a Critical crypto finding and is not one here: no consumer can supply the zero address, and the two trees whose leaves an attacker influences hash fixed-size preimages. Filing it at Informational rather than inflating it is the right call, and the reason to keep it on the record is the forward-looking one the reviewer gives - the safety is a property of today's consumers, not of the scheme.

**Remediation note.** Of the reviewer's options, the one that costs nothing onchain is to hash the participant leaf on both sides (`keccak256(abi.encode(participant))`), which also removes the `address(0)` coincidence; anything that changes the padding value would have to change `FROSTParticipantMap`, `FROSTNonceCommitmentSet` and `FROSTSignatureShares` together, since all three verify with OpenZeppelin's commutative verifier.

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain).** Certainty unchanged at **60%**; severity Informational unchanged. No PoC directory: the finding's own `## Trigger` says "none identified", and C-VAL-B is right that there is nothing further to prove — 60 records "the property is certain, the impact today is nil", and executing the property would not move that.

### What would be run, and what it would show

The finding's suggested test, in `crates/validator/src/merkle.rs`'s existing test module: build a tree over an odd number of leaves and assert that `B256::ZERO` verifies at the padded index. It is `E1` in five lines and it should be landed **as a documentation test**, with a comment saying it pins a deliberate property rather than guarding against a bug. That framing matters more than the assertion: an unexplained "zero is a valid leaf" test in a crypto module invites someone to "fix" it.

### Remediation check

I agree with C-VAL-B's ranking and would add one point about compatibility that the options treat too lightly.

**Option 3 (document the invariant and test it) is the right choice today** and is the only one with no onchain cost. The invariant to write down is precise: _every leaf must be a `keccak256` digest over a fixed-length preimage_, which holds for `preprocess::nonces_leaf` (160 bytes, `frost/preprocess.rs:164-172`) and `sign::signer_leaf` (192 bytes, `frost/sign.rs:165-179`), and does **not** hold for `participants_tree`, whose safety rests on `msg.sender != address(0)` onchain. Naming that exception explicitly is the whole value of the option, because it is currently implicit and load-bearing.

**Option 2 (hash the participant leaf on both sides) is the one structural change worth doing** and C-VAL-B endorses it for the right reason: it removes the single leaf construction that is not a digest, which turns the invariant from "true with one exception" into "true". Its cost is a change to `FROSTParticipantMap` only, not to all three verifiers. The finding calls this "breaks Solidity compatibility"; more precisely it is a coordinated change to one library and its Rust counterpart, and it must be deployed atomically with the participant Merkle roots the validators compute — i.e. it cannot ship mid-epoch. That deployment constraint is the real cost and should be in the ticket.

**Option 1 (level domain separation) is sound cryptographically and should not be taken here.** It breaks all three Solidity verifiers, every one of which uses OpenZeppelin's commutative `processProof`, so under A7 it is a protocol change. Given the impact is currently nil, the exchange rate is poor.

**Option 4 (pad by duplicating the last node) is sound, compatible, and I would take it with option 3.** It removes the "value that is not a digest" from the tree entirely while remaining verifiable by a commutative verifier, and unlike option 1 it needs no Solidity change at all — the proof shape is unchanged. The finding lists it last; on the compatibility-versus-benefit axis it is second only to option 3, and the two together get most of option 1's benefit for none of its cost.

**Severity.** Informational is correct and I want to record the same discipline C-VAL-B did: this _looks_ like a Critical crypto finding and is not one, because no consumer can supply the zero address and the two attacker-influenced trees hash fixed-size preimages. Keeping it on the record at Informational, with the invariant written down, is the correct outcome — the safety is a property of today's consumers, not of the scheme, and the next consumer is the risk.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID — and, unusually, no citation in this file needs correcting.** Certainty **60%** and severity **Informational / Informational** unchanged. Merge commit `a7f3915`.

This finding was flagged for a line-number sweep because the merge moved addresses in `contracts/src`. It turns out **none of its contract citations moved**: every one of them lands in a library the merge did not touch. Verified with `git diff 2893917 HEAD --stat -- contracts/src/libraries/FROSTParticipantMap.sol contracts/src/libraries/FROSTNonceCommitmentSet.sol contracts/src/libraries/FROSTSignatureShares.sol`, which is empty. So all of the following are still correct as written:

| Citation | Status |
| --- | --- |
| `FROSTParticipantMap.sol:139-151` (basis row 5, excerpt 5, and the `register` leaf discussion) | unchanged |
| `FROSTParticipantMap.sol:144-151` (Location line) | unchanged |
| `FROSTParticipantMap.sol:124-128` (`init` rejects a zero root) | unchanged |
| `FROSTNonceCommitmentSet.sol:147-158` (nonce leaf preimage) | unchanged |
| `FROSTNonceCommitmentSet.sol:131` (`proof.length == 10`) | unchanged |
| `FROSTSignatureShares.sol:112-129` (selection leaf preimage) | unchanged |

The merge's only Merkle-adjacent change is documentation: `9e41b49` adds `@dev` notes to `FROSTCoordinator`'s `SignShared` event (`:256-261`) and `signShare` (`:578-582`) warning that a selection root supplied by the submitter pins the Lagrange coefficient and must not be treated as evidence of participation. That is an _offchain-consumer trust_ caveat about the selection tree, not a change to how any tree is built, padded or domain-separated. The `B256::ZERO` padding and the absent leaf/internal domain separation in `crates/validator/src/merkle.rs` are untouched (`crates/validator` is unchanged by the merge), and the "safe today only by accident of what the consumers hash" framing is unaffected — if anything the new `signShare` note is a second instance of the same pattern, a safety property held by consumer discipline rather than by construction.
