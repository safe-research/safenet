//! Key-generation group derivation.
//!
//! # Threshold and Minimum Participant Count
//!
//! Let:
//!
//! - `N` be the default number of participants;
//! - `D` be the number of participants that drop out;
//! - `M` be the number of malicious participants;
//! - `T` be the minimum number of signers required; and
//! - `t` be the fractional signing threshold.
//!
//! For every subgroup, the malicious participants must remain below the signing
//! threshold:
//!
//! `M < T`
//!
//! A subgroup contains `N - D` participants, so its threshold is approximately
//! `T = t * (N - D)`. Substitution gives:
//!
//! `M < t * (N - D)`
//!
//! Solving for `D` gives the maximum number of participants that may drop out:
//!
//! `D < N - M / t`
//!
//! To tolerate up to one third of the default participant set being malicious,
//! set `M = N / 3`:
//!
//! `D < N - N / (3 * t)`
//!
//! With `t = 1 / 2`, this becomes:
//!
//! `D < N / 3`
//!
//! Therefore, fewer than one third of the participants may drop out. Equivalently,
//! the subgroup must contain strictly more than two thirds of the default
//! participant set.

#![cfg_attr(not(test), expect(dead_code))]

use crate::{
    config::Participant,
    merkle::{MerkleRoot, MerkleTree},
};
use alloy::primitives::{Address, B256, keccak256};
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, collections::BTreeSet, num::NonZeroU64};

/// A key generation group.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Group {
    id: B256,
    root: MerkleRoot,
    participants: BTreeSet<Address>,
    excluded: BTreeSet<Address>,
    count: u16,
    threshold: u16,
    context: B256,
}

impl Group {
    /// Return the ID of the group.
    pub fn id(&self) -> B256 {
        self.id
    }

    /// Return the setup parameters for the group.
    ///
    /// These are all the required parameters for starting a key generation
    /// process onchain.
    pub fn parameters(&self) -> (MerkleRoot, u16, u16, B256) {
        (self.root, self.count, self.threshold, self.context)
    }

    /// Return the group size: the count and threshold.
    pub fn size(&self) -> (u16, u16) {
        (self.count, self.threshold)
    }

    /// Return the group's participant addresses.
    pub fn participants(&self) -> &BTreeSet<Address> {
        &self.participants
    }

    /// Returns a new set of excluded participants including the ones that were
    /// already excluded in this group along with `also_excluded`.
    pub fn also_exclude<A>(&self, also_excluded: impl IntoIterator<Item = A>) -> BTreeSet<Address>
    where
        A: Borrow<Address>,
    {
        let to_exclude = also_excluded.into_iter().map(|a| *a.borrow());
        self.excluded.iter().copied().chain(to_exclude).collect()
    }

    /// Returns a new set of excluded participants containing all participants
    /// that are not participating and in the included list.
    pub fn exclude_all_others<A>(
        &self,
        include_only: impl IntoIterator<Item = A>,
    ) -> BTreeSet<Address>
    where
        A: Borrow<Address>,
    {
        let mut to_exclude = self.participants.clone();
        for include in include_only {
            to_exclude.remove(include.borrow());
        }
        self.also_exclude(to_exclude)
    }
}

/// A participant set.
pub struct ParticipantSet {
    addresses: BTreeSet<Address>,
    excluded: BTreeSet<Address>,
    context: B256,
}

impl ParticipantSet {
    /// Returns the group for the participant set.
    pub fn group(&self) -> Group {
        let (group, _) = self.group_and_participants_tree();
        group
    }

    /// Returns the addresses in the participant set.
    pub fn addresses(&self) -> &BTreeSet<Address> {
        &self.addresses
    }

    /// Participates in a group as the specified participant set.
    ///
    /// Returns the group information as the proof of participation, or `None`
    /// if the specified address is not in the participant set.
    pub fn participate_as(&self, address: Address) -> Option<(Group, Vec<B256>)> {
        if !self.addresses.contains(&address) {
            return None;
        }

        let index = self.addresses.range(..address).count();
        let (group, participants) = self.group_and_participants_tree();
        let poap = participants.proof(index);
        Some((group, poap))
    }

    fn group_and_participants_tree(&self) -> (Group, MerkleTree) {
        let tree = participants_tree(&self.addresses);
        let count = self
            .addresses
            .len()
            .try_into()
            .expect("group length is valid by construction");
        let threshold = group_threshold(count);
        let context = self.context;

        let group = Group {
            id: group_id(tree.root(), count, threshold, context),
            root: tree.root(),
            participants: self.addresses.clone(),
            excluded: self.excluded.clone(),
            count,
            threshold,
            context,
        };
        (group, tree)
    }
}

/// The epoch details for computing a participant set.
pub enum Epoch {
    Genesis {
        salt: B256,
    },
    Number {
        consensus: Address,
        number: NonZeroU64,
        excluded: BTreeSet<Address>,
    },
}

