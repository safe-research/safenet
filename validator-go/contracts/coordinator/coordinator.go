// Code generated - DO NOT EDIT.
// This file is a generated binding and any manual changes will be lost.

package coordinator

import (
	"errors"
	"math/big"
	"strings"

	ethereum "github.com/ethereum/go-ethereum"
	"github.com/ethereum/go-ethereum/accounts/abi"
	"github.com/ethereum/go-ethereum/accounts/abi/bind"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/event"
)

// Reference imports to suppress errors if they are not otherwise used.
var (
	_ = errors.New
	_ = big.NewInt
	_ = strings.NewReader
	_ = ethereum.NotFound
	_ = bind.Bind
	_ = common.Big1
	_ = types.BloomLookup
	_ = event.NewSubscription
	_ = abi.ConvertType
)

// FROSTCoordinatorCallback is an auto generated low-level Go binding around an user-defined struct.
type FROSTCoordinatorCallback struct {
	Target  common.Address
	Context []byte
}

// FROSTCoordinatorKeyGenCommitment is an auto generated low-level Go binding around an user-defined struct.
type FROSTCoordinatorKeyGenCommitment struct {
	Q  Secp256k1Point
	C  []Secp256k1Point
	R  Secp256k1Point
	Mu *big.Int
}

// FROSTCoordinatorKeyGenSecretShare is an auto generated low-level Go binding around an user-defined struct.
type FROSTCoordinatorKeyGenSecretShare struct {
	Y Secp256k1Point
	F []*big.Int
}

// FROSTCoordinatorSignNonces is an auto generated low-level Go binding around an user-defined struct.
type FROSTCoordinatorSignNonces struct {
	D Secp256k1Point
	E Secp256k1Point
}

// FROSTCoordinatorSignSelection is an auto generated low-level Go binding around an user-defined struct.
type FROSTCoordinatorSignSelection struct {
	R    Secp256k1Point
	Root [32]byte
}

// FROSTSignature is an auto generated low-level Go binding around an user-defined struct.
type FROSTSignature struct {
	R Secp256k1Point
	Z *big.Int
}

// FROSTSignatureShare is an auto generated low-level Go binding around an user-defined struct.
type FROSTSignatureShare struct {
	R Secp256k1Point
	Z *big.Int
	L *big.Int
}

// Secp256k1Point is an auto generated low-level Go binding around an user-defined struct.
type Secp256k1Point struct {
	X *big.Int
	Y *big.Int
}

// FROSTCoordinatorMetaData contains all meta data concerning the FROSTCoordinator contract.
var FROSTCoordinatorMetaData = &bind.MetaData{
	ABI: "[{\"type\":\"function\",\"name\":\"groupKey\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"}],\"outputs\":[{\"name\":\"key\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"groupParameters\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"}],\"outputs\":[{\"name\":\"participants\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"},{\"name\":\"count\",\"type\":\"uint16\",\"internalType\":\"uint16\"},{\"name\":\"threshold\",\"type\":\"uint16\",\"internalType\":\"uint16\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"groupSignCount\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"}],\"outputs\":[{\"name\":\"result\",\"type\":\"uint256\",\"internalType\":\"uint256\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"keyGen\",\"inputs\":[{\"name\":\"participants\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"},{\"name\":\"count\",\"type\":\"uint16\",\"internalType\":\"uint16\"},{\"name\":\"threshold\",\"type\":\"uint16\",\"internalType\":\"uint16\"},{\"name\":\"context\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"}],\"outputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"keyGenAndCommit\",\"inputs\":[{\"name\":\"participants\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"},{\"name\":\"count\",\"type\":\"uint16\",\"internalType\":\"uint16\"},{\"name\":\"threshold\",\"type\":\"uint16\",\"internalType\":\"uint16\"},{\"name\":\"context\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"},{\"name\":\"poap\",\"type\":\"bytes32[]\",\"internalType\":\"bytes32[]\"},{\"name\":\"commitment\",\"type\":\"tuple\",\"internalType\":\"structFROSTCoordinator.KeyGenCommitment\",\"components\":[{\"name\":\"q\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"c\",\"type\":\"tuple[]\",\"internalType\":\"structSecp256k1.Point[]\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"mu\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"outputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"committed\",\"type\":\"bool\",\"internalType\":\"bool\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"keyGenCommit\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"poap\",\"type\":\"bytes32[]\",\"internalType\":\"bytes32[]\"},{\"name\":\"commitment\",\"type\":\"tuple\",\"internalType\":\"structFROSTCoordinator.KeyGenCommitment\",\"components\":[{\"name\":\"q\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"c\",\"type\":\"tuple[]\",\"internalType\":\"structSecp256k1.Point[]\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"mu\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"outputs\":[{\"name\":\"committed\",\"type\":\"bool\",\"internalType\":\"bool\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"keyGenComplain\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"accused\",\"type\":\"address\",\"internalType\":\"address\"}],\"outputs\":[{\"name\":\"compromised\",\"type\":\"bool\",\"internalType\":\"bool\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"keyGenComplaintResponse\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"plaintiff\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"secretShare\",\"type\":\"uint256\",\"internalType\":\"uint256\"}],\"outputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"keyGenConfirm\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"}],\"outputs\":[{\"name\":\"confirmed\",\"type\":\"bool\",\"internalType\":\"bool\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"keyGenConfirmWithCallback\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"callback\",\"type\":\"tuple\",\"internalType\":\"structFROSTCoordinator.Callback\",\"components\":[{\"name\":\"target\",\"type\":\"address\",\"internalType\":\"contractIFROSTCoordinatorCallback\"},{\"name\":\"context\",\"type\":\"bytes\",\"internalType\":\"bytes\"}]}],\"outputs\":[{\"name\":\"confirmed\",\"type\":\"bool\",\"internalType\":\"bool\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"keyGenSecretShare\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"share\",\"type\":\"tuple\",\"internalType\":\"structFROSTCoordinator.KeyGenSecretShare\",\"components\":[{\"name\":\"y\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"f\",\"type\":\"uint256[]\",\"internalType\":\"uint256[]\"}]}],\"outputs\":[{\"name\":\"shared\",\"type\":\"bool\",\"internalType\":\"bool\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"participantKey\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"participant\",\"type\":\"address\",\"internalType\":\"address\"}],\"outputs\":[{\"name\":\"key\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"preprocess\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"commitment\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"}],\"outputs\":[{\"name\":\"chunk\",\"type\":\"uint64\",\"internalType\":\"uint64\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"sign\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"message\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"}],\"outputs\":[{\"name\":\"sid\",\"type\":\"bytes32\",\"internalType\":\"FROSTSignatureId.T\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"signRevealNonces\",\"inputs\":[{\"name\":\"sid\",\"type\":\"bytes32\",\"internalType\":\"FROSTSignatureId.T\"},{\"name\":\"nonces\",\"type\":\"tuple\",\"internalType\":\"structFROSTCoordinator.SignNonces\",\"components\":[{\"name\":\"d\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"e\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}]},{\"name\":\"proof\",\"type\":\"bytes32[]\",\"internalType\":\"bytes32[]\"}],\"outputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"signShare\",\"inputs\":[{\"name\":\"sid\",\"type\":\"bytes32\",\"internalType\":\"FROSTSignatureId.T\"},{\"name\":\"selection\",\"type\":\"tuple\",\"internalType\":\"structFROSTCoordinator.SignSelection\",\"components\":[{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"root\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"}]},{\"name\":\"share\",\"type\":\"tuple\",\"internalType\":\"structFROST.SignatureShare\",\"components\":[{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"z\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"l\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"proof\",\"type\":\"bytes32[]\",\"internalType\":\"bytes32[]\"}],\"outputs\":[{\"name\":\"signed\",\"type\":\"bool\",\"internalType\":\"bool\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"signShareWithCallback\",\"inputs\":[{\"name\":\"sid\",\"type\":\"bytes32\",\"internalType\":\"FROSTSignatureId.T\"},{\"name\":\"selection\",\"type\":\"tuple\",\"internalType\":\"structFROSTCoordinator.SignSelection\",\"components\":[{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"root\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"}]},{\"name\":\"share\",\"type\":\"tuple\",\"internalType\":\"structFROST.SignatureShare\",\"components\":[{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"z\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"l\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"proof\",\"type\":\"bytes32[]\",\"internalType\":\"bytes32[]\"},{\"name\":\"callback\",\"type\":\"tuple\",\"internalType\":\"structFROSTCoordinator.Callback\",\"components\":[{\"name\":\"target\",\"type\":\"address\",\"internalType\":\"contractIFROSTCoordinatorCallback\"},{\"name\":\"context\",\"type\":\"bytes\",\"internalType\":\"bytes\"}]}],\"outputs\":[{\"name\":\"signed\",\"type\":\"bool\",\"internalType\":\"bool\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"signatureValue\",\"inputs\":[{\"name\":\"sid\",\"type\":\"bytes32\",\"internalType\":\"FROSTSignatureId.T\"}],\"outputs\":[{\"name\":\"result\",\"type\":\"tuple\",\"internalType\":\"structFROST.Signature\",\"components\":[{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"z\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"signatureVerify\",\"inputs\":[{\"name\":\"sid\",\"type\":\"bytes32\",\"internalType\":\"FROSTSignatureId.T\"},{\"name\":\"gid\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"message\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"}],\"outputs\":[{\"name\":\"result\",\"type\":\"tuple\",\"internalType\":\"structFROST.Signature\",\"components\":[{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"z\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"stateMutability\":\"view\"},{\"type\":\"event\",\"name\":\"KeyGen\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"participants\",\"type\":\"bytes32\",\"indexed\":false,\"internalType\":\"bytes32\"},{\"name\":\"count\",\"type\":\"uint16\",\"indexed\":false,\"internalType\":\"uint16\"},{\"name\":\"threshold\",\"type\":\"uint16\",\"indexed\":false,\"internalType\":\"uint16\"},{\"name\":\"context\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"bytes32\"}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"KeyGenCommitted\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"participant\",\"type\":\"address\",\"indexed\":false,\"internalType\":\"address\"},{\"name\":\"commitment\",\"type\":\"tuple\",\"indexed\":false,\"internalType\":\"structFROSTCoordinator.KeyGenCommitment\",\"components\":[{\"name\":\"q\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"c\",\"type\":\"tuple[]\",\"internalType\":\"structSecp256k1.Point[]\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"mu\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"committed\",\"type\":\"bool\",\"indexed\":false,\"internalType\":\"bool\"}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"KeyGenComplained\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"plaintiff\",\"type\":\"address\",\"indexed\":false,\"internalType\":\"address\"},{\"name\":\"accused\",\"type\":\"address\",\"indexed\":false,\"internalType\":\"address\"},{\"name\":\"compromised\",\"type\":\"bool\",\"indexed\":false,\"internalType\":\"bool\"}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"KeyGenComplaintResponded\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"plaintiff\",\"type\":\"address\",\"indexed\":false,\"internalType\":\"address\"},{\"name\":\"accused\",\"type\":\"address\",\"indexed\":false,\"internalType\":\"address\"},{\"name\":\"secretShare\",\"type\":\"uint256\",\"indexed\":false,\"internalType\":\"uint256\"}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"KeyGenConfirmed\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"participant\",\"type\":\"address\",\"indexed\":false,\"internalType\":\"address\"},{\"name\":\"confirmed\",\"type\":\"bool\",\"indexed\":false,\"internalType\":\"bool\"}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"KeyGenSecretShared\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"participant\",\"type\":\"address\",\"indexed\":false,\"internalType\":\"address\"},{\"name\":\"share\",\"type\":\"tuple\",\"indexed\":false,\"internalType\":\"structFROSTCoordinator.KeyGenSecretShare\",\"components\":[{\"name\":\"y\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"f\",\"type\":\"uint256[]\",\"internalType\":\"uint256[]\"}]},{\"name\":\"shared\",\"type\":\"bool\",\"indexed\":false,\"internalType\":\"bool\"}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"Preprocess\",\"inputs\":[{\"name\":\"gid\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"participant\",\"type\":\"address\",\"indexed\":false,\"internalType\":\"address\"},{\"name\":\"chunk\",\"type\":\"uint64\",\"indexed\":false,\"internalType\":\"uint64\"},{\"name\":\"commitment\",\"type\":\"bytes32\",\"indexed\":false,\"internalType\":\"bytes32\"}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"Sign\",\"inputs\":[{\"name\":\"initiator\",\"type\":\"address\",\"indexed\":true,\"internalType\":\"address\"},{\"name\":\"gid\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"message\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"bytes32\"},{\"name\":\"sid\",\"type\":\"bytes32\",\"indexed\":false,\"internalType\":\"FROSTSignatureId.T\"},{\"name\":\"sequence\",\"type\":\"uint64\",\"indexed\":false,\"internalType\":\"uint64\"}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"SignCompleted\",\"inputs\":[{\"name\":\"sid\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"FROSTSignatureId.T\"},{\"name\":\"selectionRoot\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"bytes32\"},{\"name\":\"signature\",\"type\":\"tuple\",\"indexed\":false,\"internalType\":\"structFROST.Signature\",\"components\":[{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"z\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"SignRevealedNonces\",\"inputs\":[{\"name\":\"sid\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"FROSTSignatureId.T\"},{\"name\":\"participant\",\"type\":\"address\",\"indexed\":false,\"internalType\":\"address\"},{\"name\":\"nonces\",\"type\":\"tuple\",\"indexed\":false,\"internalType\":\"structFROSTCoordinator.SignNonces\",\"components\":[{\"name\":\"d\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"e\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}]}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"SignShared\",\"inputs\":[{\"name\":\"sid\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"FROSTSignatureId.T\"},{\"name\":\"selectionRoot\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"bytes32\"},{\"name\":\"participant\",\"type\":\"address\",\"indexed\":false,\"internalType\":\"address\"},{\"name\":\"z\",\"type\":\"uint256\",\"indexed\":false,\"internalType\":\"uint256\"}],\"anonymous\":false},{\"type\":\"error\",\"name\":\"AlreadyComplained\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"AlreadyIncluded\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"AlreadyInitialized\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"AlreadySet\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"GroupNotInitialized\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"GroupNotReady\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"InvalidGroupCommitment\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"InvalidGroupParameters\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"InvalidMessage\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"InvalidMulMulAddWitness\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"InvalidParticipant\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"InvalidRootHash\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"InvalidScalar\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"InvalidSecretShare\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"NotComplaining\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"NotIncluded\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"NotIncluded\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"NotOnCurve\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"NotParticipating\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"NotSigned\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"NotSigning\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"UnrespondedComplaints\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"WrongSignature\",\"inputs\":[]}]",
}

