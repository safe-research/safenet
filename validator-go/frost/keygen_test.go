package frost

import (
	"bytes"
	"math/big"
	"testing"

	secp256k1 "github.com/decred/dcrd/dcrec/secp256k1/v4"
	"github.com/ethereum/go-ethereum/common"
	"github.com/safe-research/safenet/validator-go/contracts/coordinator"
	"github.com/safe-research/safenet/validator-go/point"
)

func TestGenerateRound1BuildsCommitment(t *testing.T) {
	participant := common.HexToAddress("0x1111111111111111111111111111111111111111")
	round, err := GenerateRound1(participant, 3, 2, bytes.NewReader(round1Entropy()))
	if err != nil {
		t.Fatalf("GenerateRound1: %v", err)
	}

	if round.EncryptionKey == nil {
		t.Fatal("EncryptionKey is nil")
	}
	if round.SecretPackage == nil {
		t.Fatal("SecretPackage is nil")
	}
	if len(round.SecretPackage.Coefficients) != 2 {
		t.Fatalf("coefficients length = %d, want 2", len(round.SecretPackage.Coefficients))
	}
	if len(round.SecretPackage.Commitments) != 2 {
		t.Fatalf("commitments length = %d, want 2", len(round.SecretPackage.Commitments))
	}
	if len(round.Commitment.C) != 2 {
		t.Fatalf("ABI commitments length = %d, want 2", len(round.Commitment.C))
	}

	assertSamePoint(t, "q", round.Commitment.Q, round.EncryptionKey.PublicKey())
	for i, coefficient := range round.SecretPackage.Coefficients {
		assertSamePoint(t, "coefficient commitment", round.Commitment.C[i], publicKeyFromScalar(coefficient))
	}
	if err := VerifyKeyGenProof(participant, round.Commitment); err != nil {
		t.Fatalf("VerifyKeyGenProof: %v", err)
	}
}

func TestGenerateRound1RejectsInvalidParameters(t *testing.T) {
	participant := common.HexToAddress("0x1111111111111111111111111111111111111111")
	if _, err := GenerateRound1(participant, 0, 1, bytes.NewReader(round1Entropy())); err == nil {
		t.Fatal("expected error for zero maxSigners")
	}
	if _, err := GenerateRound1(participant, 1, 0, bytes.NewReader(round1Entropy())); err == nil {
		t.Fatal("expected error for zero minSigners")
	}
	if _, err := GenerateRound1(participant, 1, 2, bytes.NewReader(round1Entropy())); err == nil {
		t.Fatal("expected error when minSigners exceeds maxSigners")
	}
}

func TestGenerateRound1ShortReader(t *testing.T) {
	participant := common.HexToAddress("0x1111111111111111111111111111111111111111")
	if _, err := GenerateRound1(participant, 3, 2, bytes.NewReader([]byte("too short"))); err == nil {
		t.Fatal("expected short reader error")
	}
}

func TestVerifyKeyGenProofRejectsTamperedMu(t *testing.T) {
	participant := common.HexToAddress("0x1111111111111111111111111111111111111111")
	round, err := GenerateRound1(participant, 3, 2, bytes.NewReader(round1Entropy()))
	if err != nil {
		t.Fatalf("GenerateRound1: %v", err)
	}

	tampered := round.Commitment
	tampered.Mu = new(big.Int).Add(tampered.Mu, big.NewInt(1))
	if tampered.Mu.Cmp(curveN) >= 0 {
		tampered.Mu.Sub(tampered.Mu, curveN)
	}
	if err := VerifyKeyGenProof(participant, tampered); err == nil {
		t.Fatal("expected invalid proof error")
	}
}

func TestVerifyKeyGenProofRejectsMissingCommitment(t *testing.T) {
	participant := common.HexToAddress("0x1111111111111111111111111111111111111111")
	err := VerifyKeyGenProof(participant, coordinator.FROSTCoordinatorKeyGenCommitment{Mu: big.NewInt(1)})
	if err == nil {
		t.Fatal("expected error for missing coefficient commitments")
	}
}

func assertSamePoint(t *testing.T, label string, abi coordinator.Secp256k1Point, pub *secp256k1.PublicKey) {
	t.Helper()
	got, err := point.FromABI(abi.X, abi.Y)
	if err != nil {
		t.Fatalf("%s: ABI point invalid: %v", label, err)
	}
	if !got.IsEqual(pub) {
		t.Fatalf("%s: ABI point does not match public key", label)
	}
}

func round1Entropy() []byte {
	out := make([]byte, 0, 32*4)
	for _, b := range []byte{0x11, 0x01, 0x02, 0x03} {
		out = append(out, bytes.Repeat([]byte{b}, 32)...)
	}
	return out
}
