package point

import (
	"errors"
	"math/big"

	secp256k1 "github.com/decred/dcrd/dcrec/secp256k1/v4"
)

// ToABI converts a secp256k1 public key to its ABI x and y coordinate pair.
func ToABI(pub *secp256k1.PublicKey) (x, y *big.Int) {
	raw := pub.SerializeUncompressed() // 0x04 || x[32] || y[32]
	x = new(big.Int).SetBytes(raw[1:33])
	y = new(big.Int).SetBytes(raw[33:65])
	return
}

// FromABI parses a secp256k1 public key from ABI x and y coordinates.
// Returns an error if the coordinates are nil, out of range, or the point is not on the curve.
func FromABI(x, y *big.Int) (*secp256k1.PublicKey, error) {
	if x == nil || y == nil {
		return nil, errors.New("coordinates must not be nil")
	}
	var raw [65]byte
	raw[0] = 0x04
	x.FillBytes(raw[1:33])
	y.FillBytes(raw[33:65])
	return secp256k1.ParsePubKey(raw[:])
}
