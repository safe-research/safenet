package frost

import (
	"crypto"
	"crypto/rand"
	"errors"
	"fmt"
	"io"
	"math/big"

	_ "crypto/sha256" // register SHA-256 for crypto.SHA256

	"github.com/cloudflare/circl/expander"
	secp256k1 "github.com/decred/dcrd/dcrec/secp256k1/v4"
	"github.com/ethereum/go-ethereum/common"
	"github.com/safe-research/safenet/validator-go/contracts/coordinator"
	"github.com/safe-research/safenet/validator-go/participants"
	"github.com/safe-research/safenet/validator-go/point"
	"github.com/safe-research/safenet/validator-go/secret"
)

// Round1 contains the local state and on-chain payload produced by DKG round 1.
type Round1 struct {
	EncryptionKey *secret.EncryptionKey
	SecretPackage *Round1SecretPackage
	Commitment    coordinator.FROSTCoordinatorKeyGenCommitment
}

// Round1SecretPackage is the private state needed by DKG round 2.
type Round1SecretPackage struct {
	Identifier   *secp256k1.ModNScalar
	Coefficients []*secp256k1.ModNScalar
	Commitments  []*secp256k1.PublicKey
}

// GenerateRound1 creates the FROST DKG round-1 state for participant and returns
// the KeyGenCommitment payload expected by FROSTCoordinator.
func GenerateRound1(participant common.Address, maxSigners, minSigners uint16, r io.Reader) (*Round1, error) {
	if minSigners == 0 {
		return nil, errors.New("minSigners must be greater than zero")
	}
	if maxSigners == 0 {
		return nil, errors.New("maxSigners must be greater than zero")
	}
	if minSigners > maxSigners {
		return nil, fmt.Errorf("minSigners %d exceeds maxSigners %d", minSigners, maxSigners)
	}
	if r == nil {
		r = rand.Reader
	}

	encryptionKey, err := secret.GenerateEncryptionKey(r)
	if err != nil {
		return nil, fmt.Errorf("generate encryption key: %w", err)
	}

	identifierBytes := participants.IdentifierFromAddress(participant)
	identifier := new(secp256k1.ModNScalar)
	identifier.SetBytes(&identifierBytes)
	if identifier.IsZero() {
		return nil, errors.New("participant identifier is zero")
	}

	coefficients := make([]*secp256k1.ModNScalar, minSigners)
	commitments := make([]*secp256k1.PublicKey, minSigners)
	for i := range coefficients {
		coefficient, err := randomScalar(r)
		if err != nil {
			return nil, fmt.Errorf("generate coefficient %d: %w", i, err)
		}
		coefficients[i] = coefficient
		commitments[i] = publicKeyFromScalar(coefficient)
	}

	nonce, err := randomScalar(r)
	if err != nil {
		return nil, fmt.Errorf("generate proof nonce: %w", err)
	}
	proofR := publicKeyFromScalar(nonce)
	challenge := KeyGenChallenge(participant, commitments[0], proofR)
	mu := new(secp256k1.ModNScalar).Set(coefficients[0])
	mu.Mul(challenge).Add(nonce)

	return &Round1{
		EncryptionKey: encryptionKey,
		SecretPackage: &Round1SecretPackage{
			Identifier:   identifier,
			Coefficients: coefficients,
			Commitments:  commitments,
		},
		Commitment: coordinator.FROSTCoordinatorKeyGenCommitment{
			Q:  abiPoint(encryptionKey.PublicKey()),
			C:  abiPoints(commitments),
			R:  abiPoint(proofR),
			Mu: scalarToBig(mu),
		},
	}, nil
}

// KeyGenChallenge computes the DKG proof-of-knowledge challenge used by the
// coordinator contract: HDKG(identifier || phi_compressed || r_compressed).
func KeyGenChallenge(participant common.Address, phi, proofR *secp256k1.PublicKey) *secp256k1.ModNScalar {
	identifier := participants.IdentifierFromAddress(participant)
	input := make([]byte, 0, 98)
	input = append(input, identifier[:]...)
	input = append(input, phi.SerializeCompressed()...)
	input = append(input, proofR.SerializeCompressed()...)
	return hashToScalar("dkg", input)
}

