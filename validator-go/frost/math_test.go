package frost

import (
	"testing"

	secp256k1 "github.com/decred/dcrd/dcrec/secp256k1/v4"
)

// scalar returns a ModNScalar set to n.
func scalar(n uint32) *secp256k1.ModNScalar {
	return new(secp256k1.ModNScalar).SetInt(n)
}

// pub returns the secp256k1 point n*G as a PublicKey.
func pub(n uint32) *secp256k1.PublicKey {
	var jac secp256k1.JacobianPoint
	secp256k1.ScalarBaseMultNonConst(scalar(n), &jac)
	jac.ToAffine()
	return secp256k1.NewPublicKey(&jac.X, &jac.Y)
}

// TestEvalPolyHorner verifies small polynomial evaluations by hand.
//
//	[3, 5] at x=2: 5·2 + 3 = 13
//	[7, 11] at x=2: 11·2 + 7 = 29
func TestEvalPolyHorner(t *testing.T) {
	cases := []struct {
		coeffs []uint32
		x      uint32
		want   uint32
	}{
		{[]uint32{3, 5}, 2, 13},
		{[]uint32{7, 11}, 2, 29},
		{[]uint32{42}, 7, 42}, // constant polynomial
		{[]uint32{0, 1}, 5, 5}, // f(x)=x
	}
	for _, tc := range cases {
		coeffs := make([]*secp256k1.ModNScalar, len(tc.coeffs))
		for i, c := range tc.coeffs {
			coeffs[i] = scalar(c)
		}
		got, err := EvalPoly(coeffs, scalar(tc.x))
		if err != nil {
			t.Fatalf("coeffs=%v x=%d: %v", tc.coeffs, tc.x, err)
		}
		if !got.Equals(scalar(tc.want)) {
			t.Errorf("EvalPoly(%v, %d) = %v, want %d", tc.coeffs, tc.x, got, tc.want)
		}
	}
}

func TestEvalPolyZeroX(t *testing.T) {
	if _, err := EvalPoly([]*secp256k1.ModNScalar{scalar(1)}, scalar(0)); err == nil {
		t.Error("expected error for x=0")
	}
}

// TestEvalCommitmentVector mirrors the TypeScript test:
//
//	evalCommitment([G*3, G*5], 2) = G*3 + G*5*2 = G*13
func TestEvalCommitmentVector(t *testing.T) {
	commitments := []*secp256k1.PublicKey{pub(3), pub(5)}
	got, err := EvalCommitment(commitments, scalar(2))
	if err != nil {
		t.Fatalf("EvalCommitment: %v", err)
	}
	want := pub(13)
	if !got.IsEqual(want) {
		t.Error("EvalCommitment([G*3,G*5], 2) != G*13")
	}
}

func TestEvalCommitmentZeroX(t *testing.T) {
	// x=0 returns the constant term.
	got, err := EvalCommitment([]*secp256k1.PublicKey{pub(7), pub(11)}, scalar(0))
	if err != nil {
		t.Fatalf("EvalCommitment x=0: %v", err)
	}
	if !got.IsEqual(pub(7)) {
		t.Error("EvalCommitment([G*7,...], 0) != G*7")
	}
}

func TestEvalCommitmentEmpty(t *testing.T) {
	if _, err := EvalCommitment(nil, scalar(1)); err == nil {
		t.Error("expected error for empty commitments")
	}
}

// TestCreateVerificationShareVector is the cross-implementation vector from
// validator/src/frost/math.test.ts:
//
//	p1 coefficients [3,5], p2 coefficients [7,11], senderId=2
//	evalCommitment([G*3,G*5], 2) = G*13
//	evalCommitment([G*7,G*11], 2) = G*29
//	sum = G*42
func TestCreateVerificationShareVector(t *testing.T) {
	allCommitments := [][]*secp256k1.PublicKey{
		{pub(3), pub(5)},
		{pub(7), pub(11)},
	}
	got, err := CreateVerificationShare(allCommitments, scalar(2))
	if err != nil {
		t.Fatalf("CreateVerificationShare: %v", err)
	}
	if !got.IsEqual(pub(42)) {
		t.Error("CreateVerificationShare != G*42")
	}
}

func TestCreateVerificationShareEmpty(t *testing.T) {
	if _, err := CreateVerificationShare(nil, scalar(1)); err == nil {
		t.Error("expected error for empty allCommitments")
	}
}

// TestCreateSigningShareVector mirrors the TypeScript test: 3+5=8.
func TestCreateSigningShareVector(t *testing.T) {
	got, err := CreateSigningShare([]*secp256k1.ModNScalar{scalar(3), scalar(5)})
	if err != nil {
		t.Fatalf("CreateSigningShare: %v", err)
	}
	if !got.Equals(scalar(8)) {
		t.Error("CreateSigningShare([3,5]) != 8")
	}
}

func TestCreateSigningShareEmpty(t *testing.T) {
	if _, err := CreateSigningShare(nil); err == nil {
		t.Error("expected error for empty shares")
	}
}

// TestCreateSigningShareZeroSum mirrors the TypeScript test: 3 + (N-3) = 0 -> error.
func TestCreateSigningShareZeroSum(t *testing.T) {
	// N-3 is the additive inverse of 3 mod N.
	negThree := new(secp256k1.ModNScalar).SetInt(3).Negate()
	_, err := CreateSigningShare([]*secp256k1.ModNScalar{scalar(3), negThree})
	if err == nil {
		t.Error("expected error when shares sum to zero")
	}
}

func TestVerifyKeyMatch(t *testing.T) {
	for _, n := range []uint32{1, 2, 3, 42, 65537} {
		priv := scalar(n)
		if !VerifyKey(pub(n), priv) {
			t.Errorf("VerifyKey(G*%d, %d) = false, want true", n, n)
		}
	}
}

func TestVerifyKeyMismatch(t *testing.T) {
	if VerifyKey(pub(5), scalar(6)) {
		t.Error("VerifyKey(G*5, 6) = true, want false")
	}
}

// TestEvalPolyConsistentWithCommitment checks that scalar and point polynomials
// agree: EvalPoly(coeffs, x) * G == EvalCommitment(commitments, x).
func TestEvalPolyConsistentWithCommitment(t *testing.T) {
	coeffScalars := []*secp256k1.ModNScalar{scalar(3), scalar(5)}
	commitments := []*secp256k1.PublicKey{pub(3), pub(5)}
	x := scalar(7)

	s, err := EvalPoly(coeffScalars, x)
	if err != nil {
		t.Fatalf("EvalPoly: %v", err)
	}
	pt, err := EvalCommitment(commitments, x)
	if err != nil {
		t.Fatalf("EvalCommitment: %v", err)
	}

	// s * G should equal the point polynomial result.
	if !VerifyKey(pt, s) {
		t.Error("EvalPoly and EvalCommitment are inconsistent")
	}
}
