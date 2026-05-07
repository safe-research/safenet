package participants

import (
	"bytes"
	"encoding/hex"
	"testing"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/crypto"
)

// verifyProof checks that a Merkle proof for leaf against root is valid.
func verifyProof(root, leaf [32]byte, proof [][32]byte) bool {
	node := leaf
	for _, sibling := range proof {
		var left, right [32]byte
		if bytes.Compare(node[:], sibling[:]) <= 0 {
			left, right = node, sibling
		} else {
			left, right = sibling, node
		}
		var data [64]byte
		copy(data[:32], left[:])
		copy(data[32:], right[:])
		node = crypto.Keccak256Hash(data[:])
	}
	return node == root
}

func fromHex(t *testing.T, s string) [32]byte {
	t.Helper()
	b, err := hex.DecodeString(s)
	if err != nil {
		t.Fatalf("fromHex(%q): %v", s, err)
	}
	var out [32]byte
	copy(out[32-len(b):], b)
	return out
}

// Test vector from contracts/test/libraries/FROST.t.sol.
func TestIdentifierFromAddress(t *testing.T) {
	addr := common.HexToAddress("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")
	want := fromHex(t, "e3faf3d5fec69256091d32a1e942082b9541ff7f2c928745c0d01e922879745b")
	got := IdentifierFromAddress(addr)
	if got != want {
		t.Errorf("IdentifierFromAddress:\n got  %x\n want %x", got, want)
	}
}

// Test vector from validator/src/consensus/keyGen/utils.test.ts.
func TestCalcGroupID(t *testing.T) {
	root := fromHex(t, "0000000000000000000000000000000000000000000000000000000000000001")
	context := fromHex(t, "0000000000000000000000000000000000000000000000000000000000000002")
	want := fromHex(t, "5a646c47d456084e87ea4b1ac6ef069d1079c21f1401c60c0000000000000000")

	got := calcGroupID(root, 3, 2, context)
	if got != want {
		t.Errorf("calcGroupID:\n got  %x\n want %x", got, want)
	}
}

func TestCalcGroupIDLowerBytesZeroed(t *testing.T) {
	root := fromHex(t, "0000000000000000000000000000000000000000000000000000000000000001")
	var context [32]byte
	got := calcGroupID(root, 1, 1, context)
	for i := 24; i < 32; i++ {
		if got[i] != 0 {
			t.Errorf("calcGroupID: byte %d = %02x, want 0x00", i, got[i])
		}
	}
}

// Two-participant root: computed from sorted left-padded leaves.
func TestCalcParticipantsRootTwoParticipants(t *testing.T) {
	// Standard Anvil test accounts.
	addr0 := common.HexToAddress("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")
	addr1 := common.HexToAddress("0x70997970C51812dc3A010C7d01b50e0d17dc79C8")

	// Compute expected root manually: keccak256(sort(leaf0, leaf1)).
	var leaf0, leaf1 [32]byte
	copy(leaf0[12:], addr0.Bytes())
	copy(leaf1[12:], addr1.Bytes())
	// leaf0 (f39F...) > leaf1 (7099...) so sorted: left=leaf1, right=leaf0.
	var data [64]byte
	copy(data[:32], leaf1[:])
	copy(data[32:], leaf0[:])
	want := crypto.Keccak256Hash(data[:])

	got := CalcParticipantsRoot([]common.Address{addr0, addr1})
	if got != want {
		t.Errorf("CalcParticipantsRoot (2):\n got  %x\n want %x", got, want)
	}
}

// Result must be order-independent (addresses are sorted internally).
func TestCalcParticipantsRootOrderIndependent(t *testing.T) {
	addr0 := common.HexToAddress("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")
	addr1 := common.HexToAddress("0x70997970C51812dc3A010C7d01b50e0d17dc79C8")
	addr2 := common.HexToAddress("0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC")

	root1 := CalcParticipantsRoot([]common.Address{addr0, addr1, addr2})
	root2 := CalcParticipantsRoot([]common.Address{addr2, addr0, addr1})
	root3 := CalcParticipantsRoot([]common.Address{addr1, addr2, addr0})

	if root1 != root2 || root1 != root3 {
		t.Errorf("CalcParticipantsRoot not order-independent: %x %x %x", root1, root2, root3)
	}
}

