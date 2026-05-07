package secret

import (
	"crypto"
	"crypto/rand"
	"errors"
	"io"
	"math/big"

	_ "crypto/sha256" // register SHA-256 for crypto.SHA256

	"github.com/cloudflare/circl/expander"
	secp256k1 "github.com/decred/dcrd/dcrec/secp256k1/v4"
)

// EncryptionKey is a secp256k1 keypair used to encrypt FROST secret shares
// between participants. Encryption is ECDH-style: msg XOR (peerPub * ourPriv).x.
type EncryptionKey struct {
	secret *secp256k1.ModNScalar
	public *secp256k1.PublicKey
}

// GenerateEncryptionKey samples 32 bytes of entropy from r (or crypto/rand.Reader
// if r is nil) and derives a FROST encryption secret using hash_to_scalar with
// DST "FROST-secp256k1-SHA256-v1enc".
func GenerateEncryptionKey(r io.Reader) (*EncryptionKey, error) {
	if r == nil {
		r = rand.Reader
	}
	var entropy [32]byte
	if _, err := io.ReadFull(r, entropy[:]); err != nil {
		return nil, err
	}
	return newKey(hashToScalar("enc", entropy[:])), nil
}

// PublicKey returns the corresponding public key (G * secret).
func (k *EncryptionKey) PublicKey() *secp256k1.PublicKey {
	return k.public
}

// ECDH encrypts (or decrypts; XOR is symmetric) msg for receiverPub.
func (k *EncryptionKey) ECDH(receiverPub *secp256k1.PublicKey, msg [32]byte) ([32]byte, error) {
	return ECDH(k.secret, receiverPub, msg)
}

// ECDH computes msg XOR (receiverPub * senderPriv).x.
// Returns an error if senderPriv is zero.
func ECDH(senderPriv *secp256k1.ModNScalar, receiverPub *secp256k1.PublicKey, msg [32]byte) ([32]byte, error) {
	if senderPriv.IsZero() {
		return [32]byte{}, errors.New("private key must not be zero")
	}
	var peer secp256k1.JacobianPoint
	receiverPub.AsJacobian(&peer)
	var shared secp256k1.JacobianPoint
	secp256k1.ScalarMultNonConst(senderPriv, &peer, &shared)
	shared.ToAffine()
	var x [32]byte
	shared.X.PutBytes(&x)
	var out [32]byte
	for i := range out {
		out[i] = msg[i] ^ x[i]
	}
	return out, nil
}

func newKey(secret *secp256k1.ModNScalar) *EncryptionKey {
	var pubJac secp256k1.JacobianPoint
	secp256k1.ScalarBaseMultNonConst(secret, &pubJac)
	pubJac.ToAffine()
	return &EncryptionKey{
		secret: secret,
		public: secp256k1.NewPublicKey(&pubJac.X, &pubJac.Y),
	}
}

// secp256k1 curve order N (scalar field modulus).
var curveN, _ = new(big.Int).SetString("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141", 16)

// hashToScalar implements FROST-secp256k1-SHA256-v1 hash_to_scalar with DST
// "FROST-secp256k1-SHA256-v1" + domain.
func hashToScalar(domain string, msg []byte) *secp256k1.ModNScalar {
	dst := []byte("FROST-secp256k1-SHA256-v1" + domain)
	exp := expander.NewExpanderMD(crypto.SHA256, dst)
	uniform := exp.Expand(msg, 48)
	n := new(big.Int).SetBytes(uniform)
	n.Mod(n, curveN)
	var b [32]byte
	n.FillBytes(b[:])
	s := new(secp256k1.ModNScalar)
	s.SetBytes(&b)
	return s
}