/// The participant set active for `epoch`, returns `None` if the participant
/// set is not valid (for example, if there are not enough participants).
pub fn participants_set(participants: &[Participant], epoch: Epoch) -> Option<ParticipantSet> {
    let (mut addresses, excluded, context) = match epoch {
        Epoch::Genesis { salt } => {
            let addresses = participants
                .iter()
                .filter(|p| p.active_from == 0)
                .map(|p| p.address)
                .collect::<BTreeSet<_>>();
            let context = genesis_context(salt);
            (addresses, BTreeSet::new(), context)
        }
        Epoch::Number {
            consensus,
            number,
            excluded,
        } => {
            let addresses = participants
                .iter()
                .filter(|p| {
                    p.active_from <= number.get()
                        && p.active_before.is_none_or(|before| number < before)
                })
                .map(|p| p.address)
                .collect::<BTreeSet<_>>();
            let context = group_context(consensus, number.get());
            (addresses, excluded, context)
        }
    };

    let total = u16::try_from(addresses.len()).ok()?;
    for address in &excluded {
        addresses.remove(address);
    }

    let count = u16::try_from(addresses.len()).ok()?;
    (count >= min_participants(total)).then_some(ParticipantSet {
        addresses,
        excluded,
        context,
    })
}

/// The FROST signing threshold for a group of `count` participants.
fn group_threshold(count: u16) -> u16 {
    count / 2 + 1
}

/// The minimum number of participants that must take part in a key generation
/// for the group to be viable: strictly more than two thirds of the epoch's
/// default participant set, with a hard floor of two.
fn min_participants(count: u16) -> u16 {
    u16::max(2, count * 2 / 3 + 1)
}

/// The participants Merkle tree over the sorted `participants`, each hashed as
/// its left-padded 32-byte address leaf.
fn participants_tree(participants: &BTreeSet<Address>) -> MerkleTree {
    let leaves = participants.iter().map(|p| p.into_word()).collect();
    MerkleTree::build(leaves)
}

/// The deterministic group id for a key-generation configuration: the top 192
/// bits of `keccak256(participants, count, threshold, context)`, with the
/// low 64 bits masked to zero.
fn group_id(participants: MerkleRoot, count: u16, threshold: u16, context: B256) -> B256 {
    // `abi.encode(bytes32, uint16, uint16, bytes32)`: each value occupies a
    // 32-byte word, with the `uint16`s right-aligned (big-endian) in theirs.
    let mut buffer = [0u8; 128];
    buffer[..32].copy_from_slice(participants.0.as_slice());
    buffer[62..64].copy_from_slice(&count.to_be_bytes());
    buffer[94..96].copy_from_slice(&threshold.to_be_bytes());
    buffer[96..].copy_from_slice(context.as_slice());

    let mut id = keccak256(buffer);
    id.0[24..].fill(0);
    id
}

/// The key-generation context derived from a version number, a
/// unset, otherwise `keccak256("genesis" ++ salt)`.
fn group_context(consensus: Address, epoch: u64) -> B256 {
    const VERSION: u32 = 0;

    // `encodePacked(uint32 version, address consensus, uint64 epoch)`.
    let mut buffer = [0u8; 32];
    buffer[..4].copy_from_slice(&VERSION.to_be_bytes());
    buffer[4..24].copy_from_slice(consensus.as_slice());
    buffer[24..].copy_from_slice(&epoch.to_be_bytes());
    buffer.into()
}

