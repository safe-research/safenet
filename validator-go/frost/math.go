package frost

import (
	"errors"

	secp256k1 "github.com/decred/dcrd/dcrec/secp256k1/v4"
)

// EvalPoly evaluates a polynomial with scalar coefficients at x using Horner's
// method, modulo the secp256k1 group order N.
// coefficients[0] is the constant term. Returns an error if x is zero.
func EvalPoly(coefficients []*secp256k1.ModNScalar, x *secp256k1.ModNScalar) (*secp256k1.ModNScalar, error) {
	if x.IsZero() {
		return nil, errors.New("x must not be zero")
	}
	result := new(secp256k1.ModNScalar)
	for j := len(coefficients) - 1; j >= 0; j-- {
		result.Mul(x)
		result.Add(coefficients[j])
	}
	return result, nil
}

// EvalCommitment evaluates a commitment polynomial (secp256k1 points) at scalar x.
// For x == 0 the constant term commitments[0] is returned unchanged.
func EvalCommitment(commitments []*secp256k1.PublicKey, x *secp256k1.ModNScalar) (*secp256k1.PublicKey, error) {
	if len(commitments) == 0 {
		return nil, errors.New("commitments must not be empty")
	}
	var acc secp256k1.JacobianPoint
	commitments[0].AsJacobian(&acc)

	if !x.IsZero() {
		termPow := new(secp256k1.ModNScalar).SetInt(1)
		for j := 1; j < len(commitments); j++ {
			termPow.Mul(x)
			var cj secp256k1.JacobianPoint
			commitments[j].AsJacobian(&cj)
			var term secp256k1.JacobianPoint
			secp256k1.ScalarMultNonConst(termPow, &cj, &term)
			secp256k1.AddNonConst(&acc, &term, &acc)
		}
	}

	acc.ToAffine()
	return secp256k1.NewPublicKey(&acc.X, &acc.Y), nil
}

// CreateVerificationShare computes the verification share for senderID by
// evaluating each participant's commitment polynomial at senderID and summing
// all results.
func CreateVerificationShare(allCommitments [][]*secp256k1.PublicKey, senderID *secp256k1.ModNScalar) (*secp256k1.PublicKey, error) {
	if len(allCommitments) == 0 {
		return nil, errors.New("allCommitments must not be empty")
	}
	var acc secp256k1.JacobianPoint
	for i, commitments := range allCommitments {
		partial, err := EvalCommitment(commitments, senderID)
		if err != nil {
			return nil, err
		}
		var pt secp256k1.JacobianPoint
		partial.AsJacobian(&pt)
		if i == 0 {
			acc = pt
		} else {
			secp256k1.AddNonConst(&acc, &pt, &acc)
		}
	}
	acc.ToAffine()
	return secp256k1.NewPublicKey(&acc.X, &acc.Y), nil
}

// CreateSigningShare combines secret shares from all participants into a single
// signing share by summing them modulo N.
// Returns an error if the result is zero (invalid key) or the input is empty.
func CreateSigningShare(secretShares []*secp256k1.ModNScalar) (*secp256k1.ModNScalar, error) {
	if len(secretShares) == 0 {
		return nil, errors.New("secretShares must not be empty")
	}
	result := new(secp256k1.ModNScalar)
	for _, share := range secretShares {
		result.Add(share)
	}
	if result.IsZero() {
		return nil, errors.New("signing share is zero")
	}
	return result, nil
}

// VerifyKey reports whether G * privateKey == publicKey.
func VerifyKey(publicKey *secp256k1.PublicKey, privateKey *secp256k1.ModNScalar) bool {
	var jac secp256k1.JacobianPoint
	secp256k1.ScalarBaseMultNonConst(privateKey, &jac)
	jac.ToAffine()
	computed := secp256k1.NewPublicKey(&jac.X, &jac.Y)
	return computed.IsEqual(publicKey)
}
