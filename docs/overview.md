# Safenet Technical Overview

Safenet is a decentralized Safe transaction security network, where validators coordinate to generate cryptographic attestations that ensure some base level of transaction security, preventing whole categories of multisig hacks.

## Network Guarantees

Safenet has a Byzantine Fault Tolerance of 1/3, meaning we assume that fewer than 1/3 of the complete validator set will act dishonestly and not follow the rules. Under this assumption, Safenet guarantees that:

- No invalid transaction attestation will ever be produced.
- The network will never rollover into an epoch with a dishonest majority.

### Deterministic Checks

Safenet validators only ever attest to 100% deterministic outcomes. This is an important property of our network, as it ensures that honest nodes always agree, allowing it to maintain its security guarantees up to the network's Byzantine Fault Tolerance. **For the beta version of Safenet, this implies that Safe transaction validity is completely deterministic and limited to only what is included in the transaction data.**

In the future, we want Safenet to create economic incentives for _transaction checkers_ to provide real-time security information to Safe accounts. This information is inherently non-deterministic meaning that it cannot be provided by validators themselves. Instead, Safenet will open markets, where the transaction checkers participate as sophisticated market makers, to compete on providing the best possible security information to Safe users. Validators would then attest to how the market resolves, instead of attesting to the security of the transaction itself. The exact mechanism by which this will work is still in early development.

### Onchain Communication

For the initial beta release, Safenet validators communicate entirely on Gnosis Chain:

- Gnosis Chain has low gas fees, making it relatively cheap from an operational standpoint, despite the communication being onchain.
- Onchain communication provides the protocol with absolute ordering of messages as well as a global "clock" that validators can rely on for deterministic timeouts.
- Reduces the operational complexity of running a validator, as you do not need to expose a service to the scary internet.
- Users can directly interact with Safenet by executing transactions on a block explorer.
- Decreases the barrier to entry for implementing Safenet clients: all you need is a Gnosis Chain RPC node.

That being said, **onchain communication does not scale well with a large number of validators.** (in fact, it scales quadratically[^scale] with the number of validators). In the future, Safenet will have its own peer-to-peer network enabling much larger validator sets.

## FROST

The Flexible Round-Optimized Schnorr Threshold (FROST for short)[^rfc9591] protocol is at the core of the cryptographic guarantees provided by Safenet. FROST is a threshold signature scheme where a set of _participants_ share a _group key_, and a _selection_ of at least a _threshold_ of the participants can come together in a _signing ceremony_ to generate a signature for that group key.

### KeyGen

FROST requires a key generation phase to setup _signing key shares_ for each participant in order for them to create threshold signatures for the group key. Safenet specifically uses a _distributed key generation_ (DKG) scheme to set up these key among participants without any additional trust assumptions. The scheme is based on the one proposed in the original FROST paper[^frost] with some adjustments:

- Since all communication is done over **public** channels, secret shares that are sent to each validator during key generation need to be encrypted. We use a shared secret computed with ECDH to directly encrypt the secret share values between validators.
- A complaint flow[^dkg] was added in case a validator provides an invalid secret share. Since secret shares are encrypted, other validators can only verify their own shares that they receive. If they were to receive a valid one, they publicly shame the offending validator onchain, which forces them to publicly share the secret so all participants can verify it. Since it would be possible to reconstruct signing key shares if too many secret shares were revealed this way, the FROST group is marked as "tainted" if there are too many complaints to any validator. Given the fault-tolerance of the network, this would only ever happen if the validator that received too many complaints was indeed behaving maliciously, revealing the malicious validator and allowing honest participants to react accordingly.
- An additional third round for confirmation is added, allowing the onchain message coordination contract to mark a group as "ready".

### Sign

FROST signing ceremonies produce Schnorr signatures for the group key with at a selection of participants. The selection does not need to include all participants and only needs to be larger than the group threshold. Safenet's FROST implementation closely follows RFC 9591[^rfc9591], with only one small change to accommodate for the onchain communication channel:

- Nonces are precomputed and committed to in chunks onchain before the signing ceremony starts. This is required in order to prevent Wagner's generalized birthday attack. The commitment to a chunk is done as a Merkle root, where individual nonce pairs are revealed per signing ceremony with a Merkle proof (to ensure the nonce pair matches what was originally committed to in the chunk).

