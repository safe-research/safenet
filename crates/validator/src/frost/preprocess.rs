//! The 1024-nonce preprocessing scheme layered over standard FROST nonces.
//!
//! Each nonces' merkle proof is **precomputed at generation time** (when the
//! tree is built once anyway) and stored alongside it in a [`Nonces`], so
//! revealing a single nonce needs only that one entry rather than the whole
//! chunk resident in memory.

use super::{keygen::KeyShare, marshal};
use crate::{
    bindings,
    merkle::{MerkleRoot, MerkleTree},
};
use alloy::primitives::{B256, keccak256};
use frost_secp256k1::round1;
use rand::{CryptoRng, RngCore, SeedableRng as _};
use rand_chacha::ChaCha12Rng;
use rayon::iter::{IntoParallelIterator as _, ParallelIterator as _};

/// The number of nonces committed to per `preprocess`, matching the onchain
/// `FROSTNonceCommitmentSet` sequence chunk size.
pub const SEQUENCE_CHUNK_SIZE: u64 = 1024;

/// A single preprocessing nonce pair and its precomputed inclusion proof in the
/// chunk's merkle tree. Because the proof is stored with the nonce, a nonce can
/// be revealed in isolation without the rest of the chunk.
pub struct Nonces {
    signing_nonces: round1::SigningNonces,
    proof: Vec<B256>,
}

impl Nonces {
    /// Compute the signature nonces reveal parameters the FROST signing nonce
    /// pair.
    pub fn reveal(&self) -> (bindings::SignNonces, &[B256]) {
        (
            marshal::solidity_sign_nonces(self.signing_nonces.commitments()),
            &self.proof,
        )
    }

    /// The secret FROST signing nonce pair, consumed when producing a signature
    /// share.
    pub(super) fn signing_nonces(&self) -> &round1::SigningNonces {
        &self.signing_nonces
    }
}

/// A freshly generated chunk of preprocessing nonces: the per-offset secrets
/// (each carrying its own proof) to persist and the merkle `commitment` to
/// publish onchain.
pub struct NonceChunk {
    /// The `SEQUENCE_CHUNK_SIZE` nonces, indexed by offset. Secret; persisted to
    /// the secret store and never rolled back.
    pub nonces: Vec<Nonces>,
    /// The merkle root over the nonce commitments, published by `preprocess`.
    pub commitment: MerkleRoot,
}

impl NonceChunk {
    /// Generates a fresh chunk of `SEQUENCE_CHUNK_SIZE` nonces for `key_share`,
    /// the merkle root committing to them, and each nonce's inclusion proof. The
    /// RNG is supplied by the caller.
    pub fn generate<R>(key_share: &KeyShare, rng: &mut R) -> Result<NonceChunk, rand::Error>
    where
        R: RngCore + CryptoRng,
    {
        Self::with_size(SEQUENCE_CHUNK_SIZE, key_share, rng)
    }

    /// Same as [`Self::generate`] but with a user-specified chunk size.
    ///
    /// This allows for smaller nonce chunks for testing.
    pub fn with_size<R>(
        size: u64,
        key_share: &KeyShare,
        rng: &mut R,
    ) -> Result<NonceChunk, rand::Error>
    where
        R: RngCore + CryptoRng,
    {
        // Parallelize nonce generation to speed up the process. Note that this
        // requires us to seed one RNG per nonce pair, as the `R` passed in
        // cannot be shared across threads. The choice of [`ChaCha12Rng`] is
        // based on the fact that it is the standard RNG used by [`rand`] (both
        // the `ThreadRng` and `StdRng`), it is a cryptographically secure RNG,
        // and it allows unique random streams per nonce generation.
        let rngs = (0..size)
            .map({
                let seed = ChaCha12Rng::from_rng(&mut *rng)?;
                move |offset| {
                    let mut rng = seed.clone();
                    rng.set_stream(offset.checked_add(1).expect("chunk too large"));
                    Ok((offset, rng))
                }
            })
            .collect::<Result<Vec<_>, rand::Error>>()?;
        let signing_share = key_share.as_key_package().signing_share();
        let (signing_nonces, leaves) = rngs
            .into_par_iter()
            .map(|(offset, mut rng)| {
                let signing_nonces = round1::SigningNonces::new(signing_share, &mut rng);
                let marshaled = marshal::solidity_sign_nonces(signing_nonces.commitments());
                let leaf = nonces_leaf(offset, &marshaled);
                (signing_nonces, leaf)
            })
            .unzip::<_, _, Vec<_>, Vec<_>>();

        let tree = MerkleTree::build(leaves);
        let nonces = signing_nonces
            .into_iter()
            .enumerate()
            .map(|(i, signing_nonces)| Nonces {
                signing_nonces,
                proof: tree.proof(i),
            })
            .collect();

        Ok(NonceChunk {
            nonces,
            commitment: tree.root(),
        })
    }
}

/// The leaf hash for a nonce commitment at `offset` in the nonce tree. Defined
/// as `keccak256(abi.encode(offset, d.x, d.y, e.x, e.y))`.
pub(super) fn nonces_leaf(offset: u64, nonces: &bindings::SignNonces) -> B256 {
    let mut buf = [0u8; 160];
    buf[24..32].copy_from_slice(&offset.to_be_bytes());
    buf[32..64].copy_from_slice(&nonces.d.x.to_be_bytes::<32>());
    buf[64..96].copy_from_slice(&nonces.d.y.to_be_bytes::<32>());
    buf[96..128].copy_from_slice(&nonces.e.x.to_be_bytes::<32>());
    buf[128..160].copy_from_slice(&nonces.e.y.to_be_bytes::<32>());
    keccak256(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_key_share() -> KeyShare {
        KeyShare::from_key_package(frost_secp256k1::keys::KeyPackage::new(
            frost_secp256k1::Identifier::try_from(1).unwrap(),
            frost_secp256k1::keys::SigningShare::new(k256::Scalar::ONE),
            frost_secp256k1::keys::VerifyingShare::new(k256::ProjectivePoint::GENERATOR),
            frost_secp256k1::VerifyingKey::new(k256::ProjectivePoint::GENERATOR),
            1,
        ))
    }

    #[test]
    fn nonces_commitment_inclusion_proofs() {
        let chunk = NonceChunk::generate(&dummy_key_share(), &mut rand::thread_rng()).unwrap();
        assert_eq!(chunk.nonces.len(), SEQUENCE_CHUNK_SIZE as usize);

        for offset in [0, 1, 42, 777, (SEQUENCE_CHUNK_SIZE - 1) as usize] {
            let (nonces, proof) = &chunk.nonces[offset].reveal();
            let leaf = nonces_leaf(offset as _, nonces);
            assert!(chunk.commitment.verify(leaf, proof));
        }
    }
}