// VerifyKeyGenProof checks the round-1 proof equation mu*G = R + c*C[0].
func VerifyKeyGenProof(participant common.Address, commitment coordinator.FROSTCoordinatorKeyGenCommitment) error {
	if len(commitment.C) == 0 {
		return errors.New("commitment has no coefficient commitments")
	}
	if commitment.Mu == nil {
		return errors.New("commitment mu is nil")
	}
	phi, err := point.FromABI(commitment.C[0].X, commitment.C[0].Y)
	if err != nil {
		return fmt.Errorf("coefficient commitment: %w", err)
	}
	proofR, err := point.FromABI(commitment.R.X, commitment.R.Y)
	if err != nil {
		return fmt.Errorf("proof commitment: %w", err)
	}
	mu, err := scalarFromBig(commitment.Mu)
	if err != nil {
		return fmt.Errorf("mu: %w", err)
	}
	challenge := KeyGenChallenge(participant, phi, proofR)

	var lhs secp256k1.JacobianPoint
	secp256k1.ScalarBaseMultNonConst(mu, &lhs)

	var phiJac, rhs, challengePhi secp256k1.JacobianPoint
	phi.AsJacobian(&phiJac)
	proofR.AsJacobian(&rhs)
	secp256k1.ScalarMultNonConst(challenge, &phiJac, &challengePhi)
	secp256k1.AddNonConst(&rhs, &challengePhi, &rhs)

	lhs.ToAffine()
	rhs.ToAffine()
	if !secp256k1.NewPublicKey(&lhs.X, &lhs.Y).IsEqual(secp256k1.NewPublicKey(&rhs.X, &rhs.Y)) {
		return errors.New("invalid keygen proof")
	}
	return nil
}

func randomScalar(r io.Reader) (*secp256k1.ModNScalar, error) {
	for {
		var b [32]byte
		if _, err := io.ReadFull(r, b[:]); err != nil {
			return nil, err
		}
		n := new(big.Int).SetBytes(b[:])
		if n.Sign() == 0 || n.Cmp(curveN) >= 0 {
			continue
		}
		s := new(secp256k1.ModNScalar)
		s.SetBytes(&b)
		return s, nil
	}
}

func publicKeyFromScalar(s *secp256k1.ModNScalar) *secp256k1.PublicKey {
	var jac secp256k1.JacobianPoint
	secp256k1.ScalarBaseMultNonConst(s, &jac)
	jac.ToAffine()
	return secp256k1.NewPublicKey(&jac.X, &jac.Y)
}

func abiPoint(pub *secp256k1.PublicKey) coordinator.Secp256k1Point {
	x, y := point.ToABI(pub)
	return coordinator.Secp256k1Point{X: x, Y: y}
}

func abiPoints(pubs []*secp256k1.PublicKey) []coordinator.Secp256k1Point {
	out := make([]coordinator.Secp256k1Point, len(pubs))
	for i, pub := range pubs {
		out[i] = abiPoint(pub)
	}
	return out
}

func scalarToBig(s *secp256k1.ModNScalar) *big.Int {
	b := s.Bytes()
	return new(big.Int).SetBytes(b[:])
}

func scalarFromBig(n *big.Int) (*secp256k1.ModNScalar, error) {
	if n.Sign() <= 0 || n.Cmp(curveN) >= 0 {
		return nil, errors.New("scalar out of range")
	}
	var b [32]byte
	n.FillBytes(b[:])
	s := new(secp256k1.ModNScalar)
	s.SetBytes(&b)
	return s, nil
}

// secp256k1 curve order N (scalar field modulus).
var curveN, _ = new(big.Int).SetString("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141", 16)

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