// FROSTCoordinatorABI is the input ABI used to generate the binding from.
// Deprecated: Use FROSTCoordinatorMetaData.ABI instead.
var FROSTCoordinatorABI = FROSTCoordinatorMetaData.ABI

// FROSTCoordinator is an auto generated Go binding around an Ethereum contract.
type FROSTCoordinator struct {
	FROSTCoordinatorCaller     // Read-only binding to the contract
	FROSTCoordinatorTransactor // Write-only binding to the contract
	FROSTCoordinatorFilterer   // Log filterer for contract events
}

// FROSTCoordinatorCaller is an auto generated read-only Go binding around an Ethereum contract.
type FROSTCoordinatorCaller struct {
	contract *bind.BoundContract // Generic contract wrapper for the low level calls
}

// FROSTCoordinatorTransactor is an auto generated write-only Go binding around an Ethereum contract.
type FROSTCoordinatorTransactor struct {
	contract *bind.BoundContract // Generic contract wrapper for the low level calls
}

// FROSTCoordinatorFilterer is an auto generated log filtering Go binding around an Ethereum contract events.
type FROSTCoordinatorFilterer struct {
	contract *bind.BoundContract // Generic contract wrapper for the low level calls
}

// FROSTCoordinatorSession is an auto generated Go binding around an Ethereum contract,
// with pre-set call and transact options.
type FROSTCoordinatorSession struct {
	Contract     *FROSTCoordinator // Generic contract binding to set the session for
	CallOpts     bind.CallOpts     // Call options to use throughout this session
	TransactOpts bind.TransactOpts // Transaction auth options to use throughout this session
}

// FROSTCoordinatorCallerSession is an auto generated read-only Go binding around an Ethereum contract,
// with pre-set call options.
type FROSTCoordinatorCallerSession struct {
	Contract *FROSTCoordinatorCaller // Generic contract caller binding to set the session for
	CallOpts bind.CallOpts           // Call options to use throughout this session
}

// FROSTCoordinatorTransactorSession is an auto generated write-only Go binding around an Ethereum contract,
// with pre-set transact options.
type FROSTCoordinatorTransactorSession struct {
	Contract     *FROSTCoordinatorTransactor // Generic contract transactor binding to set the session for
	TransactOpts bind.TransactOpts           // Transaction auth options to use throughout this session
}

// FROSTCoordinatorRaw is an auto generated low-level Go binding around an Ethereum contract.
type FROSTCoordinatorRaw struct {
	Contract *FROSTCoordinator // Generic contract binding to access the raw methods on
}

// FROSTCoordinatorCallerRaw is an auto generated low-level read-only Go binding around an Ethereum contract.
type FROSTCoordinatorCallerRaw struct {
	Contract *FROSTCoordinatorCaller // Generic read-only contract binding to access the raw methods on
}

// FROSTCoordinatorTransactorRaw is an auto generated low-level write-only Go binding around an Ethereum contract.
type FROSTCoordinatorTransactorRaw struct {
	Contract *FROSTCoordinatorTransactor // Generic write-only contract binding to access the raw methods on
}

// NewFROSTCoordinator creates a new instance of FROSTCoordinator, bound to a specific deployed contract.
func NewFROSTCoordinator(address common.Address, backend bind.ContractBackend) (*FROSTCoordinator, error) {
	contract, err := bindFROSTCoordinator(address, backend, backend, backend)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinator{FROSTCoordinatorCaller: FROSTCoordinatorCaller{contract: contract}, FROSTCoordinatorTransactor: FROSTCoordinatorTransactor{contract: contract}, FROSTCoordinatorFilterer: FROSTCoordinatorFilterer{contract: contract}}, nil
}

// NewFROSTCoordinatorCaller creates a new read-only instance of FROSTCoordinator, bound to a specific deployed contract.
func NewFROSTCoordinatorCaller(address common.Address, caller bind.ContractCaller) (*FROSTCoordinatorCaller, error) {
	contract, err := bindFROSTCoordinator(address, caller, nil, nil)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinatorCaller{contract: contract}, nil
}

// NewFROSTCoordinatorTransactor creates a new write-only instance of FROSTCoordinator, bound to a specific deployed contract.
func NewFROSTCoordinatorTransactor(address common.Address, transactor bind.ContractTransactor) (*FROSTCoordinatorTransactor, error) {
	contract, err := bindFROSTCoordinator(address, nil, transactor, nil)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinatorTransactor{contract: contract}, nil
}

// NewFROSTCoordinatorFilterer creates a new log filterer instance of FROSTCoordinator, bound to a specific deployed contract.
func NewFROSTCoordinatorFilterer(address common.Address, filterer bind.ContractFilterer) (*FROSTCoordinatorFilterer, error) {
	contract, err := bindFROSTCoordinator(address, nil, nil, filterer)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinatorFilterer{contract: contract}, nil
}

// bindFROSTCoordinator binds a generic wrapper to an already deployed contract.
func bindFROSTCoordinator(address common.Address, caller bind.ContractCaller, transactor bind.ContractTransactor, filterer bind.ContractFilterer) (*bind.BoundContract, error) {
	parsed, err := FROSTCoordinatorMetaData.GetAbi()
	if err != nil {
		return nil, err
	}
	return bind.NewBoundContract(address, *parsed, caller, transactor, filterer), nil
}

// Call invokes the (constant) contract method with params as input values and
// sets the output to result. The result type might be a single field for simple
// returns, a slice of interfaces for anonymous returns and a struct for named
// returns.
func (_FROSTCoordinator *FROSTCoordinatorRaw) Call(opts *bind.CallOpts, result *[]interface{}, method string, params ...interface{}) error {
	return _FROSTCoordinator.Contract.FROSTCoordinatorCaller.contract.Call(opts, result, method, params...)
}

// Transfer initiates a plain transaction to move funds to the contract, calling
// its default method if one is available.
func (_FROSTCoordinator *FROSTCoordinatorRaw) Transfer(opts *bind.TransactOpts) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.FROSTCoordinatorTransactor.contract.Transfer(opts)
}

// Transact invokes the (paid) contract method with params as input values.
func (_FROSTCoordinator *FROSTCoordinatorRaw) Transact(opts *bind.TransactOpts, method string, params ...interface{}) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.FROSTCoordinatorTransactor.contract.Transact(opts, method, params...)
}

// Call invokes the (constant) contract method with params as input values and
// sets the output to result. The result type might be a single field for simple
// returns, a slice of interfaces for anonymous returns and a struct for named
// returns.
func (_FROSTCoordinator *FROSTCoordinatorCallerRaw) Call(opts *bind.CallOpts, result *[]interface{}, method string, params ...interface{}) error {
	return _FROSTCoordinator.Contract.contract.Call(opts, result, method, params...)
}

// Transfer initiates a plain transaction to move funds to the contract, calling
// its default method if one is available.
func (_FROSTCoordinator *FROSTCoordinatorTransactorRaw) Transfer(opts *bind.TransactOpts) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.contract.Transfer(opts)
}

// Transact invokes the (paid) contract method with params as input values.
func (_FROSTCoordinator *FROSTCoordinatorTransactorRaw) Transact(opts *bind.TransactOpts, method string, params ...interface{}) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.contract.Transact(opts, method, params...)
}

// GroupKey is a free data retrieval call binding the contract method 0x27a7dae0.
//
// Solidity: function groupKey(bytes32 gid) view returns((uint256,uint256) key)
func (_FROSTCoordinator *FROSTCoordinatorCaller) GroupKey(opts *bind.CallOpts, gid [32]byte) (Secp256k1Point, error) {
	var out []interface{}
	err := _FROSTCoordinator.contract.Call(opts, &out, "groupKey", gid)

	if err != nil {
		return *new(Secp256k1Point), err
	}

	out0 := *abi.ConvertType(out[0], new(Secp256k1Point)).(*Secp256k1Point)

	return out0, err

}

// GroupKey is a free data retrieval call binding the contract method 0x27a7dae0.
//
// Solidity: function groupKey(bytes32 gid) view returns((uint256,uint256) key)
func (_FROSTCoordinator *FROSTCoordinatorSession) GroupKey(gid [32]byte) (Secp256k1Point, error) {
	return _FROSTCoordinator.Contract.GroupKey(&_FROSTCoordinator.CallOpts, gid)
}

// GroupKey is a free data retrieval call binding the contract method 0x27a7dae0.
//
// Solidity: function groupKey(bytes32 gid) view returns((uint256,uint256) key)
func (_FROSTCoordinator *FROSTCoordinatorCallerSession) GroupKey(gid [32]byte) (Secp256k1Point, error) {
	return _FROSTCoordinator.Contract.GroupKey(&_FROSTCoordinator.CallOpts, gid)
}

// GroupParameters is a free data retrieval call binding the contract method 0x87bf093f.
//
// Solidity: function groupParameters(bytes32 gid) view returns(bytes32 participants, uint16 count, uint16 threshold)
func (_FROSTCoordinator *FROSTCoordinatorCaller) GroupParameters(opts *bind.CallOpts, gid [32]byte) (struct {
	Participants [32]byte
	Count        uint16
	Threshold    uint16
}, error) {
	var out []interface{}
	err := _FROSTCoordinator.contract.Call(opts, &out, "groupParameters", gid)

	outstruct := new(struct {
		Participants [32]byte
		Count        uint16
		Threshold    uint16
	})
	if err != nil {
		return *outstruct, err
	}

	outstruct.Participants = *abi.ConvertType(out[0], new([32]byte)).(*[32]byte)
	outstruct.Count = *abi.ConvertType(out[1], new(uint16)).(*uint16)
	outstruct.Threshold = *abi.ConvertType(out[2], new(uint16)).(*uint16)

	return *outstruct, err

}

// GroupParameters is a free data retrieval call binding the contract method 0x87bf093f.
//
// Solidity: function groupParameters(bytes32 gid) view returns(bytes32 participants, uint16 count, uint16 threshold)
func (_FROSTCoordinator *FROSTCoordinatorSession) GroupParameters(gid [32]byte) (struct {
	Participants [32]byte
	Count        uint16
	Threshold    uint16
}, error) {
	return _FROSTCoordinator.Contract.GroupParameters(&_FROSTCoordinator.CallOpts, gid)
}

// GroupParameters is a free data retrieval call binding the contract method 0x87bf093f.
//
// Solidity: function groupParameters(bytes32 gid) view returns(bytes32 participants, uint16 count, uint16 threshold)
func (_FROSTCoordinator *FROSTCoordinatorCallerSession) GroupParameters(gid [32]byte) (struct {
	Participants [32]byte
	Count        uint16
	Threshold    uint16
}, error) {
	return _FROSTCoordinator.Contract.GroupParameters(&_FROSTCoordinator.CallOpts, gid)
}

