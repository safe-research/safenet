package frost

import (
	"bytes"
	"math/big"
	"sort"
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

func TestGenerateRound2BuildsEncryptedSecretShare(t *testing.T) {
	addresses := []common.Address{
		common.HexToAddress("0x1111111111111111111111111111111111111111"),
		common.HexToAddress("0x2222222222222222222222222222222222222222"),
		common.HexToAddress("0x3333333333333333333333333333333333333333"),
	}
	round1 := generateRound1Set(t, addresses)
	commitments := commitmentMap(addresses, round1)

	got, err := GenerateRound2(round1[0].EncryptionKey, round1[0].SecretPackage, commitments)
	if err != nil {
		t.Fatalf("GenerateRound2: %v", err)
	}
	if got.SecretPackage == nil {
		t.Fatal("SecretPackage is nil")
	}
	if !got.SecretPackage.Identifier.Equals(round1[0].SecretPackage.Identifier) {
		t.Fatal("round2 identifier does not match round1 identifier")
	}
	if got.SecretPackage.OwnSigningShare == nil {
		t.Fatal("OwnSigningShare is nil")
	}
	wantOwnShare, err := EvalPoly(round1[0].SecretPackage.Coefficients, round1[0].SecretPackage.Identifier)
	if err != nil {
		t.Fatalf("EvalPoly own: %v", err)
	}
	if !got.SecretPackage.OwnSigningShare.Equals(wantOwnShare) {
		t.Fatal("own signing share mismatch")
	}
	if len(got.Share.F) != len(addresses)-1 {
		t.Fatalf("encrypted share count = %d, want %d", len(got.Share.F), len(addresses)-1)
	}

	allCommitments := make([][]*secp256k1.PublicKey, 0, len(round1))
	for _, r := range round1 {
		allCommitments = append(allCommitments, r.SecretPackage.Commitments)
	}
	wantVerifyingShare, err := CreateVerificationShare(allCommitments, round1[0].SecretPackage.Identifier)
	if err != nil {
		t.Fatalf("CreateVerificationShare: %v", err)
	}
	assertSamePoint(t, "verifying share", got.Share.Y, wantVerifyingShare)

	recipients := sortedRecipients(t, addresses, round1[0].SecretPackage.Identifier)
	for i, recipient := range recipients {
		encrypted := bigToBytes32(t, got.Share.F[i])
		plaintext, err := round1[recipient.Index].EncryptionKey.ECDH(round1[0].EncryptionKey.PublicKey(), encrypted)
		if err != nil {
			t.Fatalf("decrypt recipient %s: %v", recipient.Address, err)
		}
		wantShare, err := EvalPoly(round1[0].SecretPackage.Coefficients, recipient.Identifier)
		if err != nil {
			t.Fatalf("EvalPoly recipient %s: %v", recipient.Address, err)
		}
		if plaintext != wantShare.Bytes() {
			t.Fatalf("decrypted share for %s = %x, want %x", recipient.Address, plaintext, wantShare.Bytes())
		}
	}
}

func TestGenerateRound2RejectsMissingSelf(t *testing.T) {
	addresses := []common.Address{
		common.HexToAddress("0x1111111111111111111111111111111111111111"),
		common.HexToAddress("0x2222222222222222222222222222222222222222"),
	}
	round1 := generateRound1Set(t, addresses)
	commitments := commitmentMap(addresses, round1)
	delete(commitments, addresses[0])

	if _, err := GenerateRound2(round1[0].EncryptionKey, round1[0].SecretPackage, commitments); err == nil {
		t.Fatal("expected missing self error")
	}
}

func TestGenerateRound2RejectsEmptyCommitments(t *testing.T) {
	participant := common.HexToAddress("0x1111111111111111111111111111111111111111")
	round, err := GenerateRound1(participant, 2, 2, bytes.NewReader(round1Entropy()))
	if err != nil {
		t.Fatalf("GenerateRound1: %v", err)
	}
	if _, err := GenerateRound2(round.EncryptionKey, round.SecretPackage, nil); err == nil {
		t.Fatal("expected empty commitments error")
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

type round2Recipient struct {
	Address    common.Address
	Identifier *secp256k1.ModNScalar
	Index      int
}

func generateRound1Set(t *testing.T, addresses []common.Address) []*Round1 {
	t.Helper()
	out := make([]*Round1, len(addresses))
	for i, address := range addresses {
		entropy := bytes.Repeat([]byte{byte(i + 1)}, 32*4)
		round, err := GenerateRound1(address, uint16(len(addresses)), 2, bytes.NewReader(entropy))
		if err != nil {
			t.Fatalf("GenerateRound1[%d]: %v", i, err)
		}
		out[i] = round
	}
	return out
}

func commitmentMap(addresses []common.Address, round1 []*Round1) map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment {
	out := make(map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment, len(addresses))
	for i, address := range addresses {
		out[address] = round1[i].Commitment
	}
	return out
}

func sortedRecipients(t *testing.T, addresses []common.Address, own *secp256k1.ModNScalar) []round2Recipient {
	t.Helper()
	recipients := make([]round2Recipient, 0, len(addresses)-1)
	for i, address := range addresses {
		identifier, err := identifierScalarFromAddress(address)
		if err != nil {
			t.Fatalf("identifier %s: %v", address, err)
		}
		if identifier.Equals(own) {
			continue
		}
		recipients = append(recipients, round2Recipient{
			Address:    address,
			Identifier: identifier,
			Index:      i,
		})
	}
	sort.Slice(recipients, func(i, j int) bool {
		iBytes := recipients[i].Identifier.Bytes()
		jBytes := recipients[j].Identifier.Bytes()
		return bytes.Compare(iBytes[:], jBytes[:]) < 0
	})
	return recipients
}

func bigToBytes32(t *testing.T, n *big.Int) [32]byte {
	t.Helper()
	if n == nil {
		t.Fatal("nil big.Int")
	}
	var out [32]byte
	n.FillBytes(out[:])
	return out
}

func round1Entropy() []byte {
	out := make([]byte, 0, 32*4)
	for _, b := range []byte{0x11, 0x01, 0x02, 0x03} {
		out = append(out, bytes.Repeat([]byte{b}, 32)...)
	}
	return out
}
