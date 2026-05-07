package secret

import (
	"bytes"
	"strings"
	"testing"

	secp256k1 "github.com/decred/dcrd/dcrec/secp256k1/v4"
)

// keyFromInt builds an EncryptionKey from a small positive integer (test helper).
func keyFromInt(t *testing.T, n uint32) *EncryptionKey {
	t.Helper()
	if n == 0 {
		t.Fatal("scalar must be non-zero")
	}
	s := new(secp256k1.ModNScalar)
	s.SetInt(n)
	return newKey(s)
}

// Test vectors mirror validator-rust/src/frost/secret.rs and validator/src/frost/secret.test.ts.

func TestECDHRoundTrip(t *testing.T) {
	alice := keyFromInt(t, 2)
	bob := keyFromInt(t, 3)
	msg := [32]byte{0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe}

	encrypted, err := alice.ECDH(bob.PublicKey(), msg)
	if err != nil {
		t.Fatalf("encrypt: %v", err)
	}
	if encrypted == msg {
		t.Errorf("ciphertext equals plaintext: %x", encrypted)
	}
	decrypted, err := bob.ECDH(alice.PublicKey(), encrypted)
	if err != nil {
		t.Fatalf("decrypt: %v", err)
	}
	if decrypted != msg {
		t.Errorf("decrypted = %x, want %x", decrypted, msg)
	}
}

func TestECDHCommutativity(t *testing.T) {
	alice := keyFromInt(t, 2)
	bob := keyFromInt(t, 3)
	var msg [32]byte
	for i := range msg {
		msg[i] = 0x42
	}

	a, err := alice.ECDH(bob.PublicKey(), msg)
	if err != nil {
		t.Fatalf("alice->bob: %v", err)
	}
	b, err := bob.ECDH(alice.PublicKey(), msg)
	if err != nil {
		t.Fatalf("bob->alice: %v", err)
	}
	if a != b {
		t.Errorf("commutativity failed:\n a=%x\n b=%x", a, b)
	}
}

func TestECDHDifferentRecipient(t *testing.T) {
	alice := keyFromInt(t, 2)
	bob := keyFromInt(t, 3)
	carol := keyFromInt(t, 5)
	var msg [32]byte
	for i := range msg {
		msg[i] = 0x42
	}

	encB, _ := alice.ECDH(bob.PublicKey(), msg)
	encC, _ := alice.ECDH(carol.PublicKey(), msg)
	if encB == encC {
		t.Error("expected different ciphertexts for different recipients")
	}
}

func TestECDHIsItsOwnInverse(t *testing.T) {
	// XOR symmetry: encrypting twice with the same key pair returns the original.
	alice := keyFromInt(t, 2)
	bob := keyFromInt(t, 3)
	msg := [32]byte{0x01, 0x02, 0x03, 0x04, 0x05}

	once, _ := alice.ECDH(bob.PublicKey(), msg)
	twice, _ := alice.ECDH(bob.PublicKey(), once)
	if twice != msg {
		t.Errorf("XOR symmetry failed: %x != %x", twice, msg)
	}
}

func TestECDHZeroPrivateKey(t *testing.T) {
	bob := keyFromInt(t, 3)
	var zero secp256k1.ModNScalar
	var msg [32]byte

	if _, err := ECDH(&zero, bob.PublicKey(), msg); err == nil {
		t.Error("expected error for zero private key")
	}
}

func TestGenerateEncryptionKeyDeterministic(t *testing.T) {
	// Identical entropy must produce identical keys.
	seed := strings.Repeat("a", 32)
	k1, err := GenerateEncryptionKey(bytes.NewReader([]byte(seed)))
	if err != nil {
		t.Fatalf("k1: %v", err)
	}
	k2, err := GenerateEncryptionKey(bytes.NewReader([]byte(seed)))
	if err != nil {
		t.Fatalf("k2: %v", err)
	}
	if !k1.secret.Equals(k2.secret) {
		t.Errorf("same entropy produced different secrets:\n k1=%x\n k2=%x",
			k1.secret.Bytes(), k2.secret.Bytes())
	}
}

func TestGenerateEncryptionKeyDifferentSeeds(t *testing.T) {
	seed1 := strings.Repeat("a", 32)
	seed2 := strings.Repeat("b", 32)
	k1, _ := GenerateEncryptionKey(bytes.NewReader([]byte(seed1)))
	k2, _ := GenerateEncryptionKey(bytes.NewReader([]byte(seed2)))
	if k1.secret.Equals(k2.secret) {
		t.Error("different seeds produced same secret")
	}
	if k1.secret.IsZero() || k2.secret.IsZero() {
		t.Error("derived secret is zero")
	}
}

func TestGenerateEncryptionKeyShortReader(t *testing.T) {
	// Reader with fewer than 32 bytes should error.
	short := bytes.NewReader([]byte("only 10 by"))
	if _, err := GenerateEncryptionKey(short); err == nil {
		t.Error("expected error from short reader")
	}
}

// Round-trip via the standalone ECDH function (not the method).
func TestECDHFunctionRoundTrip(t *testing.T) {
	alice := keyFromInt(t, 7)
	bob := keyFromInt(t, 11)
	msg := [32]byte{0x11, 0x22, 0x33}

	enc, err := ECDH(alice.secret, bob.PublicKey(), msg)
	if err != nil {
		t.Fatalf("encrypt: %v", err)
	}
	dec, err := ECDH(bob.secret, alice.PublicKey(), enc)
	if err != nil {
		t.Fatalf("decrypt: %v", err)
	}
	if dec != msg {
		t.Errorf("decrypted = %x, want %x", dec, msg)
	}
}