// GroupSignCount is a free data retrieval call binding the contract method 0x92d1c76d.
//
// Solidity: function groupSignCount(bytes32 gid) view returns(uint256 result)
func (_FROSTCoordinator *FROSTCoordinatorCaller) GroupSignCount(opts *bind.CallOpts, gid [32]byte) (*big.Int, error) {
	var out []interface{}
	err := _FROSTCoordinator.contract.Call(opts, &out, "groupSignCount", gid)

	if err != nil {
		return *new(*big.Int), err
	}

	out0 := *abi.ConvertType(out[0], new(*big.Int)).(**big.Int)

	return out0, err

}

// GroupSignCount is a free data retrieval call binding the contract method 0x92d1c76d.
//
// Solidity: function groupSignCount(bytes32 gid) view returns(uint256 result)
func (_FROSTCoordinator *FROSTCoordinatorSession) GroupSignCount(gid [32]byte) (*big.Int, error) {
	return _FROSTCoordinator.Contract.GroupSignCount(&_FROSTCoordinator.CallOpts, gid)
}

// GroupSignCount is a free data retrieval call binding the contract method 0x92d1c76d.
//
// Solidity: function groupSignCount(bytes32 gid) view returns(uint256 result)
func (_FROSTCoordinator *FROSTCoordinatorCallerSession) GroupSignCount(gid [32]byte) (*big.Int, error) {
	return _FROSTCoordinator.Contract.GroupSignCount(&_FROSTCoordinator.CallOpts, gid)
}

// ParticipantKey is a free data retrieval call binding the contract method 0xf0bda61f.
//
// Solidity: function participantKey(bytes32 gid, address participant) view returns((uint256,uint256) key)
func (_FROSTCoordinator *FROSTCoordinatorCaller) ParticipantKey(opts *bind.CallOpts, gid [32]byte, participant common.Address) (Secp256k1Point, error) {
	var out []interface{}
	err := _FROSTCoordinator.contract.Call(opts, &out, "participantKey", gid, participant)

	if err != nil {
		return *new(Secp256k1Point), err
	}

	out0 := *abi.ConvertType(out[0], new(Secp256k1Point)).(*Secp256k1Point)

	return out0, err

}

// ParticipantKey is a free data retrieval call binding the contract method 0xf0bda61f.
//
// Solidity: function participantKey(bytes32 gid, address participant) view returns((uint256,uint256) key)
func (_FROSTCoordinator *FROSTCoordinatorSession) ParticipantKey(gid [32]byte, participant common.Address) (Secp256k1Point, error) {
	return _FROSTCoordinator.Contract.ParticipantKey(&_FROSTCoordinator.CallOpts, gid, participant)
}

// ParticipantKey is a free data retrieval call binding the contract method 0xf0bda61f.
//
// Solidity: function participantKey(bytes32 gid, address participant) view returns((uint256,uint256) key)
func (_FROSTCoordinator *FROSTCoordinatorCallerSession) ParticipantKey(gid [32]byte, participant common.Address) (Secp256k1Point, error) {
	return _FROSTCoordinator.Contract.ParticipantKey(&_FROSTCoordinator.CallOpts, gid, participant)
}

// SignatureValue is a free data retrieval call binding the contract method 0x5586fc0b.
//
// Solidity: function signatureValue(bytes32 sid) view returns(((uint256,uint256),uint256) result)
func (_FROSTCoordinator *FROSTCoordinatorCaller) SignatureValue(opts *bind.CallOpts, sid [32]byte) (FROSTSignature, error) {
	var out []interface{}
	err := _FROSTCoordinator.contract.Call(opts, &out, "signatureValue", sid)

	if err != nil {
		return *new(FROSTSignature), err
	}

	out0 := *abi.ConvertType(out[0], new(FROSTSignature)).(*FROSTSignature)

	return out0, err

}

// SignatureValue is a free data retrieval call binding the contract method 0x5586fc0b.
//
// Solidity: function signatureValue(bytes32 sid) view returns(((uint256,uint256),uint256) result)
func (_FROSTCoordinator *FROSTCoordinatorSession) SignatureValue(sid [32]byte) (FROSTSignature, error) {
	return _FROSTCoordinator.Contract.SignatureValue(&_FROSTCoordinator.CallOpts, sid)
}

// SignatureValue is a free data retrieval call binding the contract method 0x5586fc0b.
//
// Solidity: function signatureValue(bytes32 sid) view returns(((uint256,uint256),uint256) result)
func (_FROSTCoordinator *FROSTCoordinatorCallerSession) SignatureValue(sid [32]byte) (FROSTSignature, error) {
	return _FROSTCoordinator.Contract.SignatureValue(&_FROSTCoordinator.CallOpts, sid)
}

// SignatureVerify is a free data retrieval call binding the contract method 0x1dd921ed.
//
// Solidity: function signatureVerify(bytes32 sid, bytes32 gid, bytes32 message) view returns(((uint256,uint256),uint256) result)
func (_FROSTCoordinator *FROSTCoordinatorCaller) SignatureVerify(opts *bind.CallOpts, sid [32]byte, gid [32]byte, message [32]byte) (FROSTSignature, error) {
	var out []interface{}
	err := _FROSTCoordinator.contract.Call(opts, &out, "signatureVerify", sid, gid, message)

	if err != nil {
		return *new(FROSTSignature), err
	}

	out0 := *abi.ConvertType(out[0], new(FROSTSignature)).(*FROSTSignature)

	return out0, err

}

// SignatureVerify is a free data retrieval call binding the contract method 0x1dd921ed.
//
// Solidity: function signatureVerify(bytes32 sid, bytes32 gid, bytes32 message) view returns(((uint256,uint256),uint256) result)
func (_FROSTCoordinator *FROSTCoordinatorSession) SignatureVerify(sid [32]byte, gid [32]byte, message [32]byte) (FROSTSignature, error) {
	return _FROSTCoordinator.Contract.SignatureVerify(&_FROSTCoordinator.CallOpts, sid, gid, message)
}

// SignatureVerify is a free data retrieval call binding the contract method 0x1dd921ed.
//
// Solidity: function signatureVerify(bytes32 sid, bytes32 gid, bytes32 message) view returns(((uint256,uint256),uint256) result)
func (_FROSTCoordinator *FROSTCoordinatorCallerSession) SignatureVerify(sid [32]byte, gid [32]byte, message [32]byte) (FROSTSignature, error) {
	return _FROSTCoordinator.Contract.SignatureVerify(&_FROSTCoordinator.CallOpts, sid, gid, message)
}

// KeyGen is a paid mutator transaction binding the contract method 0x4062cb31.
//
// Solidity: function keyGen(bytes32 participants, uint16 count, uint16 threshold, bytes32 context) returns(bytes32 gid)
func (_FROSTCoordinator *FROSTCoordinatorTransactor) KeyGen(opts *bind.TransactOpts, participants [32]byte, count uint16, threshold uint16, context [32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.contract.Transact(opts, "keyGen", participants, count, threshold, context)
}

// KeyGen is a paid mutator transaction binding the contract method 0x4062cb31.
//
// Solidity: function keyGen(bytes32 participants, uint16 count, uint16 threshold, bytes32 context) returns(bytes32 gid)
func (_FROSTCoordinator *FROSTCoordinatorSession) KeyGen(participants [32]byte, count uint16, threshold uint16, context [32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGen(&_FROSTCoordinator.TransactOpts, participants, count, threshold, context)
}

// KeyGen is a paid mutator transaction binding the contract method 0x4062cb31.
//
// Solidity: function keyGen(bytes32 participants, uint16 count, uint16 threshold, bytes32 context) returns(bytes32 gid)
func (_FROSTCoordinator *FROSTCoordinatorTransactorSession) KeyGen(participants [32]byte, count uint16, threshold uint16, context [32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGen(&_FROSTCoordinator.TransactOpts, participants, count, threshold, context)
}

// KeyGenAndCommit is a paid mutator transaction binding the contract method 0x38b54463.
//
// Solidity: function keyGenAndCommit(bytes32 participants, uint16 count, uint16 threshold, bytes32 context, bytes32[] poap, ((uint256,uint256),(uint256,uint256)[],(uint256,uint256),uint256) commitment) returns(bytes32 gid, bool committed)
func (_FROSTCoordinator *FROSTCoordinatorTransactor) KeyGenAndCommit(opts *bind.TransactOpts, participants [32]byte, count uint16, threshold uint16, context [32]byte, poap [][32]byte, commitment FROSTCoordinatorKeyGenCommitment) (*types.Transaction, error) {
	return _FROSTCoordinator.contract.Transact(opts, "keyGenAndCommit", participants, count, threshold, context, poap, commitment)
}

// KeyGenAndCommit is a paid mutator transaction binding the contract method 0x38b54463.
//
// Solidity: function keyGenAndCommit(bytes32 participants, uint16 count, uint16 threshold, bytes32 context, bytes32[] poap, ((uint256,uint256),(uint256,uint256)[],(uint256,uint256),uint256) commitment) returns(bytes32 gid, bool committed)
func (_FROSTCoordinator *FROSTCoordinatorSession) KeyGenAndCommit(participants [32]byte, count uint16, threshold uint16, context [32]byte, poap [][32]byte, commitment FROSTCoordinatorKeyGenCommitment) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGenAndCommit(&_FROSTCoordinator.TransactOpts, participants, count, threshold, context, poap, commitment)
}

// KeyGenAndCommit is a paid mutator transaction binding the contract method 0x38b54463.
//
// Solidity: function keyGenAndCommit(bytes32 participants, uint16 count, uint16 threshold, bytes32 context, bytes32[] poap, ((uint256,uint256),(uint256,uint256)[],(uint256,uint256),uint256) commitment) returns(bytes32 gid, bool committed)
func (_FROSTCoordinator *FROSTCoordinatorTransactorSession) KeyGenAndCommit(participants [32]byte, count uint16, threshold uint16, context [32]byte, poap [][32]byte, commitment FROSTCoordinatorKeyGenCommitment) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGenAndCommit(&_FROSTCoordinator.TransactOpts, participants, count, threshold, context, poap, commitment)
}

// KeyGenCommit is a paid mutator transaction binding the contract method 0x158adc9f.
//
// Solidity: function keyGenCommit(bytes32 gid, bytes32[] poap, ((uint256,uint256),(uint256,uint256)[],(uint256,uint256),uint256) commitment) returns(bool committed)
func (_FROSTCoordinator *FROSTCoordinatorTransactor) KeyGenCommit(opts *bind.TransactOpts, gid [32]byte, poap [][32]byte, commitment FROSTCoordinatorKeyGenCommitment) (*types.Transaction, error) {
	return _FROSTCoordinator.contract.Transact(opts, "keyGenCommit", gid, poap, commitment)
}

// KeyGenCommit is a paid mutator transaction binding the contract method 0x158adc9f.
//
// Solidity: function keyGenCommit(bytes32 gid, bytes32[] poap, ((uint256,uint256),(uint256,uint256)[],(uint256,uint256),uint256) commitment) returns(bool committed)
func (_FROSTCoordinator *FROSTCoordinatorSession) KeyGenCommit(gid [32]byte, poap [][32]byte, commitment FROSTCoordinatorKeyGenCommitment) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGenCommit(&_FROSTCoordinator.TransactOpts, gid, poap, commitment)
}

// KeyGenCommit is a paid mutator transaction binding the contract method 0x158adc9f.
//
// Solidity: function keyGenCommit(bytes32 gid, bytes32[] poap, ((uint256,uint256),(uint256,uint256)[],(uint256,uint256),uint256) commitment) returns(bool committed)
func (_FROSTCoordinator *FROSTCoordinatorTransactorSession) KeyGenCommit(gid [32]byte, poap [][32]byte, commitment FROSTCoordinatorKeyGenCommitment) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGenCommit(&_FROSTCoordinator.TransactOpts, gid, poap, commitment)
}

// KeyGenComplain is a paid mutator transaction binding the contract method 0x2f559b6d.
//
// Solidity: function keyGenComplain(bytes32 gid, address accused) returns(bool compromised)
func (_FROSTCoordinator *FROSTCoordinatorTransactor) KeyGenComplain(opts *bind.TransactOpts, gid [32]byte, accused common.Address) (*types.Transaction, error) {
	return _FROSTCoordinator.contract.Transact(opts, "keyGenComplain", gid, accused)
}

