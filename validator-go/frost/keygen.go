package frost

import (
	"bytes"
	"crypto"
	"crypto/rand"
	"errors"
	"fmt"
	"io"
	"math/big"
	"sort"

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

// Round2 contains the local state and on-chain payload produced by DKG round 2.
type Round2 struct {
	SecretPackage *Round2SecretPackage
	Share         coordinator.FROSTCoordinatorKeyGenSecretShare
}

// Round2SecretPackage is the private state needed by DKG round 3.
type Round2SecretPackage struct {
	Identifier      *secp256k1.ModNScalar
	OwnSigningShare *secp256k1.ModNScalar
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

// GenerateRound2 evaluates the local secret polynomial for every peer,
// encrypts those shares with ECDH, and returns the on-chain secret-share
// payload. The commitments map must contain all participants, including self.
func GenerateRound2(
	encryptionKey *secret.EncryptionKey,
	secretPackage *Round1SecretPackage,
	commitments map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment,
) (*Round2, error) {
	if encryptionKey == nil {
		return nil, errors.New("encryptionKey is nil")
	}
	if secretPackage == nil {
		return nil, errors.New("secretPackage is nil")
	}
	if secretPackage.Identifier == nil {
		return nil, errors.New("secretPackage identifier is nil")
	}
	if len(secretPackage.Coefficients) == 0 {
		return nil, errors.New("secretPackage has no coefficients")
	}
	if len(commitments) == 0 {
		return nil, errors.New("commitments must not be empty")
	}

	participants, allCommitments, err := parseRound1Commitments(commitments)
	if err != nil {
		return nil, err
	}

	ownSigningShare, err := EvalPoly(secretPackage.Coefficients, secretPackage.Identifier)
	if err != nil {
		return nil, fmt.Errorf("own signing share: %w", err)
	}

	verifyingShare, err := CreateVerificationShare(allCommitments, secretPackage.Identifier)
	if err != nil {
		return nil, fmt.Errorf("verification share: %w", err)
	}

	encryptedShares := make([]*big.Int, 0, len(participants)-1)
	seenSelf := false
	for _, participant := range participants {
		if participant.Identifier.Equals(secretPackage.Identifier) {
			seenSelf = true
			continue
		}

		share, err := EvalPoly(secretPackage.Coefficients, participant.Identifier)
		if err != nil {
			return nil, fmt.Errorf("share for %s: %w", participant.Address, err)
		}
		shareBytes := share.Bytes()
		encrypted, err := encryptionKey.ECDH(participant.EncryptionPublicKey, shareBytes)
		if err != nil {
			return nil, fmt.Errorf("encrypt share for %s: %w", participant.Address, err)
		}
		encryptedShares = append(encryptedShares, new(big.Int).SetBytes(encrypted[:]))
	}
	if !seenSelf {
		return nil, errors.New("commitments missing self")
	}

	return &Round2{
		SecretPackage: &Round2SecretPackage{
			Identifier:      new(secp256k1.ModNScalar).Set(secretPackage.Identifier),
			OwnSigningShare: ownSigningShare,
		},
		Share: coordinator.FROSTCoordinatorKeyGenSecretShare{
			Y: abiPoint(verifyingShare),
			F: encryptedShares,
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

type round1Commitment struct {
	Address             common.Address
	Identifier          *secp256k1.ModNScalar
	EncryptionPublicKey *secp256k1.PublicKey
	Commitments         []*secp256k1.PublicKey
}

func parseRound1Commitments(
	commitments map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment,
) ([]round1Commitment, [][]*secp256k1.PublicKey, error) {
	participants := make([]round1Commitment, 0, len(commitments))
	allCommitments := make([][]*secp256k1.PublicKey, 0, len(commitments))
	for address, commitment := range commitments {
		identifier, err := identifierScalarFromAddress(address)
		if err != nil {
			return nil, nil, fmt.Errorf("identifier for %s: %w", address, err)
		}
		encryptionPublicKey, err := point.FromABI(commitment.Q.X, commitment.Q.Y)
		if err != nil {
			return nil, nil, fmt.Errorf("encryption key for %s: %w", address, err)
		}
		coefficientCommitments, err := parseCoefficientCommitments(commitment.C)
		if err != nil {
			return nil, nil, fmt.Errorf("commitments for %s: %w", address, err)
		}
		participants = append(participants, round1Commitment{
			Address:             address,
			Identifier:          identifier,
			EncryptionPublicKey: encryptionPublicKey,
			Commitments:         coefficientCommitments,
		})
		allCommitments = append(allCommitments, coefficientCommitments)
	}

	sort.Slice(participants, func(i, j int) bool {
		iBytes := participants[i].Identifier.Bytes()
		jBytes := participants[j].Identifier.Bytes()
		return bytes.Compare(iBytes[:], jBytes[:]) < 0
	})

	return participants, allCommitments, nil
}

func parseCoefficientCommitments(points []coordinator.Secp256k1Point) ([]*secp256k1.PublicKey, error) {
	if len(points) == 0 {
		return nil, errors.New("empty coefficient commitments")
	}
	out := make([]*secp256k1.PublicKey, len(points))
	for i, p := range points {
		pub, err := point.FromABI(p.X, p.Y)
		if err != nil {
			return nil, fmt.Errorf("coefficient %d: %w", i, err)
		}
		out[i] = pub
	}
	return out, nil
}

func identifierScalarFromAddress(address common.Address) (*secp256k1.ModNScalar, error) {
	identifierBytes := participants.IdentifierFromAddress(address)
	identifier := new(secp256k1.ModNScalar)
	identifier.SetBytes(&identifierBytes)
	if identifier.IsZero() {
		return nil, errors.New("identifier is zero")
	}
	return identifier, nil
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