/// The genesis key-generation context derived from the salt: the zero hash when
/// unset, otherwise `keccak256("genesis" ++ salt)`.
///
/// Genesis uses a different group context since we don't know the consensus
/// contract address as it depends on the genesis group ID (🐓 and 🥚 problem).
/// Instead, compute a different context based on the genesis salt (allowing the
/// genesis group ID to be parameterized and the same validator set to work
/// for multiple consensus contracts without needing to rotate the validator
/// accounts).
fn genesis_context(salt: B256) -> B256 {
    if salt == B256::ZERO {
        return B256::ZERO;
    }
    // `encodePacked(string "genesis", bytes32 salt)`.
    let mut buffer = [0u8; 7 + 32];
    buffer[..7].copy_from_slice(b"genesis");
    buffer[7..].copy_from_slice(salt.as_slice());
    keccak256(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{address, b256};
    use std::num::NonZeroU64;

    const EP06: Address = address!("0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789");
    const EP07: Address = address!("0x0000000071727De22E5E9d8BAf0edAc6f37da032");
    const EP08: Address = address!("0x4337084D9E255Ff0702461CF8895CE9E3b5Ff108");
    const EP09: Address = address!("0x433709009B8330FDa32311DF1C2AFA402eD8D009");
    const ETH: Address = address!("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE");

    const CONSENSUS: Address = address!("0x223624cBF099e5a8f8cD5aF22aFa424a1d1acEE9");

    const GENESIS_ROOT: B256 =
        b256!("0xf6a7256cea0721b8aefffe3f379ed98ea362aaf86492593bbfbda337471ecf4e");
    const GENESIS_ID: B256 =
        b256!("0x17f7ec82700b24361d1ebf306c41b6576356a5d694c2c5770000000000000000");

    fn participant(address: Address, active_from: u64, active_before: Option<u64>) -> Participant {
        Participant {
            address,
            active_from,
            active_before: active_before.map(|n| NonZeroU64::new(n).unwrap()),
        }
    }

    fn to_vec(set: &BTreeSet<Address>) -> Vec<Address> {
        set.iter().copied().collect()
    }

    #[test]
    fn thresholds_are_strict_majorities() {
        assert_eq!(group_threshold(4), 3);
        assert_eq!(group_threshold(3), 2);
        assert_eq!(group_threshold(2), 2);
    }

    #[test]
    fn minimum_participation_is_two_thirds_with_a_floor_of_two() {
        assert_eq!(min_participants(1), 2);
        assert_eq!(min_participants(2), 2);
        assert_eq!(min_participants(3), 3);
        assert_eq!(min_participants(4), 3);
        assert_eq!(min_participants(6), 5);
    }

    #[test]
    fn genesis_group_matches_the_parity_vector() {
        let participants = participants_set(
            &[
                participant(EP06, 0, None),
                participant(EP07, 0, None),
                participant(EP08, 0, Some(2)),
                participant(EP09, 0, None),
                participant(ETH, 1, None),
            ],
            Epoch::Genesis { salt: B256::ZERO },
        )
        .unwrap();

        assert_eq!(to_vec(participants.addresses()), [EP07, EP08, EP09, EP06]);

        let group = participants.group();
        assert_eq!(group.id(), GENESIS_ID);
        let (root, count, threshold, context) = group.parameters();
        assert_eq!(root, GENESIS_ROOT);
        assert_eq!(count, 4);
        assert_eq!(threshold, 3);
        assert_eq!(context, B256::ZERO);
    }

    // <https://gnosisscan.io/tx/0x02e837644c9aa6a13b0ada2c96ed3dd7bbd4685329786a8e02e8c06fa7c02688>
    #[test]
    fn actual_group() {
        let participants = participants_set(
            &[
                participant(
                    address!("0x3D58a5475c1336b0A755c3aBd298CeB9b7BB9CDe"),
                    0,
                    None,
                ),
                participant(
                    address!("0x7B0A8EFA45dE81F11F2846EC28259B62155a2b37"),
                    0,
                    None,
                ),
                participant(
                    address!("0xb0E735D4a3b70195420E0ae933689A55750CFcd2"),
                    0,
                    None,
                ),
                participant(
                    address!("0xCc00DE0eA14c08669b26DcBFE365dBD9890B04D9"),
                    0,
                    None,
                ),
                participant(
                    address!("0xD8997c2a94052C4FB79B53b3e255c1F07c99305B"),
                    0,
                    None,
                ),
                participant(
                    address!("0xF6EA21D702983c443f58A267265912FE03D2FF0b"),
                    0,
                    None,
                ),
            ],
            Epoch::Number {
                consensus: CONSENSUS,
                number: NonZeroU64::new(32729).unwrap(),
                excluded: BTreeSet::new(),
            },
        )
        .unwrap();

        assert_eq!(
            to_vec(participants.addresses()),
            [
                address!("0x3D58a5475c1336b0A755c3aBd298CeB9b7BB9CDe"),
                address!("0x7B0A8EFA45dE81F11F2846EC28259B62155a2b37"),
                address!("0xb0E735D4a3b70195420E0ae933689A55750CFcd2"),
                address!("0xCc00DE0eA14c08669b26DcBFE365dBD9890B04D9"),
                address!("0xD8997c2a94052C4FB79B53b3e255c1F07c99305B"),
                address!("0xF6EA21D702983c443f58A267265912FE03D2FF0b"),
            ]
        );

        let me = address!("0xCc00DE0eA14c08669b26DcBFE365dBD9890B04D9");
        let (group, poap) = participants.participate_as(me).unwrap();
        assert_eq!(
            group.id(),
            b256!("0x1c1b36e6366d94fd1127b605ce6846e4521c626b8852f37c0000000000000000")
        );
        assert_eq!(
            poap,
            [
                b256!("0x000000000000000000000000b0e735d4a3b70195420e0ae933689a55750cfcd2"),
                b256!("0xdcf3d8a95ead5735955afd4df827e588541656c74c06ec2b6b4d5fc2c041b3b2"),
                b256!("0x8a9bb3a37cb7d649e7b6858d0705d8312ec9cd28884242584e231ddd6f4a38fc"),
            ]
        );
        let (root, count, threshold, context) = group.parameters();
        assert!(root.verify(me.into_word(), &poap));
        assert_eq!(
            root,
            b256!("0xe861f2f29f42f74128d3d7ad30889ab7e3e4345e4a0404f1dc7f15161583da9d")
        );
        assert_eq!(count, 6);
        assert_eq!(threshold, 4);
        assert_eq!(
            context,
            b256!("0x00000000223624cbf099e5a8f8cd5af22afa424a1d1acee90000000000007fd9")
        );
    }
}