// KeyGenComplain is a paid mutator transaction binding the contract method 0x2f559b6d.
//
// Solidity: function keyGenComplain(bytes32 gid, address accused) returns(bool compromised)
func (_FROSTCoordinator *FROSTCoordinatorSession) KeyGenComplain(gid [32]byte, accused common.Address) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGenComplain(&_FROSTCoordinator.TransactOpts, gid, accused)
}

// KeyGenComplain is a paid mutator transaction binding the contract method 0x2f559b6d.
//
// Solidity: function keyGenComplain(bytes32 gid, address accused) returns(bool compromised)
func (_FROSTCoordinator *FROSTCoordinatorTransactorSession) KeyGenComplain(gid [32]byte, accused common.Address) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGenComplain(&_FROSTCoordinator.TransactOpts, gid, accused)
}

// KeyGenComplaintResponse is a paid mutator transaction binding the contract method 0x53a8081a.
//
// Solidity: function keyGenComplaintResponse(bytes32 gid, address plaintiff, uint256 secretShare) returns()
func (_FROSTCoordinator *FROSTCoordinatorTransactor) KeyGenComplaintResponse(opts *bind.TransactOpts, gid [32]byte, plaintiff common.Address, secretShare *big.Int) (*types.Transaction, error) {
	return _FROSTCoordinator.contract.Transact(opts, "keyGenComplaintResponse", gid, plaintiff, secretShare)
}

// KeyGenComplaintResponse is a paid mutator transaction binding the contract method 0x53a8081a.
//
// Solidity: function keyGenComplaintResponse(bytes32 gid, address plaintiff, uint256 secretShare) returns()
func (_FROSTCoordinator *FROSTCoordinatorSession) KeyGenComplaintResponse(gid [32]byte, plaintiff common.Address, secretShare *big.Int) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGenComplaintResponse(&_FROSTCoordinator.TransactOpts, gid, plaintiff, secretShare)
}

// KeyGenComplaintResponse is a paid mutator transaction binding the contract method 0x53a8081a.
//
// Solidity: function keyGenComplaintResponse(bytes32 gid, address plaintiff, uint256 secretShare) returns()
func (_FROSTCoordinator *FROSTCoordinatorTransactorSession) KeyGenComplaintResponse(gid [32]byte, plaintiff common.Address, secretShare *big.Int) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGenComplaintResponse(&_FROSTCoordinator.TransactOpts, gid, plaintiff, secretShare)
}

// KeyGenConfirm is a paid mutator transaction binding the contract method 0x1169f60e.
//
// Solidity: function keyGenConfirm(bytes32 gid) returns(bool confirmed)
func (_FROSTCoordinator *FROSTCoordinatorTransactor) KeyGenConfirm(opts *bind.TransactOpts, gid [32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.contract.Transact(opts, "keyGenConfirm", gid)
}

// KeyGenConfirm is a paid mutator transaction binding the contract method 0x1169f60e.
//
// Solidity: function keyGenConfirm(bytes32 gid) returns(bool confirmed)
func (_FROSTCoordinator *FROSTCoordinatorSession) KeyGenConfirm(gid [32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGenConfirm(&_FROSTCoordinator.TransactOpts, gid)
}

// KeyGenConfirm is a paid mutator transaction binding the contract method 0x1169f60e.
//
// Solidity: function keyGenConfirm(bytes32 gid) returns(bool confirmed)
func (_FROSTCoordinator *FROSTCoordinatorTransactorSession) KeyGenConfirm(gid [32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGenConfirm(&_FROSTCoordinator.TransactOpts, gid)
}

// KeyGenConfirmWithCallback is a paid mutator transaction binding the contract method 0x1896ae36.
//
// Solidity: function keyGenConfirmWithCallback(bytes32 gid, (address,bytes) callback) returns(bool confirmed)
func (_FROSTCoordinator *FROSTCoordinatorTransactor) KeyGenConfirmWithCallback(opts *bind.TransactOpts, gid [32]byte, callback FROSTCoordinatorCallback) (*types.Transaction, error) {
	return _FROSTCoordinator.contract.Transact(opts, "keyGenConfirmWithCallback", gid, callback)
}

// KeyGenConfirmWithCallback is a paid mutator transaction binding the contract method 0x1896ae36.
//
// Solidity: function keyGenConfirmWithCallback(bytes32 gid, (address,bytes) callback) returns(bool confirmed)
func (_FROSTCoordinator *FROSTCoordinatorSession) KeyGenConfirmWithCallback(gid [32]byte, callback FROSTCoordinatorCallback) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGenConfirmWithCallback(&_FROSTCoordinator.TransactOpts, gid, callback)
}

// KeyGenConfirmWithCallback is a paid mutator transaction binding the contract method 0x1896ae36.
//
// Solidity: function keyGenConfirmWithCallback(bytes32 gid, (address,bytes) callback) returns(bool confirmed)
func (_FROSTCoordinator *FROSTCoordinatorTransactorSession) KeyGenConfirmWithCallback(gid [32]byte, callback FROSTCoordinatorCallback) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGenConfirmWithCallback(&_FROSTCoordinator.TransactOpts, gid, callback)
}

// KeyGenSecretShare is a paid mutator transaction binding the contract method 0x7d10c04b.
//
// Solidity: function keyGenSecretShare(bytes32 gid, ((uint256,uint256),uint256[]) share) returns(bool shared)
func (_FROSTCoordinator *FROSTCoordinatorTransactor) KeyGenSecretShare(opts *bind.TransactOpts, gid [32]byte, share FROSTCoordinatorKeyGenSecretShare) (*types.Transaction, error) {
	return _FROSTCoordinator.contract.Transact(opts, "keyGenSecretShare", gid, share)
}

// KeyGenSecretShare is a paid mutator transaction binding the contract method 0x7d10c04b.
//
// Solidity: function keyGenSecretShare(bytes32 gid, ((uint256,uint256),uint256[]) share) returns(bool shared)
func (_FROSTCoordinator *FROSTCoordinatorSession) KeyGenSecretShare(gid [32]byte, share FROSTCoordinatorKeyGenSecretShare) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGenSecretShare(&_FROSTCoordinator.TransactOpts, gid, share)
}

// KeyGenSecretShare is a paid mutator transaction binding the contract method 0x7d10c04b.
//
// Solidity: function keyGenSecretShare(bytes32 gid, ((uint256,uint256),uint256[]) share) returns(bool shared)
func (_FROSTCoordinator *FROSTCoordinatorTransactorSession) KeyGenSecretShare(gid [32]byte, share FROSTCoordinatorKeyGenSecretShare) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.KeyGenSecretShare(&_FROSTCoordinator.TransactOpts, gid, share)
}