### Parameters

- `min_group_size = ((total_validators * 2) / 3) + 1` To protect against the _shrinking quorum attack_, Safenet requires that the participant set must always be greater than two-thirds of the total registered participants. This prevents an attacker from forcing honest validators offline to gain control of a smaller, compromised quorum.
- `threshold = (group_size / 2) + 1`: By requiring more than half of the participants to sign, Safenet ensures that only one valid consensus decision can exist at any time, preventing forks or competing attestations.

## Epochs

The network is segmented into _epochs_: periods of `0x5afe` (23294) blocks that fix a set of _participants_ from the complete validator set. During the epoch, all participants are expected to take part in _signing ceremonies_ to either attest to valid Safe transactions, or to the rollover to the next epoch. Each epoch corresponds to a new FROST KeyGen process including the participant set. This ensures that the validator key shares used for creating attestations as are periodically rotated. Therefore, each epoch has a new FROST group key.

Epoch rollover, in other words changing to a new epoch after the previous one is over, requires an attestation. The previous epoch's FROST group needs to agree on what the new epoch's FROST group should be. By having the current epoch's participant set attest to each epoch rollover, we can create an _attestation chain_ in order to cryptographically verify the current epoch state. This works by starting by some well-known _genesis epoch_ and verifying each rollover attestation until the current epoch. This enables permissionless Safenet oracles to exist on all chains supported by the Safe smart account, and is key to implementing Safe transaction guards that prevent the execution of malicious Safe transactions.

Failure to perform a key generation or sign an attestation to the epoch rollover before the start of an epoch will cause it to be skipped. In this case, the previous epoch will continue for another period, giving another chance for the validators to agree on a rollover. This ensures that the network can continue to produce transaction attestations under partial outages, allowing Safe smart accounts to continue to transaction with the added security from Safenet.

> [!NOTE]
> For the initial beta launch, the complete validator set will be fixed to a small group of 4-5 validators, and each epoch's participant set will try to include all of them. Offline validator(s) will be automatically excluded from epochs where they fail to participate in time, allowing the network to continue functioning in case of an intermittent outage from one of the nodes. In the future, we plan to increase the validator set size and allow operators to join permissionlessly.

### State Diagram

``` mermaid
---
title: Epoch Rollover
---
stateDiagram-v2
	commit : Collecting Commitments
	secret_share : Secret Sharing
	confirm : Confirming
	signing : Signing
	success : Success
	failure : Failure

	[*] --> commit

	commit --> commit : KeyGenCommitted(committed=false)
	commit --> secret_share : KeyGenCommitted(committed=true)
	commit --> failure : timeout

	secret_share --> secret_share : KeyGenSecretShared(completed=false)<br>KeyGenComplained(compromised=false)<br>KeyGenComplaintResponded()
	secret_share --> confirm : KeyGenSecretShared(completed=true)
	secret_share --> failure : KeyGenComplained(compromised=true)<br>timeout

	confirm --> confirm : KeyGenConfirmed(completed=false)<br>KeyGenComplained(compromised=false)<br>KeyGenComplaintResponded()
	confirm --> signing : KeyGenConfirmed(completed=true)
	confirm --> failure : KeyGenComplained(compromised=true)<br>complaint_timeout<br>confirm_timeout

	signing --> success : EpochStaged()

	success --> [*]
	failure --> [*]
```



[^rfc9591]: [RFC 9591 - The Flexible Round-Optimized Schnorr Threshold (FROST) Protocol for Two‑Round Schnorr Signatures](https://datatracker.ietf.org/doc/html/rfc9591)
[^frost]: [FROST: Flexible Round-Optimized Schnorr Threshold Signatures](https://web.archive.org/web/20260218133025/https://eprint.iacr.org/2020/852.pdf)
[^dkg]: [Secure Distributed Key Generation for Discrete-Log Based Cryptosystems](https://web.archive.org/web/20260218134139/https://link.springer.com/content/pdf/10.1007/s00145-006-0347-3.pdf)
[^scale]: [KeyGen Scaling on Secret Share Reveals](https://github.com/safe-research/safenet/issues/20)
