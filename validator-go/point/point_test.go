package point

import (
	"math/big"
	"testing"

	secp256k1 "github.com/decred/dcrd/dcrec/secp256k1/v4"
)

func pubFromScalar(t *testing.T, n uint32) *secp256k1.PublicKey {
	t.Helper()
	var s secp256k1.ModNScalar
	s.SetInt(n)
	var jac secp256k1.JacobianPoint
	secp256k1.ScalarBaseMultNonConst(&s, &jac)
	jac.ToAffine()
	return secp256k1.NewPublicKey(&jac.X, &jac.Y)
}

func TestRoundTrip(t *testing.T) {
	for _, scalar := range []uint32{1, 2, 3, 7, 42, 65537} {
		pub := pubFromScalar(t, scalar)
		x, y := ToABI(pub)
		got, err := FromABI(x, y)
		if err != nil {
			t.Fatalf("scalar=%d: FromABI: %v", scalar, err)
		}
		if !got.IsEqual(pub) {
			t.Errorf("scalar=%d: round-trip mismatch", scalar)
		}
	}
}

// Test vectors: well-known secp256k1 generator coordinates.
// Gx/Gy are specified in the secp256k1 standard (SEC 2, section 2.4.1).
func TestKnownGeneratorPoint(t *testing.T) {
	Gx, _ := new(big.Int).SetString("79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798", 16)
	Gy, _ := new(big.Int).SetString("483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8", 16)

	pub, err := FromABI(Gx, Gy)
	if err != nil {
		t.Fatalf("FromABI G: %v", err)
	}

	G := pubFromScalar(t, 1)
	if !pub.IsEqual(G) {
		t.Errorf("parsed generator does not match scalar×G")
	}

	x, y := ToABI(G)
	if x.Cmp(Gx) != 0 || y.Cmp(Gy) != 0 {
		t.Errorf("ToABI(G) = (%x, %x), want (%x, %x)", x, y, Gx, Gy)
	}
}

func TestFromABIPointNotOnCurve(t *testing.T) {
	// (1, 1) is not on secp256k1.
	if _, err := FromABI(big.NewInt(1), big.NewInt(1)); err == nil {
		t.Error("expected error for point not on curve")
	}
}

func TestFromABINilCoordinates(t *testing.T) {
	one := big.NewInt(1)
	if _, err := FromABI(nil, one); err == nil {
		t.Error("expected error for nil x")
	}
	if _, err := FromABI(one, nil); err == nil {
		t.Error("expected error for nil y")
	}
}

func TestToABICoordinateSizes(t *testing.T) {
	// Both coordinates must fit in exactly 32 bytes.
	pub := pubFromScalar(t, 1)
	x, y := ToABI(pub)
	if x.BitLen() > 256 {
		t.Errorf("x exceeds 256 bits: %d", x.BitLen())
	}
	if y.BitLen() > 256 {
		t.Errorf("y exceeds 256 bits: %d", y.BitLen())
	}
}

func TestFromABICoordinateOutOfRange(t *testing.T) {
	// x larger than the field prime p should be rejected.
	p, _ := new(big.Int).SetString("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F", 16)
	overflow := new(big.Int).Add(p, big.NewInt(1))
	if _, err := FromABI(overflow, big.NewInt(0)); err == nil {
		t.Error("expected error for x >= p")
	}
}