// Preprocess is a paid mutator transaction binding the contract method 0x42b29c61.
//
// Solidity: function preprocess(bytes32 gid, bytes32 commitment) returns(uint64 chunk)
func (_FROSTCoordinator *FROSTCoordinatorTransactor) Preprocess(opts *bind.TransactOpts, gid [32]byte, commitment [32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.contract.Transact(opts, "preprocess", gid, commitment)
}

// Preprocess is a paid mutator transaction binding the contract method 0x42b29c61.
//
// Solidity: function preprocess(bytes32 gid, bytes32 commitment) returns(uint64 chunk)
func (_FROSTCoordinator *FROSTCoordinatorSession) Preprocess(gid [32]byte, commitment [32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.Preprocess(&_FROSTCoordinator.TransactOpts, gid, commitment)
}

// Preprocess is a paid mutator transaction binding the contract method 0x42b29c61.
//
// Solidity: function preprocess(bytes32 gid, bytes32 commitment) returns(uint64 chunk)
func (_FROSTCoordinator *FROSTCoordinatorTransactorSession) Preprocess(gid [32]byte, commitment [32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.Preprocess(&_FROSTCoordinator.TransactOpts, gid, commitment)
}

// Sign is a paid mutator transaction binding the contract method 0x86f57635.
//
// Solidity: function sign(bytes32 gid, bytes32 message) returns(bytes32 sid)
func (_FROSTCoordinator *FROSTCoordinatorTransactor) Sign(opts *bind.TransactOpts, gid [32]byte, message [32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.contract.Transact(opts, "sign", gid, message)
}

// Sign is a paid mutator transaction binding the contract method 0x86f57635.
//
// Solidity: function sign(bytes32 gid, bytes32 message) returns(bytes32 sid)
func (_FROSTCoordinator *FROSTCoordinatorSession) Sign(gid [32]byte, message [32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.Sign(&_FROSTCoordinator.TransactOpts, gid, message)
}

// Sign is a paid mutator transaction binding the contract method 0x86f57635.
//
// Solidity: function sign(bytes32 gid, bytes32 message) returns(bytes32 sid)
func (_FROSTCoordinator *FROSTCoordinatorTransactorSession) Sign(gid [32]byte, message [32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.Sign(&_FROSTCoordinator.TransactOpts, gid, message)
}

// SignRevealNonces is a paid mutator transaction binding the contract method 0x527bdde9.
//
// Solidity: function signRevealNonces(bytes32 sid, ((uint256,uint256),(uint256,uint256)) nonces, bytes32[] proof) returns()
func (_FROSTCoordinator *FROSTCoordinatorTransactor) SignRevealNonces(opts *bind.TransactOpts, sid [32]byte, nonces FROSTCoordinatorSignNonces, proof [][32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.contract.Transact(opts, "signRevealNonces", sid, nonces, proof)
}

// SignRevealNonces is a paid mutator transaction binding the contract method 0x527bdde9.
//
// Solidity: function signRevealNonces(bytes32 sid, ((uint256,uint256),(uint256,uint256)) nonces, bytes32[] proof) returns()
func (_FROSTCoordinator *FROSTCoordinatorSession) SignRevealNonces(sid [32]byte, nonces FROSTCoordinatorSignNonces, proof [][32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.SignRevealNonces(&_FROSTCoordinator.TransactOpts, sid, nonces, proof)
}

// SignRevealNonces is a paid mutator transaction binding the contract method 0x527bdde9.
//
// Solidity: function signRevealNonces(bytes32 sid, ((uint256,uint256),(uint256,uint256)) nonces, bytes32[] proof) returns()
func (_FROSTCoordinator *FROSTCoordinatorTransactorSession) SignRevealNonces(sid [32]byte, nonces FROSTCoordinatorSignNonces, proof [][32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.SignRevealNonces(&_FROSTCoordinator.TransactOpts, sid, nonces, proof)
}

// SignShare is a paid mutator transaction binding the contract method 0x243e8b83.
//
// Solidity: function signShare(bytes32 sid, ((uint256,uint256),bytes32) selection, ((uint256,uint256),uint256,uint256) share, bytes32[] proof) returns(bool signed)
func (_FROSTCoordinator *FROSTCoordinatorTransactor) SignShare(opts *bind.TransactOpts, sid [32]byte, selection FROSTCoordinatorSignSelection, share FROSTSignatureShare, proof [][32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.contract.Transact(opts, "signShare", sid, selection, share, proof)
}

// SignShare is a paid mutator transaction binding the contract method 0x243e8b83.
//
// Solidity: function signShare(bytes32 sid, ((uint256,uint256),bytes32) selection, ((uint256,uint256),uint256,uint256) share, bytes32[] proof) returns(bool signed)
func (_FROSTCoordinator *FROSTCoordinatorSession) SignShare(sid [32]byte, selection FROSTCoordinatorSignSelection, share FROSTSignatureShare, proof [][32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.SignShare(&_FROSTCoordinator.TransactOpts, sid, selection, share, proof)
}

// SignShare is a paid mutator transaction binding the contract method 0x243e8b83.
//
// Solidity: function signShare(bytes32 sid, ((uint256,uint256),bytes32) selection, ((uint256,uint256),uint256,uint256) share, bytes32[] proof) returns(bool signed)
func (_FROSTCoordinator *FROSTCoordinatorTransactorSession) SignShare(sid [32]byte, selection FROSTCoordinatorSignSelection, share FROSTSignatureShare, proof [][32]byte) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.SignShare(&_FROSTCoordinator.TransactOpts, sid, selection, share, proof)
}

// SignShareWithCallback is a paid mutator transaction binding the contract method 0x95b57d9d.
//
// Solidity: function signShareWithCallback(bytes32 sid, ((uint256,uint256),bytes32) selection, ((uint256,uint256),uint256,uint256) share, bytes32[] proof, (address,bytes) callback) returns(bool signed)
func (_FROSTCoordinator *FROSTCoordinatorTransactor) SignShareWithCallback(opts *bind.TransactOpts, sid [32]byte, selection FROSTCoordinatorSignSelection, share FROSTSignatureShare, proof [][32]byte, callback FROSTCoordinatorCallback) (*types.Transaction, error) {
	return _FROSTCoordinator.contract.Transact(opts, "signShareWithCallback", sid, selection, share, proof, callback)
}

// SignShareWithCallback is a paid mutator transaction binding the contract method 0x95b57d9d.
//
// Solidity: function signShareWithCallback(bytes32 sid, ((uint256,uint256),bytes32) selection, ((uint256,uint256),uint256,uint256) share, bytes32[] proof, (address,bytes) callback) returns(bool signed)
func (_FROSTCoordinator *FROSTCoordinatorSession) SignShareWithCallback(sid [32]byte, selection FROSTCoordinatorSignSelection, share FROSTSignatureShare, proof [][32]byte, callback FROSTCoordinatorCallback) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.SignShareWithCallback(&_FROSTCoordinator.TransactOpts, sid, selection, share, proof, callback)
}

// SignShareWithCallback is a paid mutator transaction binding the contract method 0x95b57d9d.
//
// Solidity: function signShareWithCallback(bytes32 sid, ((uint256,uint256),bytes32) selection, ((uint256,uint256),uint256,uint256) share, bytes32[] proof, (address,bytes) callback) returns(bool signed)
func (_FROSTCoordinator *FROSTCoordinatorTransactorSession) SignShareWithCallback(sid [32]byte, selection FROSTCoordinatorSignSelection, share FROSTSignatureShare, proof [][32]byte, callback FROSTCoordinatorCallback) (*types.Transaction, error) {
	return _FROSTCoordinator.Contract.SignShareWithCallback(&_FROSTCoordinator.TransactOpts, sid, selection, share, proof, callback)
}

// FROSTCoordinatorKeyGenIterator is returned from FilterKeyGen and is used to iterate over the raw logs and unpacked data for KeyGen events raised by the FROSTCoordinator contract.
type FROSTCoordinatorKeyGenIterator struct {
	Event *FROSTCoordinatorKeyGen // Event containing the contract specifics and raw log

	contract *bind.BoundContract // Generic contract to use for unpacking event data
	event    string              // Event name to use for unpacking event data

	logs chan types.Log        // Log channel receiving the found contract events
	sub  ethereum.Subscription // Subscription for errors, completion and termination
	done bool                  // Whether the subscription completed delivering logs
	fail error                 // Occurred error to stop iteration
}

// Next advances the iterator to the subsequent event, returning whether there
// are any more events found. In case of a retrieval or parsing error, false is
// returned and Error() can be queried for the exact failure.
func (it *FROSTCoordinatorKeyGenIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(FROSTCoordinatorKeyGen)
			if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
				it.fail = err
				return false
			}
			it.Event.Raw = log
			return true

		default:
			return false
		}
	}
	// Iterator still in progress, wait for either a data or an error event
	select {
	case log := <-it.logs:
		it.Event = new(FROSTCoordinatorKeyGen)
		if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
			it.fail = err
			return false
		}
		it.Event.Raw = log
		return true

	case err := <-it.sub.Err():
		it.done = true
		it.fail = err
		return it.Next()
	}
}

// Error returns any retrieval or parsing error occurred during filtering.
func (it *FROSTCoordinatorKeyGenIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *FROSTCoordinatorKeyGenIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// FROSTCoordinatorKeyGen represents a KeyGen event raised by the FROSTCoordinator contract.
type FROSTCoordinatorKeyGen struct {
	Gid          [32]byte
	Participants [32]byte
	Count        uint16
	Threshold    uint16
	Context      [32]byte
	Raw          types.Log // Blockchain specific contextual infos
}

// FilterKeyGen is a free log retrieval operation binding the contract event 0xb3b10f23809cbdcdb7d4ff0d2cb4e573182d7704ca17ccf8321788a097a35ee7.
//
// Solidity: event KeyGen(bytes32 indexed gid, bytes32 participants, uint16 count, uint16 threshold, bytes32 indexed context)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) FilterKeyGen(opts *bind.FilterOpts, gid [][32]byte, context [][32]byte) (*FROSTCoordinatorKeyGenIterator, error) {

	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}

	var contextRule []interface{}
	for _, contextItem := range context {
		contextRule = append(contextRule, contextItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.FilterLogs(opts, "KeyGen", gidRule, contextRule)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinatorKeyGenIterator{contract: _FROSTCoordinator.contract, event: "KeyGen", logs: logs, sub: sub}, nil
}

// WatchKeyGen is a free log subscription operation binding the contract event 0xb3b10f23809cbdcdb7d4ff0d2cb4e573182d7704ca17ccf8321788a097a35ee7.
//
// Solidity: event KeyGen(bytes32 indexed gid, bytes32 participants, uint16 count, uint16 threshold, bytes32 indexed context)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) WatchKeyGen(opts *bind.WatchOpts, sink chan<- *FROSTCoordinatorKeyGen, gid [][32]byte, context [][32]byte) (event.Subscription, error) {

	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}

	var contextRule []interface{}
	for _, contextItem := range context {
		contextRule = append(contextRule, contextItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.WatchLogs(opts, "KeyGen", gidRule, contextRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(FROSTCoordinatorKeyGen)
				if err := _FROSTCoordinator.contract.UnpackLog(event, "KeyGen", log); err != nil {
					return err
				}
				event.Raw = log

				select {
				case sink <- event:
				case err := <-sub.Err():
					return err
				case <-quit:
					return nil
				}
			case err := <-sub.Err():
				return err
			case <-quit:
				return nil
			}
		}
	}), nil
}

// ParseKeyGen is a log parse operation binding the contract event 0xb3b10f23809cbdcdb7d4ff0d2cb4e573182d7704ca17ccf8321788a097a35ee7.
//
// Solidity: event KeyGen(bytes32 indexed gid, bytes32 participants, uint16 count, uint16 threshold, bytes32 indexed context)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) ParseKeyGen(log types.Log) (*FROSTCoordinatorKeyGen, error) {
	event := new(FROSTCoordinatorKeyGen)
	if err := _FROSTCoordinator.contract.UnpackLog(event, "KeyGen", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// FROSTCoordinatorKeyGenCommittedIterator is returned from FilterKeyGenCommitted and is used to iterate over the raw logs and unpacked data for KeyGenCommitted events raised by the FROSTCoordinator contract.
type FROSTCoordinatorKeyGenCommittedIterator struct {
	Event *FROSTCoordinatorKeyGenCommitted // Event containing the contract specifics and raw log

	contract *bind.BoundContract // Generic contract to use for unpacking event data
	event    string              // Event name to use for unpacking event data

	logs chan types.Log        // Log channel receiving the found contract events
	sub  ethereum.Subscription // Subscription for errors, completion and termination
	done bool                  // Whether the subscription completed delivering logs
	fail error                 // Occurred error to stop iteration
}

// Next advances the iterator to the subsequent event, returning whether there
// are any more events found. In case of a retrieval or parsing error, false is
// returned and Error() can be queried for the exact failure.
func (it *FROSTCoordinatorKeyGenCommittedIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(FROSTCoordinatorKeyGenCommitted)
			if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
				it.fail = err
				return false
			}
			it.Event.Raw = log
			return true

		default:
			return false
		}
	}
	// Iterator still in progress, wait for either a data or an error event
	select {
	case log := <-it.logs:
		it.Event = new(FROSTCoordinatorKeyGenCommitted)
		if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
			it.fail = err
			return false
		}
		it.Event.Raw = log
		return true

	case err := <-it.sub.Err():
		it.done = true
		it.fail = err
		return it.Next()
	}
}

// Error returns any retrieval or parsing error occurred during filtering.
func (it *FROSTCoordinatorKeyGenCommittedIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *FROSTCoordinatorKeyGenCommittedIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// FROSTCoordinatorKeyGenCommitted represents a KeyGenCommitted event raised by the FROSTCoordinator contract.
type FROSTCoordinatorKeyGenCommitted struct {
	Gid         [32]byte
	Participant common.Address
	Commitment  FROSTCoordinatorKeyGenCommitment
	Committed   bool
	Raw         types.Log // Blockchain specific contextual infos
}

// FilterKeyGenCommitted is a free log retrieval operation binding the contract event 0x0070280cc511b50af569112a8d3fb2c711e5c858ce8d13e4f827275259733f3d.
//
// Solidity: event KeyGenCommitted(bytes32 indexed gid, address participant, ((uint256,uint256),(uint256,uint256)[],(uint256,uint256),uint256) commitment, bool committed)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) FilterKeyGenCommitted(opts *bind.FilterOpts, gid [][32]byte) (*FROSTCoordinatorKeyGenCommittedIterator, error) {

	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.FilterLogs(opts, "KeyGenCommitted", gidRule)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinatorKeyGenCommittedIterator{contract: _FROSTCoordinator.contract, event: "KeyGenCommitted", logs: logs, sub: sub}, nil
}

// WatchKeyGenCommitted is a free log subscription operation binding the contract event 0x0070280cc511b50af569112a8d3fb2c711e5c858ce8d13e4f827275259733f3d.
//
// Solidity: event KeyGenCommitted(bytes32 indexed gid, address participant, ((uint256,uint256),(uint256,uint256)[],(uint256,uint256),uint256) commitment, bool committed)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) WatchKeyGenCommitted(opts *bind.WatchOpts, sink chan<- *FROSTCoordinatorKeyGenCommitted, gid [][32]byte) (event.Subscription, error) {

	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.WatchLogs(opts, "KeyGenCommitted", gidRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(FROSTCoordinatorKeyGenCommitted)
				if err := _FROSTCoordinator.contract.UnpackLog(event, "KeyGenCommitted", log); err != nil {
					return err
				}
				event.Raw = log

				select {
				case sink <- event:
				case err := <-sub.Err():
					return err
				case <-quit:
					return nil
				}
			case err := <-sub.Err():
				return err
			case <-quit:
				return nil
			}
		}
	}), nil
}

// ParseKeyGenCommitted is a log parse operation binding the contract event 0x0070280cc511b50af569112a8d3fb2c711e5c858ce8d13e4f827275259733f3d.
//
// Solidity: event KeyGenCommitted(bytes32 indexed gid, address participant, ((uint256,uint256),(uint256,uint256)[],(uint256,uint256),uint256) commitment, bool committed)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) ParseKeyGenCommitted(log types.Log) (*FROSTCoordinatorKeyGenCommitted, error) {
	event := new(FROSTCoordinatorKeyGenCommitted)
	if err := _FROSTCoordinator.contract.UnpackLog(event, "KeyGenCommitted", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// FROSTCoordinatorKeyGenComplainedIterator is returned from FilterKeyGenComplained and is used to iterate over the raw logs and unpacked data for KeyGenComplained events raised by the FROSTCoordinator contract.
type FROSTCoordinatorKeyGenComplainedIterator struct {
	Event *FROSTCoordinatorKeyGenComplained // Event containing the contract specifics and raw log

	contract *bind.BoundContract // Generic contract to use for unpacking event data
	event    string              // Event name to use for unpacking event data

	logs chan types.Log        // Log channel receiving the found contract events
	sub  ethereum.Subscription // Subscription for errors, completion and termination
	done bool                  // Whether the subscription completed delivering logs
	fail error                 // Occurred error to stop iteration
}

// Next advances the iterator to the subsequent event, returning whether there
// are any more events found. In case of a retrieval or parsing error, false is
// returned and Error() can be queried for the exact failure.
func (it *FROSTCoordinatorKeyGenComplainedIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(FROSTCoordinatorKeyGenComplained)
			if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
				it.fail = err
				return false
			}
			it.Event.Raw = log
			return true

		default:
			return false
		}
	}
	// Iterator still in progress, wait for either a data or an error event
	select {
	case log := <-it.logs:
		it.Event = new(FROSTCoordinatorKeyGenComplained)
		if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
			it.fail = err
			return false
		}
		it.Event.Raw = log
		return true

	case err := <-it.sub.Err():
		it.done = true
		it.fail = err
		return it.Next()
	}
}

// Error returns any retrieval or parsing error occurred during filtering.
func (it *FROSTCoordinatorKeyGenComplainedIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *FROSTCoordinatorKeyGenComplainedIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// FROSTCoordinatorKeyGenComplained represents a KeyGenComplained event raised by the FROSTCoordinator contract.
type FROSTCoordinatorKeyGenComplained struct {
	Gid         [32]byte
	Plaintiff   common.Address
	Accused     common.Address
	Compromised bool
	Raw         types.Log // Blockchain specific contextual infos
}

// FilterKeyGenComplained is a free log retrieval operation binding the contract event 0xfacda0c1a23c91046de84f88c9fb4f3cd4360b4ae1b821b127968fa5d9db5fb9.
//
// Solidity: event KeyGenComplained(bytes32 indexed gid, address plaintiff, address accused, bool compromised)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) FilterKeyGenComplained(opts *bind.FilterOpts, gid [][32]byte) (*FROSTCoordinatorKeyGenComplainedIterator, error) {

	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.FilterLogs(opts, "KeyGenComplained", gidRule)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinatorKeyGenComplainedIterator{contract: _FROSTCoordinator.contract, event: "KeyGenComplained", logs: logs, sub: sub}, nil
}