func TestGenerateParticipantProofTwoLeaves(t *testing.T) {
	addr0 := common.HexToAddress("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")
	addr1 := common.HexToAddress("0x70997970C51812dc3A010C7d01b50e0d17dc79C8")
	participants := []common.Address{addr0, addr1}
	root := CalcParticipantsRoot(participants)

	for _, addr := range participants {
		proof, ok := GenerateParticipantProof(participants, addr)
		if !ok {
			t.Fatalf("GenerateParticipantProof: addr %s not found", addr)
		}
		if len(proof) != 1 {
			t.Errorf("proof length = %d, want 1", len(proof))
		}
		var leaf [32]byte
		copy(leaf[12:], addr.Bytes())
		if !verifyProof(root, leaf, proof) {
			t.Errorf("proof invalid for addr %s", addr)
		}
	}
}

func TestGenerateParticipantProofThreeLeaves(t *testing.T) {
	addr0 := common.HexToAddress("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")
	addr1 := common.HexToAddress("0x70997970C51812dc3A010C7d01b50e0d17dc79C8")
	addr2 := common.HexToAddress("0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC")
	participants := []common.Address{addr0, addr1, addr2}
	root := CalcParticipantsRoot(participants)

	for _, addr := range participants {
		proof, ok := GenerateParticipantProof(participants, addr)
		if !ok {
			t.Fatalf("GenerateParticipantProof: addr %s not found", addr)
		}
		if len(proof) != 2 {
			t.Errorf("proof length = %d, want 2", len(proof))
		}
		var leaf [32]byte
		copy(leaf[12:], addr.Bytes())
		if !verifyProof(root, leaf, proof) {
			t.Errorf("proof invalid for addr %s", addr)
		}
	}
}

func TestGenerateParticipantProofNotMember(t *testing.T) {
	addr0 := common.HexToAddress("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")
	addr1 := common.HexToAddress("0x70997970C51812dc3A010C7d01b50e0d17dc79C8")
	outsider := common.HexToAddress("0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC")

	_, ok := GenerateParticipantProof([]common.Address{addr0, addr1}, outsider)
	if ok {
		t.Error("expected ok=false for non-member")
	}
}

func TestCalcGenesisGroupIDZeroSalt(t *testing.T) {
	addr0 := common.HexToAddress("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")
	addr1 := common.HexToAddress("0x70997970C51812dc3A010C7d01b50e0d17dc79C8")
	participants := []common.Address{addr0, addr1}

	var zeroSalt [32]byte
	got := CalcGenesisGroupID(participants, zeroSalt)

	// With zero salt, context is zero, so verify by calling calcGroupID directly.
	root := CalcParticipantsRoot(participants)
	count := uint16(len(participants))
	threshold := count/2 + 1
	want := calcGroupID(root, count, threshold, [32]byte{})

	if got != want {
		t.Errorf("CalcGenesisGroupID (zero salt):\n got  %x\n want %x", got, want)
	}
	// Lower 8 bytes must be zero.
	for i := 24; i < 32; i++ {
		if got[i] != 0 {
			t.Errorf("byte %d = %02x, want 0x00", i, got[i])
		}
	}
}

func TestCalcGenesisGroupIDNonZeroSalt(t *testing.T) {
	addr0 := common.HexToAddress("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")
	addr1 := common.HexToAddress("0x70997970C51812dc3A010C7d01b50e0d17dc79C8")
	participants := []common.Address{addr0, addr1}

	salt := fromHex(t, "0000000000000000000000000000000000000000000000000000000000000001")
	got := CalcGenesisGroupID(participants, salt)

	// Must differ from the zero-salt result.
	zeroResult := CalcGenesisGroupID(participants, [32]byte{})
	if got == zeroResult {
		t.Error("non-zero salt should produce a different group ID")
	}
	// Lower 8 bytes must still be zero.
	for i := 24; i < 32; i++ {
		if got[i] != 0 {
			t.Errorf("byte %d = %02x, want 0x00", i, got[i])
		}
	}
}
