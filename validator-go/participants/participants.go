package participants

import (
	"bytes"
	"crypto"
	"math/big"
	"sort"

	_ "crypto/sha256" // register SHA-256 for crypto.SHA256

	"github.com/cloudflare/circl/expander"
	"github.com/ethereum/go-ethereum/common"
	ethcrypto "github.com/ethereum/go-ethereum/crypto"
)

// IdentifierFromAddress derives a FROST identifier scalar from an Ethereum address.
// Implements the HID function from FROST-secp256k1-SHA256-v1.
func IdentifierFromAddress(addr common.Address) [32]byte {
	return hashToFieldScalar(addr.Bytes())
}

// CalcParticipantsRoot computes the canonical Merkle root of a set of participants.
// Addresses are left-padded to 32 bytes, sorted, and assembled into a Merkle tree
// where each parent is keccak256(canonically sorted pair of children).
func CalcParticipantsRoot(participants []common.Address) [32]byte {
	return merkleRoot(participantLeaves(participants))
}

// CalcGenesisGroupID computes the genesis group ID for a set of participants and salt.
// If genesisSalt is zero, context is also zero; otherwise context = keccak256("genesis" || salt).
func CalcGenesisGroupID(participants []common.Address, genesisSalt [32]byte) [32]byte {
	var zeroSalt [32]byte
	var context [32]byte
	if genesisSalt != zeroSalt {
		// encodePacked(["string", "bytes32"], ["genesis", genesisSalt])
		var packed [39]byte
		copy(packed[:7], "genesis")
		copy(packed[7:], genesisSalt[:])
		context = ethcrypto.Keccak256Hash(packed[:])
	}
	root := CalcParticipantsRoot(participants)
	count := uint16(len(participants))
	threshold := count/2 + 1
	return calcGroupID(root, count, threshold, context)
}

// GenerateParticipantProof returns the Merkle proof for own within the participants set.
// Returns (nil, false) if own is not a member.
func GenerateParticipantProof(participants []common.Address, own common.Address) ([][32]byte, bool) {
	leaves := participantLeaves(participants)
	var ownLeaf [32]byte
	copy(ownLeaf[12:], own.Bytes())

	idx := -1
	for i, leaf := range leaves {
		if leaf == ownLeaf {
			idx = i
			break
		}
	}
	if idx < 0 {
		return nil, false
	}
	return merkleProof(leaves, idx), true
}

// calcGroupID is the primitive shared by CalcGenesisGroupID and future epoch group IDs.
func calcGroupID(root [32]byte, count, threshold uint16, context [32]byte) [32]byte {
	// ABI-encode (bytes32, uint16, uint16, bytes32): each field right-aligned in a 32-byte slot.
	var buf [128]byte
	copy(buf[0:32], root[:])
	buf[62] = byte(count >> 8)
	buf[63] = byte(count)
	buf[94] = byte(threshold >> 8)
	buf[95] = byte(threshold)
	copy(buf[96:128], context[:])

	gid := ethcrypto.Keccak256Hash(buf[:])
	// Mask the lower 8 bytes to zero, matching the on-chain FROSTGroupId.mask().
	for i := 24; i < 32; i++ {
		gid[i] = 0
	}
	return gid
}

func participantLeaves(participants []common.Address) [][32]byte {
	leaves := make([][32]byte, len(participants))
	for i, p := range participants {
		var leaf [32]byte
		copy(leaf[12:], p.Bytes()) // left-pad 20-byte address into 32-byte slot
		leaves[i] = leaf
	}
	sort.Slice(leaves, func(i, j int) bool {
		return bytes.Compare(leaves[i][:], leaves[j][:]) < 0
	})
	return leaves
}

func buildMerkleTree(leaves [][32]byte) [][][32]byte {
	tree := [][][32]byte{append([][32]byte(nil), leaves...)}
	for len(tree[len(tree)-1]) > 1 {
		level := tree[len(tree)-1]
		pairs := (len(level) + 1) / 2
		next := make([][32]byte, pairs)
		for i := 0; i < pairs; i++ {
			a := level[i*2]
			var b [32]byte
			if i*2+1 < len(level) {
				b = level[i*2+1]
			}
			var left, right [32]byte
			if bytes.Compare(a[:], b[:]) <= 0 {
				left, right = a, b
			} else {
				left, right = b, a
			}
			var data [64]byte
			copy(data[:32], left[:])
			copy(data[32:], right[:])
			next[i] = ethcrypto.Keccak256Hash(data[:])
		}
		tree = append(tree, next)
	}
	return tree
}

func merkleRoot(leaves [][32]byte) [32]byte {
	if len(leaves) == 0 {
		return [32]byte{}
	}
	tree := buildMerkleTree(leaves)
	return tree[len(tree)-1][0]
}

func merkleProof(leaves [][32]byte, index int) [][32]byte {
	tree := buildMerkleTree(leaves)
	height := len(tree)
	proof := make([][32]byte, 0, height-1)
	current := index
	for i := 0; i < height-1; i++ {
		neighbor := current ^ 1
		var sibling [32]byte
		if neighbor < len(tree[i]) {
			sibling = tree[i][neighbor]
		}
		proof = append(proof, sibling)
		current >>= 1
	}
	return proof
}

// secp256k1 curve order N (scalar field modulus).
var curveN, _ = new(big.Int).SetString("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141", 16)

// hashToFieldScalar implements the FROST-secp256k1-SHA256-v1 HID function:
// hash_to_field(msg, count=1) with DST "FROST-secp256k1-SHA256-v1id".
// Returns the scalar as a 32-byte big-endian value.
func hashToFieldScalar(msg []byte) [32]byte {
	dst := []byte("FROST-secp256k1-SHA256-v1id")
	// L = ceil((log2(N) + k) / 8) = ceil((256 + 128) / 8) = 48 for secp256k1, k=128.
	exp := expander.NewExpanderMD(crypto.SHA256, dst)
	uniform := exp.Expand(msg, 48)
	n := new(big.Int).SetBytes(uniform)
	n.Mod(n, curveN)
	var out [32]byte
	n.FillBytes(out[:])
	return out
}