// WatchKeyGenComplained is a free log subscription operation binding the contract event 0xfacda0c1a23c91046de84f88c9fb4f3cd4360b4ae1b821b127968fa5d9db5fb9.
//
// Solidity: event KeyGenComplained(bytes32 indexed gid, address plaintiff, address accused, bool compromised)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) WatchKeyGenComplained(opts *bind.WatchOpts, sink chan<- *FROSTCoordinatorKeyGenComplained, gid [][32]byte) (event.Subscription, error) {

	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.WatchLogs(opts, "KeyGenComplained", gidRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(FROSTCoordinatorKeyGenComplained)
				if err := _FROSTCoordinator.contract.UnpackLog(event, "KeyGenComplained", log); err != nil {
					return err
				}
				event.Raw = log

				select {
				case sink <- event:
				case err := <-sub.Err():
					return err
				case <-quit:
					return nil
				}
			case err := <-sub.Err():
				return err
			case <-quit:
				return nil
			}
		}
	}), nil
}

// ParseKeyGenComplained is a log parse operation binding the contract event 0xfacda0c1a23c91046de84f88c9fb4f3cd4360b4ae1b821b127968fa5d9db5fb9.
//
// Solidity: event KeyGenComplained(bytes32 indexed gid, address plaintiff, address accused, bool compromised)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) ParseKeyGenComplained(log types.Log) (*FROSTCoordinatorKeyGenComplained, error) {
	event := new(FROSTCoordinatorKeyGenComplained)
	if err := _FROSTCoordinator.contract.UnpackLog(event, "KeyGenComplained", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// FROSTCoordinatorKeyGenComplaintRespondedIterator is returned from FilterKeyGenComplaintResponded and is used to iterate over the raw logs and unpacked data for KeyGenComplaintResponded events raised by the FROSTCoordinator contract.
type FROSTCoordinatorKeyGenComplaintRespondedIterator struct {
	Event *FROSTCoordinatorKeyGenComplaintResponded // Event containing the contract specifics and raw log

	contract *bind.BoundContract // Generic contract to use for unpacking event data
	event    string              // Event name to use for unpacking event data

	logs chan types.Log        // Log channel receiving the found contract events
	sub  ethereum.Subscription // Subscription for errors, completion and termination
	done bool                  // Whether the subscription completed delivering logs
	fail error                 // Occurred error to stop iteration
}

// Next advances the iterator to the subsequent event, returning whether there
// are any more events found. In case of a retrieval or parsing error, false is
// returned and Error() can be queried for the exact failure.
func (it *FROSTCoordinatorKeyGenComplaintRespondedIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(FROSTCoordinatorKeyGenComplaintResponded)
			if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
				it.fail = err
				return false
			}
			it.Event.Raw = log
			return true

		default:
			return false
		}
	}
	// Iterator still in progress, wait for either a data or an error event
	select {
	case log := <-it.logs:
		it.Event = new(FROSTCoordinatorKeyGenComplaintResponded)
		if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
			it.fail = err
			return false
		}
		it.Event.Raw = log
		return true

	case err := <-it.sub.Err():
		it.done = true
		it.fail = err
		return it.Next()
	}
}

// Error returns any retrieval or parsing error occurred during filtering.
func (it *FROSTCoordinatorKeyGenComplaintRespondedIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *FROSTCoordinatorKeyGenComplaintRespondedIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// FROSTCoordinatorKeyGenComplaintResponded represents a KeyGenComplaintResponded event raised by the FROSTCoordinator contract.
type FROSTCoordinatorKeyGenComplaintResponded struct {
	Gid         [32]byte
	Plaintiff   common.Address
	Accused     common.Address
	SecretShare *big.Int
	Raw         types.Log // Blockchain specific contextual infos
}

// FilterKeyGenComplaintResponded is a free log retrieval operation binding the contract event 0x752c21192c04ff0d891e5e67e2e34f0411cd5285d216a8068b19842343e9cbb2.
//
// Solidity: event KeyGenComplaintResponded(bytes32 indexed gid, address plaintiff, address accused, uint256 secretShare)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) FilterKeyGenComplaintResponded(opts *bind.FilterOpts, gid [][32]byte) (*FROSTCoordinatorKeyGenComplaintRespondedIterator, error) {

	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.FilterLogs(opts, "KeyGenComplaintResponded", gidRule)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinatorKeyGenComplaintRespondedIterator{contract: _FROSTCoordinator.contract, event: "KeyGenComplaintResponded", logs: logs, sub: sub}, nil
}

// WatchKeyGenComplaintResponded is a free log subscription operation binding the contract event 0x752c21192c04ff0d891e5e67e2e34f0411cd5285d216a8068b19842343e9cbb2.
//
// Solidity: event KeyGenComplaintResponded(bytes32 indexed gid, address plaintiff, address accused, uint256 secretShare)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) WatchKeyGenComplaintResponded(opts *bind.WatchOpts, sink chan<- *FROSTCoordinatorKeyGenComplaintResponded, gid [][32]byte) (event.Subscription, error) {

	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.WatchLogs(opts, "KeyGenComplaintResponded", gidRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(FROSTCoordinatorKeyGenComplaintResponded)
				if err := _FROSTCoordinator.contract.UnpackLog(event, "KeyGenComplaintResponded", log); err != nil {
					return err
				}
				event.Raw = log

				select {
				case sink <- event:
				case err := <-sub.Err():
					return err
				case <-quit:
					return nil
				}
			case err := <-sub.Err():
				return err
			case <-quit:
				return nil
			}
		}
	}), nil
}

// ParseKeyGenComplaintResponded is a log parse operation binding the contract event 0x752c21192c04ff0d891e5e67e2e34f0411cd5285d216a8068b19842343e9cbb2.
//
// Solidity: event KeyGenComplaintResponded(bytes32 indexed gid, address plaintiff, address accused, uint256 secretShare)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) ParseKeyGenComplaintResponded(log types.Log) (*FROSTCoordinatorKeyGenComplaintResponded, error) {
	event := new(FROSTCoordinatorKeyGenComplaintResponded)
	if err := _FROSTCoordinator.contract.UnpackLog(event, "KeyGenComplaintResponded", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// FROSTCoordinatorKeyGenConfirmedIterator is returned from FilterKeyGenConfirmed and is used to iterate over the raw logs and unpacked data for KeyGenConfirmed events raised by the FROSTCoordinator contract.
type FROSTCoordinatorKeyGenConfirmedIterator struct {
	Event *FROSTCoordinatorKeyGenConfirmed // Event containing the contract specifics and raw log

	contract *bind.BoundContract // Generic contract to use for unpacking event data
	event    string              // Event name to use for unpacking event data

	logs chan types.Log        // Log channel receiving the found contract events
	sub  ethereum.Subscription // Subscription for errors, completion and termination
	done bool                  // Whether the subscription completed delivering logs
	fail error                 // Occurred error to stop iteration
}

// Next advances the iterator to the subsequent event, returning whether there
// are any more events found. In case of a retrieval or parsing error, false is
// returned and Error() can be queried for the exact failure.
func (it *FROSTCoordinatorKeyGenConfirmedIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(FROSTCoordinatorKeyGenConfirmed)
			if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
				it.fail = err
				return false
			}
			it.Event.Raw = log
			return true

		default:
			return false
		}
	}
	// Iterator still in progress, wait for either a data or an error event
	select {
	case log := <-it.logs:
		it.Event = new(FROSTCoordinatorKeyGenConfirmed)
		if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
			it.fail = err
			return false
		}
		it.Event.Raw = log
		return true

	case err := <-it.sub.Err():
		it.done = true
		it.fail = err
		return it.Next()
	}
}

// Error returns any retrieval or parsing error occurred during filtering.
func (it *FROSTCoordinatorKeyGenConfirmedIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *FROSTCoordinatorKeyGenConfirmedIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// FROSTCoordinatorKeyGenConfirmed represents a KeyGenConfirmed event raised by the FROSTCoordinator contract.
type FROSTCoordinatorKeyGenConfirmed struct {
	Gid         [32]byte
	Participant common.Address
	Confirmed   bool
	Raw         types.Log // Blockchain specific contextual infos
}

// FilterKeyGenConfirmed is a free log retrieval operation binding the contract event 0x2553b3b5476eaf8b6ccc0c1656cd21552f8e85959654fd47d733f6e94bc65202.
//
// Solidity: event KeyGenConfirmed(bytes32 indexed gid, address participant, bool confirmed)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) FilterKeyGenConfirmed(opts *bind.FilterOpts, gid [][32]byte) (*FROSTCoordinatorKeyGenConfirmedIterator, error) {

	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.FilterLogs(opts, "KeyGenConfirmed", gidRule)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinatorKeyGenConfirmedIterator{contract: _FROSTCoordinator.contract, event: "KeyGenConfirmed", logs: logs, sub: sub}, nil
}

// WatchKeyGenConfirmed is a free log subscription operation binding the contract event 0x2553b3b5476eaf8b6ccc0c1656cd21552f8e85959654fd47d733f6e94bc65202.
//
// Solidity: event KeyGenConfirmed(bytes32 indexed gid, address participant, bool confirmed)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) WatchKeyGenConfirmed(opts *bind.WatchOpts, sink chan<- *FROSTCoordinatorKeyGenConfirmed, gid [][32]byte) (event.Subscription, error) {

	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.WatchLogs(opts, "KeyGenConfirmed", gidRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(FROSTCoordinatorKeyGenConfirmed)
				if err := _FROSTCoordinator.contract.UnpackLog(event, "KeyGenConfirmed", log); err != nil {
					return err
				}
				event.Raw = log

				select {
				case sink <- event:
				case err := <-sub.Err():
					return err
				case <-quit:
					return nil
				}
			case err := <-sub.Err():
				return err
			case <-quit:
				return nil
			}
		}
	}), nil
}

// ParseKeyGenConfirmed is a log parse operation binding the contract event 0x2553b3b5476eaf8b6ccc0c1656cd21552f8e85959654fd47d733f6e94bc65202.
//
// Solidity: event KeyGenConfirmed(bytes32 indexed gid, address participant, bool confirmed)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) ParseKeyGenConfirmed(log types.Log) (*FROSTCoordinatorKeyGenConfirmed, error) {
	event := new(FROSTCoordinatorKeyGenConfirmed)
	if err := _FROSTCoordinator.contract.UnpackLog(event, "KeyGenConfirmed", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// FROSTCoordinatorKeyGenSecretSharedIterator is returned from FilterKeyGenSecretShared and is used to iterate over the raw logs and unpacked data for KeyGenSecretShared events raised by the FROSTCoordinator contract.
type FROSTCoordinatorKeyGenSecretSharedIterator struct {
	Event *FROSTCoordinatorKeyGenSecretShared // Event containing the contract specifics and raw log

	contract *bind.BoundContract // Generic contract to use for unpacking event data
	event    string              // Event name to use for unpacking event data

	logs chan types.Log        // Log channel receiving the found contract events
	sub  ethereum.Subscription // Subscription for errors, completion and termination
	done bool                  // Whether the subscription completed delivering logs
	fail error                 // Occurred error to stop iteration
}

// Next advances the iterator to the subsequent event, returning whether there
// are any more events found. In case of a retrieval or parsing error, false is
// returned and Error() can be queried for the exact failure.
func (it *FROSTCoordinatorKeyGenSecretSharedIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(FROSTCoordinatorKeyGenSecretShared)
			if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
				it.fail = err
				return false
			}
			it.Event.Raw = log
			return true

		default:
			return false
		}
	}
	// Iterator still in progress, wait for either a data or an error event
	select {
	case log := <-it.logs:
		it.Event = new(FROSTCoordinatorKeyGenSecretShared)
		if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
			it.fail = err
			return false
		}
		it.Event.Raw = log
		return true

	case err := <-it.sub.Err():
		it.done = true
		it.fail = err
		return it.Next()
	}
}

// Error returns any retrieval or parsing error occurred during filtering.
func (it *FROSTCoordinatorKeyGenSecretSharedIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *FROSTCoordinatorKeyGenSecretSharedIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// FROSTCoordinatorKeyGenSecretShared represents a KeyGenSecretShared event raised by the FROSTCoordinator contract.
type FROSTCoordinatorKeyGenSecretShared struct {
	Gid         [32]byte
	Participant common.Address
	Share       FROSTCoordinatorKeyGenSecretShare
	Shared      bool
	Raw         types.Log // Blockchain specific contextual infos
}

// FilterKeyGenSecretShared is a free log retrieval operation binding the contract event 0x8ad66fd7316af8b492b75c5aa77da4d401a44f502913a095f4142696005832b7.
//
// Solidity: event KeyGenSecretShared(bytes32 indexed gid, address participant, ((uint256,uint256),uint256[]) share, bool shared)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) FilterKeyGenSecretShared(opts *bind.FilterOpts, gid [][32]byte) (*FROSTCoordinatorKeyGenSecretSharedIterator, error) {

	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.FilterLogs(opts, "KeyGenSecretShared", gidRule)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinatorKeyGenSecretSharedIterator{contract: _FROSTCoordinator.contract, event: "KeyGenSecretShared", logs: logs, sub: sub}, nil
}

// WatchKeyGenSecretShared is a free log subscription operation binding the contract event 0x8ad66fd7316af8b492b75c5aa77da4d401a44f502913a095f4142696005832b7.
//
// Solidity: event KeyGenSecretShared(bytes32 indexed gid, address participant, ((uint256,uint256),uint256[]) share, bool shared)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) WatchKeyGenSecretShared(opts *bind.WatchOpts, sink chan<- *FROSTCoordinatorKeyGenSecretShared, gid [][32]byte) (event.Subscription, error) {

	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.WatchLogs(opts, "KeyGenSecretShared", gidRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(FROSTCoordinatorKeyGenSecretShared)
				if err := _FROSTCoordinator.contract.UnpackLog(event, "KeyGenSecretShared", log); err != nil {
					return err
				}
				event.Raw = log

				select {
				case sink <- event:
				case err := <-sub.Err():
					return err
				case <-quit:
					return nil
				}
			case err := <-sub.Err():
				return err
			case <-quit:
				return nil
			}
		}
	}), nil
}

// ParseKeyGenSecretShared is a log parse operation binding the contract event 0x8ad66fd7316af8b492b75c5aa77da4d401a44f502913a095f4142696005832b7.
//
// Solidity: event KeyGenSecretShared(bytes32 indexed gid, address participant, ((uint256,uint256),uint256[]) share, bool shared)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) ParseKeyGenSecretShared(log types.Log) (*FROSTCoordinatorKeyGenSecretShared, error) {
	event := new(FROSTCoordinatorKeyGenSecretShared)
	if err := _FROSTCoordinator.contract.UnpackLog(event, "KeyGenSecretShared", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// FROSTCoordinatorPreprocessIterator is returned from FilterPreprocess and is used to iterate over the raw logs and unpacked data for Preprocess events raised by the FROSTCoordinator contract.
type FROSTCoordinatorPreprocessIterator struct {
	Event *FROSTCoordinatorPreprocess // Event containing the contract specifics and raw log

	contract *bind.BoundContract // Generic contract to use for unpacking event data
	event    string              // Event name to use for unpacking event data

	logs chan types.Log        // Log channel receiving the found contract events
	sub  ethereum.Subscription // Subscription for errors, completion and termination
	done bool                  // Whether the subscription completed delivering logs
	fail error                 // Occurred error to stop iteration
}

// Next advances the iterator to the subsequent event, returning whether there
// are any more events found. In case of a retrieval or parsing error, false is
// returned and Error() can be queried for the exact failure.
func (it *FROSTCoordinatorPreprocessIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(FROSTCoordinatorPreprocess)
			if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
				it.fail = err
				return false
			}
			it.Event.Raw = log
			return true

		default:
			return false
		}
	}
	// Iterator still in progress, wait for either a data or an error event
	select {
	case log := <-it.logs:
		it.Event = new(FROSTCoordinatorPreprocess)
		if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
			it.fail = err
			return false
		}
		it.Event.Raw = log
		return true

	case err := <-it.sub.Err():
		it.done = true
		it.fail = err
		return it.Next()
	}
}

// Error returns any retrieval or parsing error occurred during filtering.
func (it *FROSTCoordinatorPreprocessIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *FROSTCoordinatorPreprocessIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// FROSTCoordinatorPreprocess represents a Preprocess event raised by the FROSTCoordinator contract.
type FROSTCoordinatorPreprocess struct {
	Gid         [32]byte
	Participant common.Address
	Chunk       uint64
	Commitment  [32]byte
	Raw         types.Log // Blockchain specific contextual infos
}

// FilterPreprocess is a free log retrieval operation binding the contract event 0x38107eecb8be72b1b829bce317d7b161fe99c4ac90b58abda7c5ce969f196c6c.
//
// Solidity: event Preprocess(bytes32 indexed gid, address participant, uint64 chunk, bytes32 commitment)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) FilterPreprocess(opts *bind.FilterOpts, gid [][32]byte) (*FROSTCoordinatorPreprocessIterator, error) {

	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.FilterLogs(opts, "Preprocess", gidRule)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinatorPreprocessIterator{contract: _FROSTCoordinator.contract, event: "Preprocess", logs: logs, sub: sub}, nil
}

// WatchPreprocess is a free log subscription operation binding the contract event 0x38107eecb8be72b1b829bce317d7b161fe99c4ac90b58abda7c5ce969f196c6c.
//
// Solidity: event Preprocess(bytes32 indexed gid, address participant, uint64 chunk, bytes32 commitment)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) WatchPreprocess(opts *bind.WatchOpts, sink chan<- *FROSTCoordinatorPreprocess, gid [][32]byte) (event.Subscription, error) {

	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.WatchLogs(opts, "Preprocess", gidRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(FROSTCoordinatorPreprocess)
				if err := _FROSTCoordinator.contract.UnpackLog(event, "Preprocess", log); err != nil {
					return err
				}
				event.Raw = log

				select {
				case sink <- event:
				case err := <-sub.Err():
					return err
				case <-quit:
					return nil
				}
			case err := <-sub.Err():
				return err
			case <-quit:
				return nil
			}
		}
	}), nil
}

// ParsePreprocess is a log parse operation binding the contract event 0x38107eecb8be72b1b829bce317d7b161fe99c4ac90b58abda7c5ce969f196c6c.
//
// Solidity: event Preprocess(bytes32 indexed gid, address participant, uint64 chunk, bytes32 commitment)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) ParsePreprocess(log types.Log) (*FROSTCoordinatorPreprocess, error) {
	event := new(FROSTCoordinatorPreprocess)
	if err := _FROSTCoordinator.contract.UnpackLog(event, "Preprocess", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// FROSTCoordinatorSignIterator is returned from FilterSign and is used to iterate over the raw logs and unpacked data for Sign events raised by the FROSTCoordinator contract.
type FROSTCoordinatorSignIterator struct {
	Event *FROSTCoordinatorSign // Event containing the contract specifics and raw log

	contract *bind.BoundContract // Generic contract to use for unpacking event data
	event    string              // Event name to use for unpacking event data

	logs chan types.Log        // Log channel receiving the found contract events
	sub  ethereum.Subscription // Subscription for errors, completion and termination
	done bool                  // Whether the subscription completed delivering logs
	fail error                 // Occurred error to stop iteration
}

// Next advances the iterator to the subsequent event, returning whether there
// are any more events found. In case of a retrieval or parsing error, false is
// returned and Error() can be queried for the exact failure.
func (it *FROSTCoordinatorSignIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(FROSTCoordinatorSign)
			if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
				it.fail = err
				return false
			}
			it.Event.Raw = log
			return true

		default:
			return false
		}
	}
	// Iterator still in progress, wait for either a data or an error event
	select {
	case log := <-it.logs:
		it.Event = new(FROSTCoordinatorSign)
		if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
			it.fail = err
			return false
		}
		it.Event.Raw = log
		return true

	case err := <-it.sub.Err():
		it.done = true
		it.fail = err
		return it.Next()
	}
}

// Error returns any retrieval or parsing error occurred during filtering.
func (it *FROSTCoordinatorSignIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *FROSTCoordinatorSignIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// FROSTCoordinatorSign represents a Sign event raised by the FROSTCoordinator contract.
type FROSTCoordinatorSign struct {
	Initiator common.Address
	Gid       [32]byte
	Message   [32]byte
	Sid       [32]byte
	Sequence  uint64
	Raw       types.Log // Blockchain specific contextual infos
}

// FilterSign is a free log retrieval operation binding the contract event 0xb48d242879f9f3df555c800db966f65cba128c7213198748fa202ed54e092691.
//
// Solidity: event Sign(address indexed initiator, bytes32 indexed gid, bytes32 indexed message, bytes32 sid, uint64 sequence)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) FilterSign(opts *bind.FilterOpts, initiator []common.Address, gid [][32]byte, message [][32]byte) (*FROSTCoordinatorSignIterator, error) {

	var initiatorRule []interface{}
	for _, initiatorItem := range initiator {
		initiatorRule = append(initiatorRule, initiatorItem)
	}
	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}
	var messageRule []interface{}
	for _, messageItem := range message {
		messageRule = append(messageRule, messageItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.FilterLogs(opts, "Sign", initiatorRule, gidRule, messageRule)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinatorSignIterator{contract: _FROSTCoordinator.contract, event: "Sign", logs: logs, sub: sub}, nil
}

// WatchSign is a free log subscription operation binding the contract event 0xb48d242879f9f3df555c800db966f65cba128c7213198748fa202ed54e092691.
//
// Solidity: event Sign(address indexed initiator, bytes32 indexed gid, bytes32 indexed message, bytes32 sid, uint64 sequence)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) WatchSign(opts *bind.WatchOpts, sink chan<- *FROSTCoordinatorSign, initiator []common.Address, gid [][32]byte, message [][32]byte) (event.Subscription, error) {

	var initiatorRule []interface{}
	for _, initiatorItem := range initiator {
		initiatorRule = append(initiatorRule, initiatorItem)
	}
	var gidRule []interface{}
	for _, gidItem := range gid {
		gidRule = append(gidRule, gidItem)
	}
	var messageRule []interface{}
	for _, messageItem := range message {
		messageRule = append(messageRule, messageItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.WatchLogs(opts, "Sign", initiatorRule, gidRule, messageRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(FROSTCoordinatorSign)
				if err := _FROSTCoordinator.contract.UnpackLog(event, "Sign", log); err != nil {
					return err
				}
				event.Raw = log

				select {
				case sink <- event:
				case err := <-sub.Err():
					return err
				case <-quit:
					return nil
				}
			case err := <-sub.Err():
				return err
			case <-quit:
				return nil
			}
		}
	}), nil
}

// ParseSign is a log parse operation binding the contract event 0xb48d242879f9f3df555c800db966f65cba128c7213198748fa202ed54e092691.
//
// Solidity: event Sign(address indexed initiator, bytes32 indexed gid, bytes32 indexed message, bytes32 sid, uint64 sequence)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) ParseSign(log types.Log) (*FROSTCoordinatorSign, error) {
	event := new(FROSTCoordinatorSign)
	if err := _FROSTCoordinator.contract.UnpackLog(event, "Sign", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// FROSTCoordinatorSignCompletedIterator is returned from FilterSignCompleted and is used to iterate over the raw logs and unpacked data for SignCompleted events raised by the FROSTCoordinator contract.
type FROSTCoordinatorSignCompletedIterator struct {
	Event *FROSTCoordinatorSignCompleted // Event containing the contract specifics and raw log

	contract *bind.BoundContract // Generic contract to use for unpacking event data
	event    string              // Event name to use for unpacking event data

	logs chan types.Log        // Log channel receiving the found contract events
	sub  ethereum.Subscription // Subscription for errors, completion and termination
	done bool                  // Whether the subscription completed delivering logs
	fail error                 // Occurred error to stop iteration
}

// Next advances the iterator to the subsequent event, returning whether there
// are any more events found. In case of a retrieval or parsing error, false is
// returned and Error() can be queried for the exact failure.
func (it *FROSTCoordinatorSignCompletedIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(FROSTCoordinatorSignCompleted)
			if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
				it.fail = err
				return false
			}
			it.Event.Raw = log
			return true

		default:
			return false
		}
	}
	// Iterator still in progress, wait for either a data or an error event
	select {
	case log := <-it.logs:
		it.Event = new(FROSTCoordinatorSignCompleted)
		if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
			it.fail = err
			return false
		}
		it.Event.Raw = log
		return true

	case err := <-it.sub.Err():
		it.done = true
		it.fail = err
		return it.Next()
	}
}

// Error returns any retrieval or parsing error occurred during filtering.
func (it *FROSTCoordinatorSignCompletedIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *FROSTCoordinatorSignCompletedIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// FROSTCoordinatorSignCompleted represents a SignCompleted event raised by the FROSTCoordinator contract.
type FROSTCoordinatorSignCompleted struct {
	Sid           [32]byte
	SelectionRoot [32]byte
	Signature     FROSTSignature
	Raw           types.Log // Blockchain specific contextual infos
}

// FilterSignCompleted is a free log retrieval operation binding the contract event 0x7f1641fa8e46c311f05dc0cb9f69f0ac0f27dd12388a5587c7983de9e99a028d.
//
// Solidity: event SignCompleted(bytes32 indexed sid, bytes32 indexed selectionRoot, ((uint256,uint256),uint256) signature)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) FilterSignCompleted(opts *bind.FilterOpts, sid [][32]byte, selectionRoot [][32]byte) (*FROSTCoordinatorSignCompletedIterator, error) {

	var sidRule []interface{}
	for _, sidItem := range sid {
		sidRule = append(sidRule, sidItem)
	}
	var selectionRootRule []interface{}
	for _, selectionRootItem := range selectionRoot {
		selectionRootRule = append(selectionRootRule, selectionRootItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.FilterLogs(opts, "SignCompleted", sidRule, selectionRootRule)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinatorSignCompletedIterator{contract: _FROSTCoordinator.contract, event: "SignCompleted", logs: logs, sub: sub}, nil
}

// WatchSignCompleted is a free log subscription operation binding the contract event 0x7f1641fa8e46c311f05dc0cb9f69f0ac0f27dd12388a5587c7983de9e99a028d.
//
// Solidity: event SignCompleted(bytes32 indexed sid, bytes32 indexed selectionRoot, ((uint256,uint256),uint256) signature)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) WatchSignCompleted(opts *bind.WatchOpts, sink chan<- *FROSTCoordinatorSignCompleted, sid [][32]byte, selectionRoot [][32]byte) (event.Subscription, error) {

	var sidRule []interface{}
	for _, sidItem := range sid {
		sidRule = append(sidRule, sidItem)
	}
	var selectionRootRule []interface{}
	for _, selectionRootItem := range selectionRoot {
		selectionRootRule = append(selectionRootRule, selectionRootItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.WatchLogs(opts, "SignCompleted", sidRule, selectionRootRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(FROSTCoordinatorSignCompleted)
				if err := _FROSTCoordinator.contract.UnpackLog(event, "SignCompleted", log); err != nil {
					return err
				}
				event.Raw = log

				select {
				case sink <- event:
				case err := <-sub.Err():
					return err
				case <-quit:
					return nil
				}
			case err := <-sub.Err():
				return err
			case <-quit:
				return nil
			}
		}
	}), nil
}

// ParseSignCompleted is a log parse operation binding the contract event 0x7f1641fa8e46c311f05dc0cb9f69f0ac0f27dd12388a5587c7983de9e99a028d.
//
// Solidity: event SignCompleted(bytes32 indexed sid, bytes32 indexed selectionRoot, ((uint256,uint256),uint256) signature)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) ParseSignCompleted(log types.Log) (*FROSTCoordinatorSignCompleted, error) {
	event := new(FROSTCoordinatorSignCompleted)
	if err := _FROSTCoordinator.contract.UnpackLog(event, "SignCompleted", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// FROSTCoordinatorSignRevealedNoncesIterator is returned from FilterSignRevealedNonces and is used to iterate over the raw logs and unpacked data for SignRevealedNonces events raised by the FROSTCoordinator contract.
type FROSTCoordinatorSignRevealedNoncesIterator struct {
	Event *FROSTCoordinatorSignRevealedNonces // Event containing the contract specifics and raw log

	contract *bind.BoundContract // Generic contract to use for unpacking event data
	event    string              // Event name to use for unpacking event data

	logs chan types.Log        // Log channel receiving the found contract events
	sub  ethereum.Subscription // Subscription for errors, completion and termination
	done bool                  // Whether the subscription completed delivering logs
	fail error                 // Occurred error to stop iteration
}

// Next advances the iterator to the subsequent event, returning whether there
// are any more events found. In case of a retrieval or parsing error, false is
// returned and Error() can be queried for the exact failure.
func (it *FROSTCoordinatorSignRevealedNoncesIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(FROSTCoordinatorSignRevealedNonces)
			if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
				it.fail = err
				return false
			}
			it.Event.Raw = log
			return true

		default:
			return false
		}
	}
	// Iterator still in progress, wait for either a data or an error event
	select {
	case log := <-it.logs:
		it.Event = new(FROSTCoordinatorSignRevealedNonces)
		if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
			it.fail = err
			return false
		}
		it.Event.Raw = log
		return true

	case err := <-it.sub.Err():
		it.done = true
		it.fail = err
		return it.Next()
	}
}

// Error returns any retrieval or parsing error occurred during filtering.
func (it *FROSTCoordinatorSignRevealedNoncesIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *FROSTCoordinatorSignRevealedNoncesIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// FROSTCoordinatorSignRevealedNonces represents a SignRevealedNonces event raised by the FROSTCoordinator contract.
type FROSTCoordinatorSignRevealedNonces struct {
	Sid         [32]byte
	Participant common.Address
	Nonces      FROSTCoordinatorSignNonces
	Raw         types.Log // Blockchain specific contextual infos
}

// FilterSignRevealedNonces is a free log retrieval operation binding the contract event 0xa8415ae8824ba92b55156b0447b9b9bbc3ba63988b076fb0c8d8e180893d1a46.
//
// Solidity: event SignRevealedNonces(bytes32 indexed sid, address participant, ((uint256,uint256),(uint256,uint256)) nonces)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) FilterSignRevealedNonces(opts *bind.FilterOpts, sid [][32]byte) (*FROSTCoordinatorSignRevealedNoncesIterator, error) {

	var sidRule []interface{}
	for _, sidItem := range sid {
		sidRule = append(sidRule, sidItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.FilterLogs(opts, "SignRevealedNonces", sidRule)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinatorSignRevealedNoncesIterator{contract: _FROSTCoordinator.contract, event: "SignRevealedNonces", logs: logs, sub: sub}, nil
}

// WatchSignRevealedNonces is a free log subscription operation binding the contract event 0xa8415ae8824ba92b55156b0447b9b9bbc3ba63988b076fb0c8d8e180893d1a46.
//
// Solidity: event SignRevealedNonces(bytes32 indexed sid, address participant, ((uint256,uint256),(uint256,uint256)) nonces)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) WatchSignRevealedNonces(opts *bind.WatchOpts, sink chan<- *FROSTCoordinatorSignRevealedNonces, sid [][32]byte) (event.Subscription, error) {

	var sidRule []interface{}
	for _, sidItem := range sid {
		sidRule = append(sidRule, sidItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.WatchLogs(opts, "SignRevealedNonces", sidRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(FROSTCoordinatorSignRevealedNonces)
				if err := _FROSTCoordinator.contract.UnpackLog(event, "SignRevealedNonces", log); err != nil {
					return err
				}
				event.Raw = log

				select {
				case sink <- event:
				case err := <-sub.Err():
					return err
				case <-quit:
					return nil
				}
			case err := <-sub.Err():
				return err
			case <-quit:
				return nil
			}
		}
	}), nil
}

// ParseSignRevealedNonces is a log parse operation binding the contract event 0xa8415ae8824ba92b55156b0447b9b9bbc3ba63988b076fb0c8d8e180893d1a46.
//
// Solidity: event SignRevealedNonces(bytes32 indexed sid, address participant, ((uint256,uint256),(uint256,uint256)) nonces)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) ParseSignRevealedNonces(log types.Log) (*FROSTCoordinatorSignRevealedNonces, error) {
	event := new(FROSTCoordinatorSignRevealedNonces)
	if err := _FROSTCoordinator.contract.UnpackLog(event, "SignRevealedNonces", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// FROSTCoordinatorSignSharedIterator is returned from FilterSignShared and is used to iterate over the raw logs and unpacked data for SignShared events raised by the FROSTCoordinator contract.
type FROSTCoordinatorSignSharedIterator struct {
	Event *FROSTCoordinatorSignShared // Event containing the contract specifics and raw log

	contract *bind.BoundContract // Generic contract to use for unpacking event data
	event    string              // Event name to use for unpacking event data

	logs chan types.Log        // Log channel receiving the found contract events
	sub  ethereum.Subscription // Subscription for errors, completion and termination
	done bool                  // Whether the subscription completed delivering logs
	fail error                 // Occurred error to stop iteration
}

// Next advances the iterator to the subsequent event, returning whether there
// are any more events found. In case of a retrieval or parsing error, false is
// returned and Error() can be queried for the exact failure.
func (it *FROSTCoordinatorSignSharedIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(FROSTCoordinatorSignShared)
			if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
				it.fail = err
				return false
			}
			it.Event.Raw = log
			return true

		default:
			return false
		}
	}
	// Iterator still in progress, wait for either a data or an error event
	select {
	case log := <-it.logs:
		it.Event = new(FROSTCoordinatorSignShared)
		if err := it.contract.UnpackLog(it.Event, it.event, log); err != nil {
			it.fail = err
			return false
		}
		it.Event.Raw = log
		return true

	case err := <-it.sub.Err():
		it.done = true
		it.fail = err
		return it.Next()
	}
}

// Error returns any retrieval or parsing error occurred during filtering.
func (it *FROSTCoordinatorSignSharedIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *FROSTCoordinatorSignSharedIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// FROSTCoordinatorSignShared represents a SignShared event raised by the FROSTCoordinator contract.
type FROSTCoordinatorSignShared struct {
	Sid           [32]byte
	SelectionRoot [32]byte
	Participant   common.Address
	Z             *big.Int
	Raw           types.Log // Blockchain specific contextual infos
}

// FilterSignShared is a free log retrieval operation binding the contract event 0x25a4d6e8d11a9fdc20ffdd826473485ae4cdd453271726c072a16836c1882e7c.
//
// Solidity: event SignShared(bytes32 indexed sid, bytes32 indexed selectionRoot, address participant, uint256 z)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) FilterSignShared(opts *bind.FilterOpts, sid [][32]byte, selectionRoot [][32]byte) (*FROSTCoordinatorSignSharedIterator, error) {

	var sidRule []interface{}
	for _, sidItem := range sid {
		sidRule = append(sidRule, sidItem)
	}
	var selectionRootRule []interface{}
	for _, selectionRootItem := range selectionRoot {
		selectionRootRule = append(selectionRootRule, selectionRootItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.FilterLogs(opts, "SignShared", sidRule, selectionRootRule)
	if err != nil {
		return nil, err
	}
	return &FROSTCoordinatorSignSharedIterator{contract: _FROSTCoordinator.contract, event: "SignShared", logs: logs, sub: sub}, nil
}

// WatchSignShared is a free log subscription operation binding the contract event 0x25a4d6e8d11a9fdc20ffdd826473485ae4cdd453271726c072a16836c1882e7c.
//
// Solidity: event SignShared(bytes32 indexed sid, bytes32 indexed selectionRoot, address participant, uint256 z)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) WatchSignShared(opts *bind.WatchOpts, sink chan<- *FROSTCoordinatorSignShared, sid [][32]byte, selectionRoot [][32]byte) (event.Subscription, error) {

	var sidRule []interface{}
	for _, sidItem := range sid {
		sidRule = append(sidRule, sidItem)
	}
	var selectionRootRule []interface{}
	for _, selectionRootItem := range selectionRoot {
		selectionRootRule = append(selectionRootRule, selectionRootItem)
	}

	logs, sub, err := _FROSTCoordinator.contract.WatchLogs(opts, "SignShared", sidRule, selectionRootRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(FROSTCoordinatorSignShared)
				if err := _FROSTCoordinator.contract.UnpackLog(event, "SignShared", log); err != nil {
					return err
				}
				event.Raw = log

				select {
				case sink <- event:
				case err := <-sub.Err():
					return err
				case <-quit:
					return nil
				}
			case err := <-sub.Err():
				return err
			case <-quit:
				return nil
			}
		}
	}), nil
}

// ParseSignShared is a log parse operation binding the contract event 0x25a4d6e8d11a9fdc20ffdd826473485ae4cdd453271726c072a16836c1882e7c.
//
// Solidity: event SignShared(bytes32 indexed sid, bytes32 indexed selectionRoot, address participant, uint256 z)
func (_FROSTCoordinator *FROSTCoordinatorFilterer) ParseSignShared(log types.Log) (*FROSTCoordinatorSignShared, error) {
	event := new(FROSTCoordinatorSignShared)
	if err := _FROSTCoordinator.contract.UnpackLog(event, "SignShared", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}
