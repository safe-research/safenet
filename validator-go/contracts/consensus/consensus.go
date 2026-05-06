// Code generated - DO NOT EDIT.
// This file is a generated binding and any manual changes will be lost.

package consensus

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

// ConsensusEpochs is an auto generated low-level Go binding around an user-defined struct.
type ConsensusEpochs struct {
	Previous      uint64
	Active        uint64
	Staged        uint64
	RolloverBlock uint64
}

// FROSTSignature is an auto generated low-level Go binding around an user-defined struct.
type FROSTSignature struct {
	R Secp256k1Point
	Z *big.Int
}

// SafeTransactionT is an auto generated low-level Go binding around an user-defined struct.
type SafeTransactionT struct {
	ChainId        *big.Int
	Safe           common.Address
	To             common.Address
	Value          *big.Int
	Data           []byte
	Operation      uint8
	SafeTxGas      *big.Int
	BaseGas        *big.Int
	GasPrice       *big.Int
	GasToken       common.Address
	RefundReceiver common.Address
	Nonce          *big.Int
}

// Secp256k1Point is an auto generated low-level Go binding around an user-defined struct.
type Secp256k1Point struct {
	X *big.Int
	Y *big.Int
}

// ConsensusMetaData contains all meta data concerning the Consensus contract.
var ConsensusMetaData = &bind.MetaData{
	ABI: "[{\"type\":\"constructor\",\"inputs\":[{\"name\":\"coordinator\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"groupId\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"attestTransaction\",\"inputs\":[{\"name\":\"epoch\",\"type\":\"uint64\",\"internalType\":\"uint64\"},{\"name\":\"chainId\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"safe\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"safeTxStructHash\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"},{\"name\":\"signatureId\",\"type\":\"bytes32\",\"internalType\":\"FROSTSignatureId.T\"}],\"outputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"domainSeparator\",\"inputs\":[],\"outputs\":[{\"name\":\"result\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"getActiveEpoch\",\"inputs\":[],\"outputs\":[{\"name\":\"epoch\",\"type\":\"uint64\",\"internalType\":\"uint64\"},{\"name\":\"groupId\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"getAttestationSignatureId\",\"inputs\":[{\"name\":\"message\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"}],\"outputs\":[{\"name\":\"signature\",\"type\":\"bytes32\",\"internalType\":\"FROSTSignatureId.T\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"getCoordinator\",\"inputs\":[],\"outputs\":[{\"name\":\"coordinator\",\"type\":\"address\",\"internalType\":\"address\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"getEpochGroupId\",\"inputs\":[{\"name\":\"epoch\",\"type\":\"uint64\",\"internalType\":\"uint64\"}],\"outputs\":[{\"name\":\"groupId\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"getEpochsState\",\"inputs\":[],\"outputs\":[{\"name\":\"epochs\",\"type\":\"tuple\",\"internalType\":\"structConsensus.Epochs\",\"components\":[{\"name\":\"previous\",\"type\":\"uint64\",\"internalType\":\"uint64\"},{\"name\":\"active\",\"type\":\"uint64\",\"internalType\":\"uint64\"},{\"name\":\"staged\",\"type\":\"uint64\",\"internalType\":\"uint64\"},{\"name\":\"rolloverBlock\",\"type\":\"uint64\",\"internalType\":\"uint64\"}]}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"getRecentTransactionAttestation\",\"inputs\":[{\"name\":\"transaction\",\"type\":\"tuple\",\"internalType\":\"structSafeTransaction.T\",\"components\":[{\"name\":\"chainId\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"safe\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"to\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"value\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"data\",\"type\":\"bytes\",\"internalType\":\"bytes\"},{\"name\":\"operation\",\"type\":\"uint8\",\"internalType\":\"enumSafeTransaction.Operation\"},{\"name\":\"safeTxGas\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"baseGas\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"gasPrice\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"gasToken\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"refundReceiver\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"nonce\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"outputs\":[{\"name\":\"epoch\",\"type\":\"uint64\",\"internalType\":\"uint64\"},{\"name\":\"signature\",\"type\":\"tuple\",\"internalType\":\"structFROST.Signature\",\"components\":[{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"z\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"getRecentTransactionAttestationByHash\",\"inputs\":[{\"name\":\"safeTxHash\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"}],\"outputs\":[{\"name\":\"epoch\",\"type\":\"uint64\",\"internalType\":\"uint64\"},{\"name\":\"signature\",\"type\":\"tuple\",\"internalType\":\"structFROST.Signature\",\"components\":[{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"z\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"getTransactionAttestation\",\"inputs\":[{\"name\":\"epoch\",\"type\":\"uint64\",\"internalType\":\"uint64\"},{\"name\":\"transaction\",\"type\":\"tuple\",\"internalType\":\"structSafeTransaction.T\",\"components\":[{\"name\":\"chainId\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"safe\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"to\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"value\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"data\",\"type\":\"bytes\",\"internalType\":\"bytes\"},{\"name\":\"operation\",\"type\":\"uint8\",\"internalType\":\"enumSafeTransaction.Operation\"},{\"name\":\"safeTxGas\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"baseGas\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"gasPrice\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"gasToken\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"refundReceiver\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"nonce\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"outputs\":[{\"name\":\"signature\",\"type\":\"tuple\",\"internalType\":\"structFROST.Signature\",\"components\":[{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"z\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"getTransactionAttestationByHash\",\"inputs\":[{\"name\":\"epoch\",\"type\":\"uint64\",\"internalType\":\"uint64\"},{\"name\":\"safeTxHash\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"}],\"outputs\":[{\"name\":\"signature\",\"type\":\"tuple\",\"internalType\":\"structFROST.Signature\",\"components\":[{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"z\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"getValidatorStaker\",\"inputs\":[{\"name\":\"validator\",\"type\":\"address\",\"internalType\":\"address\"}],\"outputs\":[{\"name\":\"staker\",\"type\":\"address\",\"internalType\":\"address\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"onKeyGenCompleted\",\"inputs\":[{\"name\":\"groupId\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"context\",\"type\":\"bytes\",\"internalType\":\"bytes\"}],\"outputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"onSignCompleted\",\"inputs\":[{\"name\":\"signatureId\",\"type\":\"bytes32\",\"internalType\":\"FROSTSignatureId.T\"},{\"name\":\"context\",\"type\":\"bytes\",\"internalType\":\"bytes\"}],\"outputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"proposeBasicTransaction\",\"inputs\":[{\"name\":\"chainId\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"safe\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"to\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"value\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"data\",\"type\":\"bytes\",\"internalType\":\"bytes\"},{\"name\":\"nonce\",\"type\":\"uint256\",\"internalType\":\"uint256\"}],\"outputs\":[{\"name\":\"safeTxHash\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"proposeEpoch\",\"inputs\":[{\"name\":\"proposedEpoch\",\"type\":\"uint64\",\"internalType\":\"uint64\"},{\"name\":\"rolloverBlock\",\"type\":\"uint64\",\"internalType\":\"uint64\"},{\"name\":\"groupId\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"}],\"outputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"proposeTransaction\",\"inputs\":[{\"name\":\"transaction\",\"type\":\"tuple\",\"internalType\":\"structSafeTransaction.T\",\"components\":[{\"name\":\"chainId\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"safe\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"to\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"value\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"data\",\"type\":\"bytes\",\"internalType\":\"bytes\"},{\"name\":\"operation\",\"type\":\"uint8\",\"internalType\":\"enumSafeTransaction.Operation\"},{\"name\":\"safeTxGas\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"baseGas\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"gasPrice\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"gasToken\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"refundReceiver\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"nonce\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"outputs\":[{\"name\":\"safeTxHash\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"setValidatorStaker\",\"inputs\":[{\"name\":\"staker\",\"type\":\"address\",\"internalType\":\"address\"}],\"outputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"stageEpoch\",\"inputs\":[{\"name\":\"proposedEpoch\",\"type\":\"uint64\",\"internalType\":\"uint64\"},{\"name\":\"rolloverBlock\",\"type\":\"uint64\",\"internalType\":\"uint64\"},{\"name\":\"groupId\",\"type\":\"bytes32\",\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"signatureId\",\"type\":\"bytes32\",\"internalType\":\"FROSTSignatureId.T\"}],\"outputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"supportsInterface\",\"inputs\":[{\"name\":\"interfaceId\",\"type\":\"bytes4\",\"internalType\":\"bytes4\"}],\"outputs\":[{\"name\":\"\",\"type\":\"bool\",\"internalType\":\"bool\"}],\"stateMutability\":\"pure\"},{\"type\":\"event\",\"name\":\"EpochProposed\",\"inputs\":[{\"name\":\"activeEpoch\",\"type\":\"uint64\",\"indexed\":true,\"internalType\":\"uint64\"},{\"name\":\"proposedEpoch\",\"type\":\"uint64\",\"indexed\":true,\"internalType\":\"uint64\"},{\"name\":\"rolloverBlock\",\"type\":\"uint64\",\"indexed\":false,\"internalType\":\"uint64\"},{\"name\":\"groupId\",\"type\":\"bytes32\",\"indexed\":false,\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"groupKey\",\"type\":\"tuple\",\"indexed\":false,\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"EpochRolledOver\",\"inputs\":[{\"name\":\"newActiveEpoch\",\"type\":\"uint64\",\"indexed\":true,\"internalType\":\"uint64\"}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"EpochStaged\",\"inputs\":[{\"name\":\"activeEpoch\",\"type\":\"uint64\",\"indexed\":true,\"internalType\":\"uint64\"},{\"name\":\"proposedEpoch\",\"type\":\"uint64\",\"indexed\":true,\"internalType\":\"uint64\"},{\"name\":\"rolloverBlock\",\"type\":\"uint64\",\"indexed\":false,\"internalType\":\"uint64\"},{\"name\":\"groupId\",\"type\":\"bytes32\",\"indexed\":false,\"internalType\":\"FROSTGroupId.T\"},{\"name\":\"groupKey\",\"type\":\"tuple\",\"indexed\":false,\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"signatureId\",\"type\":\"bytes32\",\"indexed\":false,\"internalType\":\"FROSTSignatureId.T\"},{\"name\":\"attestation\",\"type\":\"tuple\",\"indexed\":false,\"internalType\":\"structFROST.Signature\",\"components\":[{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"z\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"TransactionAttested\",\"inputs\":[{\"name\":\"safeTxHash\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"bytes32\"},{\"name\":\"chainId\",\"type\":\"uint256\",\"indexed\":true,\"internalType\":\"uint256\"},{\"name\":\"safe\",\"type\":\"address\",\"indexed\":true,\"internalType\":\"address\"},{\"name\":\"epoch\",\"type\":\"uint64\",\"indexed\":false,\"internalType\":\"uint64\"},{\"name\":\"signatureId\",\"type\":\"bytes32\",\"indexed\":false,\"internalType\":\"FROSTSignatureId.T\"},{\"name\":\"attestation\",\"type\":\"tuple\",\"indexed\":false,\"internalType\":\"structFROST.Signature\",\"components\":[{\"name\":\"r\",\"type\":\"tuple\",\"internalType\":\"structSecp256k1.Point\",\"components\":[{\"name\":\"x\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"y\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]},{\"name\":\"z\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"TransactionProposed\",\"inputs\":[{\"name\":\"safeTxHash\",\"type\":\"bytes32\",\"indexed\":true,\"internalType\":\"bytes32\"},{\"name\":\"chainId\",\"type\":\"uint256\",\"indexed\":true,\"internalType\":\"uint256\"},{\"name\":\"safe\",\"type\":\"address\",\"indexed\":true,\"internalType\":\"address\"},{\"name\":\"epoch\",\"type\":\"uint64\",\"indexed\":false,\"internalType\":\"uint64\"},{\"name\":\"transaction\",\"type\":\"tuple\",\"indexed\":false,\"internalType\":\"structSafeTransaction.T\",\"components\":[{\"name\":\"chainId\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"safe\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"to\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"value\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"data\",\"type\":\"bytes\",\"internalType\":\"bytes\"},{\"name\":\"operation\",\"type\":\"uint8\",\"internalType\":\"enumSafeTransaction.Operation\"},{\"name\":\"safeTxGas\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"baseGas\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"gasPrice\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"gasToken\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"refundReceiver\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"nonce\",\"type\":\"uint256\",\"internalType\":\"uint256\"}]}],\"anonymous\":false},{\"type\":\"event\",\"name\":\"ValidatorStakerSet\",\"inputs\":[{\"name\":\"validator\",\"type\":\"address\",\"indexed\":true,\"internalType\":\"address\"},{\"name\":\"staker\",\"type\":\"address\",\"indexed\":false,\"internalType\":\"address\"}],\"anonymous\":false},{\"type\":\"error\",\"name\":\"AlreadyAttested\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"InvalidRollover\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"NotCoordinator\",\"inputs\":[]},{\"type\":\"error\",\"name\":\"UnknownSignatureSelector\",\"inputs\":[]}]",
}

// ConsensusABI is the input ABI used to generate the binding from.
// Deprecated: Use ConsensusMetaData.ABI instead.
var ConsensusABI = ConsensusMetaData.ABI

// Consensus is an auto generated Go binding around an Ethereum contract.
type Consensus struct {
	ConsensusCaller     // Read-only binding to the contract
	ConsensusTransactor // Write-only binding to the contract
	ConsensusFilterer   // Log filterer for contract events
}

// ConsensusCaller is an auto generated read-only Go binding around an Ethereum contract.
type ConsensusCaller struct {
	contract *bind.BoundContract // Generic contract wrapper for the low level calls
}

// ConsensusTransactor is an auto generated write-only Go binding around an Ethereum contract.
type ConsensusTransactor struct {
	contract *bind.BoundContract // Generic contract wrapper for the low level calls
}

// ConsensusFilterer is an auto generated log filtering Go binding around an Ethereum contract events.
type ConsensusFilterer struct {
	contract *bind.BoundContract // Generic contract wrapper for the low level calls
}

// ConsensusSession is an auto generated Go binding around an Ethereum contract,
// with pre-set call and transact options.
type ConsensusSession struct {
	Contract     *Consensus        // Generic contract binding to set the session for
	CallOpts     bind.CallOpts     // Call options to use throughout this session
	TransactOpts bind.TransactOpts // Transaction auth options to use throughout this session
}

// ConsensusCallerSession is an auto generated read-only Go binding around an Ethereum contract,
// with pre-set call options.
type ConsensusCallerSession struct {
	Contract *ConsensusCaller // Generic contract caller binding to set the session for
	CallOpts bind.CallOpts    // Call options to use throughout this session
}

// ConsensusTransactorSession is an auto generated write-only Go binding around an Ethereum contract,
// with pre-set transact options.
type ConsensusTransactorSession struct {
	Contract     *ConsensusTransactor // Generic contract transactor binding to set the session for
	TransactOpts bind.TransactOpts    // Transaction auth options to use throughout this session
}

// ConsensusRaw is an auto generated low-level Go binding around an Ethereum contract.
type ConsensusRaw struct {
	Contract *Consensus // Generic contract binding to access the raw methods on
}

// ConsensusCallerRaw is an auto generated low-level read-only Go binding around an Ethereum contract.
type ConsensusCallerRaw struct {
	Contract *ConsensusCaller // Generic read-only contract binding to access the raw methods on
}

// ConsensusTransactorRaw is an auto generated low-level write-only Go binding around an Ethereum contract.
type ConsensusTransactorRaw struct {
	Contract *ConsensusTransactor // Generic write-only contract binding to access the raw methods on
}

// NewConsensus creates a new instance of Consensus, bound to a specific deployed contract.
func NewConsensus(address common.Address, backend bind.ContractBackend) (*Consensus, error) {
	contract, err := bindConsensus(address, backend, backend, backend)
	if err != nil {
		return nil, err
	}
	return &Consensus{ConsensusCaller: ConsensusCaller{contract: contract}, ConsensusTransactor: ConsensusTransactor{contract: contract}, ConsensusFilterer: ConsensusFilterer{contract: contract}}, nil
}

// NewConsensusCaller creates a new read-only instance of Consensus, bound to a specific deployed contract.
func NewConsensusCaller(address common.Address, caller bind.ContractCaller) (*ConsensusCaller, error) {
	contract, err := bindConsensus(address, caller, nil, nil)
	if err != nil {
		return nil, err
	}
	return &ConsensusCaller{contract: contract}, nil
}

// NewConsensusTransactor creates a new write-only instance of Consensus, bound to a specific deployed contract.
func NewConsensusTransactor(address common.Address, transactor bind.ContractTransactor) (*ConsensusTransactor, error) {
	contract, err := bindConsensus(address, nil, transactor, nil)
	if err != nil {
		return nil, err
	}
	return &ConsensusTransactor{contract: contract}, nil
}

// NewConsensusFilterer creates a new log filterer instance of Consensus, bound to a specific deployed contract.
func NewConsensusFilterer(address common.Address, filterer bind.ContractFilterer) (*ConsensusFilterer, error) {
	contract, err := bindConsensus(address, nil, nil, filterer)
	if err != nil {
		return nil, err
	}
	return &ConsensusFilterer{contract: contract}, nil
}

// bindConsensus binds a generic wrapper to an already deployed contract.
func bindConsensus(address common.Address, caller bind.ContractCaller, transactor bind.ContractTransactor, filterer bind.ContractFilterer) (*bind.BoundContract, error) {
	parsed, err := ConsensusMetaData.GetAbi()
	if err != nil {
		return nil, err
	}
	return bind.NewBoundContract(address, *parsed, caller, transactor, filterer), nil
}

// Call invokes the (constant) contract method with params as input values and
// sets the output to result. The result type might be a single field for simple
// returns, a slice of interfaces for anonymous returns and a struct for named
// returns.
func (_Consensus *ConsensusRaw) Call(opts *bind.CallOpts, result *[]interface{}, method string, params ...interface{}) error {
	return _Consensus.Contract.ConsensusCaller.contract.Call(opts, result, method, params...)
}

// Transfer initiates a plain transaction to move funds to the contract, calling
// its default method if one is available.
func (_Consensus *ConsensusRaw) Transfer(opts *bind.TransactOpts) (*types.Transaction, error) {
	return _Consensus.Contract.ConsensusTransactor.contract.Transfer(opts)
}

// Transact invokes the (paid) contract method with params as input values.
func (_Consensus *ConsensusRaw) Transact(opts *bind.TransactOpts, method string, params ...interface{}) (*types.Transaction, error) {
	return _Consensus.Contract.ConsensusTransactor.contract.Transact(opts, method, params...)
}

// Call invokes the (constant) contract method with params as input values and
// sets the output to result. The result type might be a single field for simple
// returns, a slice of interfaces for anonymous returns and a struct for named
// returns.
func (_Consensus *ConsensusCallerRaw) Call(opts *bind.CallOpts, result *[]interface{}, method string, params ...interface{}) error {
	return _Consensus.Contract.contract.Call(opts, result, method, params...)
}

// Transfer initiates a plain transaction to move funds to the contract, calling
// its default method if one is available.
func (_Consensus *ConsensusTransactorRaw) Transfer(opts *bind.TransactOpts) (*types.Transaction, error) {
	return _Consensus.Contract.contract.Transfer(opts)
}

// Transact invokes the (paid) contract method with params as input values.
func (_Consensus *ConsensusTransactorRaw) Transact(opts *bind.TransactOpts, method string, params ...interface{}) (*types.Transaction, error) {
	return _Consensus.Contract.contract.Transact(opts, method, params...)
}

// DomainSeparator is a free data retrieval call binding the contract method 0xf698da25.
//
// Solidity: function domainSeparator() view returns(bytes32 result)
func (_Consensus *ConsensusCaller) DomainSeparator(opts *bind.CallOpts) ([32]byte, error) {
	var out []interface{}
	err := _Consensus.contract.Call(opts, &out, "domainSeparator")

	if err != nil {
		return *new([32]byte), err
	}

	out0 := *abi.ConvertType(out[0], new([32]byte)).(*[32]byte)

	return out0, err

}

// DomainSeparator is a free data retrieval call binding the contract method 0xf698da25.
//
// Solidity: function domainSeparator() view returns(bytes32 result)
func (_Consensus *ConsensusSession) DomainSeparator() ([32]byte, error) {
	return _Consensus.Contract.DomainSeparator(&_Consensus.CallOpts)
}

// DomainSeparator is a free data retrieval call binding the contract method 0xf698da25.
//
// Solidity: function domainSeparator() view returns(bytes32 result)
func (_Consensus *ConsensusCallerSession) DomainSeparator() ([32]byte, error) {
	return _Consensus.Contract.DomainSeparator(&_Consensus.CallOpts)
}

// GetActiveEpoch is a free data retrieval call binding the contract method 0x69542f9f.
//
// Solidity: function getActiveEpoch() view returns(uint64 epoch, bytes32 groupId)
func (_Consensus *ConsensusCaller) GetActiveEpoch(opts *bind.CallOpts) (struct {
	Epoch   uint64
	GroupId [32]byte
}, error) {
	var out []interface{}
	err := _Consensus.contract.Call(opts, &out, "getActiveEpoch")

	outstruct := new(struct {
		Epoch   uint64
		GroupId [32]byte
	})
	if err != nil {
		return *outstruct, err
	}

	outstruct.Epoch = *abi.ConvertType(out[0], new(uint64)).(*uint64)
	outstruct.GroupId = *abi.ConvertType(out[1], new([32]byte)).(*[32]byte)

	return *outstruct, err

}

// GetActiveEpoch is a free data retrieval call binding the contract method 0x69542f9f.
//
// Solidity: function getActiveEpoch() view returns(uint64 epoch, bytes32 groupId)
func (_Consensus *ConsensusSession) GetActiveEpoch() (struct {
	Epoch   uint64
	GroupId [32]byte
}, error) {
	return _Consensus.Contract.GetActiveEpoch(&_Consensus.CallOpts)
}

// GetActiveEpoch is a free data retrieval call binding the contract method 0x69542f9f.
//
// Solidity: function getActiveEpoch() view returns(uint64 epoch, bytes32 groupId)
func (_Consensus *ConsensusCallerSession) GetActiveEpoch() (struct {
	Epoch   uint64
	GroupId [32]byte
}, error) {
	return _Consensus.Contract.GetActiveEpoch(&_Consensus.CallOpts)
}

// GetAttestationSignatureId is a free data retrieval call binding the contract method 0x522d53c6.
//
// Solidity: function getAttestationSignatureId(bytes32 message) view returns(bytes32 signature)
func (_Consensus *ConsensusCaller) GetAttestationSignatureId(opts *bind.CallOpts, message [32]byte) ([32]byte, error) {
	var out []interface{}
	err := _Consensus.contract.Call(opts, &out, "getAttestationSignatureId", message)

	if err != nil {
		return *new([32]byte), err
	}

	out0 := *abi.ConvertType(out[0], new([32]byte)).(*[32]byte)

	return out0, err

}

// GetAttestationSignatureId is a free data retrieval call binding the contract method 0x522d53c6.
//
// Solidity: function getAttestationSignatureId(bytes32 message) view returns(bytes32 signature)
func (_Consensus *ConsensusSession) GetAttestationSignatureId(message [32]byte) ([32]byte, error) {
	return _Consensus.Contract.GetAttestationSignatureId(&_Consensus.CallOpts, message)
}

// GetAttestationSignatureId is a free data retrieval call binding the contract method 0x522d53c6.
//
// Solidity: function getAttestationSignatureId(bytes32 message) view returns(bytes32 signature)
func (_Consensus *ConsensusCallerSession) GetAttestationSignatureId(message [32]byte) ([32]byte, error) {
	return _Consensus.Contract.GetAttestationSignatureId(&_Consensus.CallOpts, message)
}

// GetCoordinator is a free data retrieval call binding the contract method 0x71977fe0.
//
// Solidity: function getCoordinator() view returns(address coordinator)
func (_Consensus *ConsensusCaller) GetCoordinator(opts *bind.CallOpts) (common.Address, error) {
	var out []interface{}
	err := _Consensus.contract.Call(opts, &out, "getCoordinator")

	if err != nil {
		return *new(common.Address), err
	}

	out0 := *abi.ConvertType(out[0], new(common.Address)).(*common.Address)

	return out0, err

}

// GetCoordinator is a free data retrieval call binding the contract method 0x71977fe0.
//
// Solidity: function getCoordinator() view returns(address coordinator)
func (_Consensus *ConsensusSession) GetCoordinator() (common.Address, error) {
	return _Consensus.Contract.GetCoordinator(&_Consensus.CallOpts)
}

// GetCoordinator is a free data retrieval call binding the contract method 0x71977fe0.
//
// Solidity: function getCoordinator() view returns(address coordinator)
func (_Consensus *ConsensusCallerSession) GetCoordinator() (common.Address, error) {
	return _Consensus.Contract.GetCoordinator(&_Consensus.CallOpts)
}

// GetEpochGroupId is a free data retrieval call binding the contract method 0xe753bd85.
//
// Solidity: function getEpochGroupId(uint64 epoch) view returns(bytes32 groupId)
func (_Consensus *ConsensusCaller) GetEpochGroupId(opts *bind.CallOpts, epoch uint64) ([32]byte, error) {
	var out []interface{}
	err := _Consensus.contract.Call(opts, &out, "getEpochGroupId", epoch)

	if err != nil {
		return *new([32]byte), err
	}

	out0 := *abi.ConvertType(out[0], new([32]byte)).(*[32]byte)

	return out0, err

}

// GetEpochGroupId is a free data retrieval call binding the contract method 0xe753bd85.
//
// Solidity: function getEpochGroupId(uint64 epoch) view returns(bytes32 groupId)
func (_Consensus *ConsensusSession) GetEpochGroupId(epoch uint64) ([32]byte, error) {
	return _Consensus.Contract.GetEpochGroupId(&_Consensus.CallOpts, epoch)
}

// GetEpochGroupId is a free data retrieval call binding the contract method 0xe753bd85.
//
// Solidity: function getEpochGroupId(uint64 epoch) view returns(bytes32 groupId)
func (_Consensus *ConsensusCallerSession) GetEpochGroupId(epoch uint64) ([32]byte, error) {
	return _Consensus.Contract.GetEpochGroupId(&_Consensus.CallOpts, epoch)
}

// GetEpochsState is a free data retrieval call binding the contract method 0xfc7cbb02.
//
// Solidity: function getEpochsState() view returns((uint64,uint64,uint64,uint64) epochs)
func (_Consensus *ConsensusCaller) GetEpochsState(opts *bind.CallOpts) (ConsensusEpochs, error) {
	var out []interface{}
	err := _Consensus.contract.Call(opts, &out, "getEpochsState")

	if err != nil {
		return *new(ConsensusEpochs), err
	}

	out0 := *abi.ConvertType(out[0], new(ConsensusEpochs)).(*ConsensusEpochs)

	return out0, err

}

// GetEpochsState is a free data retrieval call binding the contract method 0xfc7cbb02.
//
// Solidity: function getEpochsState() view returns((uint64,uint64,uint64,uint64) epochs)
func (_Consensus *ConsensusSession) GetEpochsState() (ConsensusEpochs, error) {
	return _Consensus.Contract.GetEpochsState(&_Consensus.CallOpts)
}

// GetEpochsState is a free data retrieval call binding the contract method 0xfc7cbb02.
//
// Solidity: function getEpochsState() view returns((uint64,uint64,uint64,uint64) epochs)
func (_Consensus *ConsensusCallerSession) GetEpochsState() (ConsensusEpochs, error) {
	return _Consensus.Contract.GetEpochsState(&_Consensus.CallOpts)
}

// GetRecentTransactionAttestation is a free data retrieval call binding the contract method 0x4b7ba9e8.
//
// Solidity: function getRecentTransactionAttestation((uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256) transaction) view returns(uint64 epoch, ((uint256,uint256),uint256) signature)
func (_Consensus *ConsensusCaller) GetRecentTransactionAttestation(opts *bind.CallOpts, transaction SafeTransactionT) (struct {
	Epoch     uint64
	Signature FROSTSignature
}, error) {
	var out []interface{}
	err := _Consensus.contract.Call(opts, &out, "getRecentTransactionAttestation", transaction)

	outstruct := new(struct {
		Epoch     uint64
		Signature FROSTSignature
	})
	if err != nil {
		return *outstruct, err
	}

	outstruct.Epoch = *abi.ConvertType(out[0], new(uint64)).(*uint64)
	outstruct.Signature = *abi.ConvertType(out[1], new(FROSTSignature)).(*FROSTSignature)

	return *outstruct, err

}

// GetRecentTransactionAttestation is a free data retrieval call binding the contract method 0x4b7ba9e8.
//
// Solidity: function getRecentTransactionAttestation((uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256) transaction) view returns(uint64 epoch, ((uint256,uint256),uint256) signature)
func (_Consensus *ConsensusSession) GetRecentTransactionAttestation(transaction SafeTransactionT) (struct {
	Epoch     uint64
	Signature FROSTSignature
}, error) {
	return _Consensus.Contract.GetRecentTransactionAttestation(&_Consensus.CallOpts, transaction)
}

// GetRecentTransactionAttestation is a free data retrieval call binding the contract method 0x4b7ba9e8.
//
// Solidity: function getRecentTransactionAttestation((uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256) transaction) view returns(uint64 epoch, ((uint256,uint256),uint256) signature)
func (_Consensus *ConsensusCallerSession) GetRecentTransactionAttestation(transaction SafeTransactionT) (struct {
	Epoch     uint64
	Signature FROSTSignature
}, error) {
	return _Consensus.Contract.GetRecentTransactionAttestation(&_Consensus.CallOpts, transaction)
}

// GetRecentTransactionAttestationByHash is a free data retrieval call binding the contract method 0xb49c5bae.
//
// Solidity: function getRecentTransactionAttestationByHash(bytes32 safeTxHash) view returns(uint64 epoch, ((uint256,uint256),uint256) signature)
func (_Consensus *ConsensusCaller) GetRecentTransactionAttestationByHash(opts *bind.CallOpts, safeTxHash [32]byte) (struct {
	Epoch     uint64
	Signature FROSTSignature
}, error) {
	var out []interface{}
	err := _Consensus.contract.Call(opts, &out, "getRecentTransactionAttestationByHash", safeTxHash)

	outstruct := new(struct {
		Epoch     uint64
		Signature FROSTSignature
	})
	if err != nil {
		return *outstruct, err
	}

	outstruct.Epoch = *abi.ConvertType(out[0], new(uint64)).(*uint64)
	outstruct.Signature = *abi.ConvertType(out[1], new(FROSTSignature)).(*FROSTSignature)

	return *outstruct, err

}

// GetRecentTransactionAttestationByHash is a free data retrieval call binding the contract method 0xb49c5bae.
//
// Solidity: function getRecentTransactionAttestationByHash(bytes32 safeTxHash) view returns(uint64 epoch, ((uint256,uint256),uint256) signature)
func (_Consensus *ConsensusSession) GetRecentTransactionAttestationByHash(safeTxHash [32]byte) (struct {
	Epoch     uint64
	Signature FROSTSignature
}, error) {
	return _Consensus.Contract.GetRecentTransactionAttestationByHash(&_Consensus.CallOpts, safeTxHash)
}

// GetRecentTransactionAttestationByHash is a free data retrieval call binding the contract method 0xb49c5bae.
//
// Solidity: function getRecentTransactionAttestationByHash(bytes32 safeTxHash) view returns(uint64 epoch, ((uint256,uint256),uint256) signature)
func (_Consensus *ConsensusCallerSession) GetRecentTransactionAttestationByHash(safeTxHash [32]byte) (struct {
	Epoch     uint64
	Signature FROSTSignature
}, error) {
	return _Consensus.Contract.GetRecentTransactionAttestationByHash(&_Consensus.CallOpts, safeTxHash)
}

// GetTransactionAttestation is a free data retrieval call binding the contract method 0xe29d3196.
//
// Solidity: function getTransactionAttestation(uint64 epoch, (uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256) transaction) view returns(((uint256,uint256),uint256) signature)
func (_Consensus *ConsensusCaller) GetTransactionAttestation(opts *bind.CallOpts, epoch uint64, transaction SafeTransactionT) (FROSTSignature, error) {
	var out []interface{}
	err := _Consensus.contract.Call(opts, &out, "getTransactionAttestation", epoch, transaction)

	if err != nil {
		return *new(FROSTSignature), err
	}

	out0 := *abi.ConvertType(out[0], new(FROSTSignature)).(*FROSTSignature)

	return out0, err

}

// GetTransactionAttestation is a free data retrieval call binding the contract method 0xe29d3196.
//
// Solidity: function getTransactionAttestation(uint64 epoch, (uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256) transaction) view returns(((uint256,uint256),uint256) signature)
func (_Consensus *ConsensusSession) GetTransactionAttestation(epoch uint64, transaction SafeTransactionT) (FROSTSignature, error) {
	return _Consensus.Contract.GetTransactionAttestation(&_Consensus.CallOpts, epoch, transaction)
}

// GetTransactionAttestation is a free data retrieval call binding the contract method 0xe29d3196.
//
// Solidity: function getTransactionAttestation(uint64 epoch, (uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256) transaction) view returns(((uint256,uint256),uint256) signature)
func (_Consensus *ConsensusCallerSession) GetTransactionAttestation(epoch uint64, transaction SafeTransactionT) (FROSTSignature, error) {
	return _Consensus.Contract.GetTransactionAttestation(&_Consensus.CallOpts, epoch, transaction)
}

// GetTransactionAttestationByHash is a free data retrieval call binding the contract method 0x76eb9e4b.
//
// Solidity: function getTransactionAttestationByHash(uint64 epoch, bytes32 safeTxHash) view returns(((uint256,uint256),uint256) signature)
func (_Consensus *ConsensusCaller) GetTransactionAttestationByHash(opts *bind.CallOpts, epoch uint64, safeTxHash [32]byte) (FROSTSignature, error) {
	var out []interface{}
	err := _Consensus.contract.Call(opts, &out, "getTransactionAttestationByHash", epoch, safeTxHash)

	if err != nil {
		return *new(FROSTSignature), err
	}

	out0 := *abi.ConvertType(out[0], new(FROSTSignature)).(*FROSTSignature)

	return out0, err

}

// GetTransactionAttestationByHash is a free data retrieval call binding the contract method 0x76eb9e4b.
//
// Solidity: function getTransactionAttestationByHash(uint64 epoch, bytes32 safeTxHash) view returns(((uint256,uint256),uint256) signature)
func (_Consensus *ConsensusSession) GetTransactionAttestationByHash(epoch uint64, safeTxHash [32]byte) (FROSTSignature, error) {
	return _Consensus.Contract.GetTransactionAttestationByHash(&_Consensus.CallOpts, epoch, safeTxHash)
}

// GetTransactionAttestationByHash is a free data retrieval call binding the contract method 0x76eb9e4b.
//
// Solidity: function getTransactionAttestationByHash(uint64 epoch, bytes32 safeTxHash) view returns(((uint256,uint256),uint256) signature)
func (_Consensus *ConsensusCallerSession) GetTransactionAttestationByHash(epoch uint64, safeTxHash [32]byte) (FROSTSignature, error) {
	return _Consensus.Contract.GetTransactionAttestationByHash(&_Consensus.CallOpts, epoch, safeTxHash)
}

// GetValidatorStaker is a free data retrieval call binding the contract method 0x110aec04.
//
// Solidity: function getValidatorStaker(address validator) view returns(address staker)
func (_Consensus *ConsensusCaller) GetValidatorStaker(opts *bind.CallOpts, validator common.Address) (common.Address, error) {
	var out []interface{}
	err := _Consensus.contract.Call(opts, &out, "getValidatorStaker", validator)

	if err != nil {
		return *new(common.Address), err
	}

	out0 := *abi.ConvertType(out[0], new(common.Address)).(*common.Address)

	return out0, err

}

// GetValidatorStaker is a free data retrieval call binding the contract method 0x110aec04.
//
// Solidity: function getValidatorStaker(address validator) view returns(address staker)
func (_Consensus *ConsensusSession) GetValidatorStaker(validator common.Address) (common.Address, error) {
	return _Consensus.Contract.GetValidatorStaker(&_Consensus.CallOpts, validator)
}

// GetValidatorStaker is a free data retrieval call binding the contract method 0x110aec04.
//
// Solidity: function getValidatorStaker(address validator) view returns(address staker)
func (_Consensus *ConsensusCallerSession) GetValidatorStaker(validator common.Address) (common.Address, error) {
	return _Consensus.Contract.GetValidatorStaker(&_Consensus.CallOpts, validator)
}

// SupportsInterface is a free data retrieval call binding the contract method 0x01ffc9a7.
//
// Solidity: function supportsInterface(bytes4 interfaceId) pure returns(bool)
func (_Consensus *ConsensusCaller) SupportsInterface(opts *bind.CallOpts, interfaceId [4]byte) (bool, error) {
	var out []interface{}
	err := _Consensus.contract.Call(opts, &out, "supportsInterface", interfaceId)

	if err != nil {
		return *new(bool), err
	}

	out0 := *abi.ConvertType(out[0], new(bool)).(*bool)

	return out0, err

}

// SupportsInterface is a free data retrieval call binding the contract method 0x01ffc9a7.
//
// Solidity: function supportsInterface(bytes4 interfaceId) pure returns(bool)
func (_Consensus *ConsensusSession) SupportsInterface(interfaceId [4]byte) (bool, error) {
	return _Consensus.Contract.SupportsInterface(&_Consensus.CallOpts, interfaceId)
}

// SupportsInterface is a free data retrieval call binding the contract method 0x01ffc9a7.
//
// Solidity: function supportsInterface(bytes4 interfaceId) pure returns(bool)
func (_Consensus *ConsensusCallerSession) SupportsInterface(interfaceId [4]byte) (bool, error) {
	return _Consensus.Contract.SupportsInterface(&_Consensus.CallOpts, interfaceId)
}

// AttestTransaction is a paid mutator transaction binding the contract method 0xaa8d1739.
//
// Solidity: function attestTransaction(uint64 epoch, uint256 chainId, address safe, bytes32 safeTxStructHash, bytes32 signatureId) returns()
func (_Consensus *ConsensusTransactor) AttestTransaction(opts *bind.TransactOpts, epoch uint64, chainId *big.Int, safe common.Address, safeTxStructHash [32]byte, signatureId [32]byte) (*types.Transaction, error) {
	return _Consensus.contract.Transact(opts, "attestTransaction", epoch, chainId, safe, safeTxStructHash, signatureId)
}

// AttestTransaction is a paid mutator transaction binding the contract method 0xaa8d1739.
//
// Solidity: function attestTransaction(uint64 epoch, uint256 chainId, address safe, bytes32 safeTxStructHash, bytes32 signatureId) returns()
func (_Consensus *ConsensusSession) AttestTransaction(epoch uint64, chainId *big.Int, safe common.Address, safeTxStructHash [32]byte, signatureId [32]byte) (*types.Transaction, error) {
	return _Consensus.Contract.AttestTransaction(&_Consensus.TransactOpts, epoch, chainId, safe, safeTxStructHash, signatureId)
}

// AttestTransaction is a paid mutator transaction binding the contract method 0xaa8d1739.
//
// Solidity: function attestTransaction(uint64 epoch, uint256 chainId, address safe, bytes32 safeTxStructHash, bytes32 signatureId) returns()
func (_Consensus *ConsensusTransactorSession) AttestTransaction(epoch uint64, chainId *big.Int, safe common.Address, safeTxStructHash [32]byte, signatureId [32]byte) (*types.Transaction, error) {
	return _Consensus.Contract.AttestTransaction(&_Consensus.TransactOpts, epoch, chainId, safe, safeTxStructHash, signatureId)
}

// OnKeyGenCompleted is a paid mutator transaction binding the contract method 0x39d4d751.
//
// Solidity: function onKeyGenCompleted(bytes32 groupId, bytes context) returns()
func (_Consensus *ConsensusTransactor) OnKeyGenCompleted(opts *bind.TransactOpts, groupId [32]byte, context []byte) (*types.Transaction, error) {
	return _Consensus.contract.Transact(opts, "onKeyGenCompleted", groupId, context)
}

// OnKeyGenCompleted is a paid mutator transaction binding the contract method 0x39d4d751.
//
// Solidity: function onKeyGenCompleted(bytes32 groupId, bytes context) returns()
func (_Consensus *ConsensusSession) OnKeyGenCompleted(groupId [32]byte, context []byte) (*types.Transaction, error) {
	return _Consensus.Contract.OnKeyGenCompleted(&_Consensus.TransactOpts, groupId, context)
}

// OnKeyGenCompleted is a paid mutator transaction binding the contract method 0x39d4d751.
//
// Solidity: function onKeyGenCompleted(bytes32 groupId, bytes context) returns()
func (_Consensus *ConsensusTransactorSession) OnKeyGenCompleted(groupId [32]byte, context []byte) (*types.Transaction, error) {
	return _Consensus.Contract.OnKeyGenCompleted(&_Consensus.TransactOpts, groupId, context)
}

// OnSignCompleted is a paid mutator transaction binding the contract method 0x3f6b9558.
//
// Solidity: function onSignCompleted(bytes32 signatureId, bytes context) returns()
func (_Consensus *ConsensusTransactor) OnSignCompleted(opts *bind.TransactOpts, signatureId [32]byte, context []byte) (*types.Transaction, error) {
	return _Consensus.contract.Transact(opts, "onSignCompleted", signatureId, context)
}

// OnSignCompleted is a paid mutator transaction binding the contract method 0x3f6b9558.
//
// Solidity: function onSignCompleted(bytes32 signatureId, bytes context) returns()
func (_Consensus *ConsensusSession) OnSignCompleted(signatureId [32]byte, context []byte) (*types.Transaction, error) {
	return _Consensus.Contract.OnSignCompleted(&_Consensus.TransactOpts, signatureId, context)
}

// OnSignCompleted is a paid mutator transaction binding the contract method 0x3f6b9558.
//
// Solidity: function onSignCompleted(bytes32 signatureId, bytes context) returns()
func (_Consensus *ConsensusTransactorSession) OnSignCompleted(signatureId [32]byte, context []byte) (*types.Transaction, error) {
	return _Consensus.Contract.OnSignCompleted(&_Consensus.TransactOpts, signatureId, context)
}

// ProposeBasicTransaction is a paid mutator transaction binding the contract method 0xe5582562.
//
// Solidity: function proposeBasicTransaction(uint256 chainId, address safe, address to, uint256 value, bytes data, uint256 nonce) returns(bytes32 safeTxHash)
func (_Consensus *ConsensusTransactor) ProposeBasicTransaction(opts *bind.TransactOpts, chainId *big.Int, safe common.Address, to common.Address, value *big.Int, data []byte, nonce *big.Int) (*types.Transaction, error) {
	return _Consensus.contract.Transact(opts, "proposeBasicTransaction", chainId, safe, to, value, data, nonce)
}

// ProposeBasicTransaction is a paid mutator transaction binding the contract method 0xe5582562.
//
// Solidity: function proposeBasicTransaction(uint256 chainId, address safe, address to, uint256 value, bytes data, uint256 nonce) returns(bytes32 safeTxHash)
func (_Consensus *ConsensusSession) ProposeBasicTransaction(chainId *big.Int, safe common.Address, to common.Address, value *big.Int, data []byte, nonce *big.Int) (*types.Transaction, error) {
	return _Consensus.Contract.ProposeBasicTransaction(&_Consensus.TransactOpts, chainId, safe, to, value, data, nonce)
}

// ProposeBasicTransaction is a paid mutator transaction binding the contract method 0xe5582562.
//
// Solidity: function proposeBasicTransaction(uint256 chainId, address safe, address to, uint256 value, bytes data, uint256 nonce) returns(bytes32 safeTxHash)
func (_Consensus *ConsensusTransactorSession) ProposeBasicTransaction(chainId *big.Int, safe common.Address, to common.Address, value *big.Int, data []byte, nonce *big.Int) (*types.Transaction, error) {
	return _Consensus.Contract.ProposeBasicTransaction(&_Consensus.TransactOpts, chainId, safe, to, value, data, nonce)
}

// ProposeEpoch is a paid mutator transaction binding the contract method 0x2c0aff7a.
//
// Solidity: function proposeEpoch(uint64 proposedEpoch, uint64 rolloverBlock, bytes32 groupId) returns()
func (_Consensus *ConsensusTransactor) ProposeEpoch(opts *bind.TransactOpts, proposedEpoch uint64, rolloverBlock uint64, groupId [32]byte) (*types.Transaction, error) {
	return _Consensus.contract.Transact(opts, "proposeEpoch", proposedEpoch, rolloverBlock, groupId)
}

// ProposeEpoch is a paid mutator transaction binding the contract method 0x2c0aff7a.
//
// Solidity: function proposeEpoch(uint64 proposedEpoch, uint64 rolloverBlock, bytes32 groupId) returns()
func (_Consensus *ConsensusSession) ProposeEpoch(proposedEpoch uint64, rolloverBlock uint64, groupId [32]byte) (*types.Transaction, error) {
	return _Consensus.Contract.ProposeEpoch(&_Consensus.TransactOpts, proposedEpoch, rolloverBlock, groupId)
}

// ProposeEpoch is a paid mutator transaction binding the contract method 0x2c0aff7a.
//
// Solidity: function proposeEpoch(uint64 proposedEpoch, uint64 rolloverBlock, bytes32 groupId) returns()
func (_Consensus *ConsensusTransactorSession) ProposeEpoch(proposedEpoch uint64, rolloverBlock uint64, groupId [32]byte) (*types.Transaction, error) {
	return _Consensus.Contract.ProposeEpoch(&_Consensus.TransactOpts, proposedEpoch, rolloverBlock, groupId)
}

// ProposeTransaction is a paid mutator transaction binding the contract method 0x61ff123c.
//
// Solidity: function proposeTransaction((uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256) transaction) returns(bytes32 safeTxHash)
func (_Consensus *ConsensusTransactor) ProposeTransaction(opts *bind.TransactOpts, transaction SafeTransactionT) (*types.Transaction, error) {
	return _Consensus.contract.Transact(opts, "proposeTransaction", transaction)
}

// ProposeTransaction is a paid mutator transaction binding the contract method 0x61ff123c.
//
// Solidity: function proposeTransaction((uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256) transaction) returns(bytes32 safeTxHash)
func (_Consensus *ConsensusSession) ProposeTransaction(transaction SafeTransactionT) (*types.Transaction, error) {
	return _Consensus.Contract.ProposeTransaction(&_Consensus.TransactOpts, transaction)
}

// ProposeTransaction is a paid mutator transaction binding the contract method 0x61ff123c.
//
// Solidity: function proposeTransaction((uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256) transaction) returns(bytes32 safeTxHash)
func (_Consensus *ConsensusTransactorSession) ProposeTransaction(transaction SafeTransactionT) (*types.Transaction, error) {
	return _Consensus.Contract.ProposeTransaction(&_Consensus.TransactOpts, transaction)
}

// SetValidatorStaker is a paid mutator transaction binding the contract method 0xbbce66a6.
//
// Solidity: function setValidatorStaker(address staker) returns()
func (_Consensus *ConsensusTransactor) SetValidatorStaker(opts *bind.TransactOpts, staker common.Address) (*types.Transaction, error) {
	return _Consensus.contract.Transact(opts, "setValidatorStaker", staker)
}

// SetValidatorStaker is a paid mutator transaction binding the contract method 0xbbce66a6.
//
// Solidity: function setValidatorStaker(address staker) returns()
func (_Consensus *ConsensusSession) SetValidatorStaker(staker common.Address) (*types.Transaction, error) {
	return _Consensus.Contract.SetValidatorStaker(&_Consensus.TransactOpts, staker)
}

// SetValidatorStaker is a paid mutator transaction binding the contract method 0xbbce66a6.
//
// Solidity: function setValidatorStaker(address staker) returns()
func (_Consensus *ConsensusTransactorSession) SetValidatorStaker(staker common.Address) (*types.Transaction, error) {
	return _Consensus.Contract.SetValidatorStaker(&_Consensus.TransactOpts, staker)
}

// StageEpoch is a paid mutator transaction binding the contract method 0xea5eeafa.
//
// Solidity: function stageEpoch(uint64 proposedEpoch, uint64 rolloverBlock, bytes32 groupId, bytes32 signatureId) returns()
func (_Consensus *ConsensusTransactor) StageEpoch(opts *bind.TransactOpts, proposedEpoch uint64, rolloverBlock uint64, groupId [32]byte, signatureId [32]byte) (*types.Transaction, error) {
	return _Consensus.contract.Transact(opts, "stageEpoch", proposedEpoch, rolloverBlock, groupId, signatureId)
}

// StageEpoch is a paid mutator transaction binding the contract method 0xea5eeafa.
//
// Solidity: function stageEpoch(uint64 proposedEpoch, uint64 rolloverBlock, bytes32 groupId, bytes32 signatureId) returns()
func (_Consensus *ConsensusSession) StageEpoch(proposedEpoch uint64, rolloverBlock uint64, groupId [32]byte, signatureId [32]byte) (*types.Transaction, error) {
	return _Consensus.Contract.StageEpoch(&_Consensus.TransactOpts, proposedEpoch, rolloverBlock, groupId, signatureId)
}

// StageEpoch is a paid mutator transaction binding the contract method 0xea5eeafa.
//
// Solidity: function stageEpoch(uint64 proposedEpoch, uint64 rolloverBlock, bytes32 groupId, bytes32 signatureId) returns()
func (_Consensus *ConsensusTransactorSession) StageEpoch(proposedEpoch uint64, rolloverBlock uint64, groupId [32]byte, signatureId [32]byte) (*types.Transaction, error) {
	return _Consensus.Contract.StageEpoch(&_Consensus.TransactOpts, proposedEpoch, rolloverBlock, groupId, signatureId)
}

// ConsensusEpochProposedIterator is returned from FilterEpochProposed and is used to iterate over the raw logs and unpacked data for EpochProposed events raised by the Consensus contract.
type ConsensusEpochProposedIterator struct {
	Event *ConsensusEpochProposed // Event containing the contract specifics and raw log

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
func (it *ConsensusEpochProposedIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(ConsensusEpochProposed)
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
		it.Event = new(ConsensusEpochProposed)
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
func (it *ConsensusEpochProposedIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *ConsensusEpochProposedIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// ConsensusEpochProposed represents a EpochProposed event raised by the Consensus contract.
type ConsensusEpochProposed struct {
	ActiveEpoch   uint64
	ProposedEpoch uint64
	RolloverBlock uint64
	GroupId       [32]byte
	GroupKey      Secp256k1Point
	Raw           types.Log // Blockchain specific contextual infos
}

// FilterEpochProposed is a free log retrieval operation binding the contract event 0x6be947707aed4e645f9b39db04fcc849c9a98cc44427e3d4bfc40258b26b7b13.
//
// Solidity: event EpochProposed(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, bytes32 groupId, (uint256,uint256) groupKey)
func (_Consensus *ConsensusFilterer) FilterEpochProposed(opts *bind.FilterOpts, activeEpoch []uint64, proposedEpoch []uint64) (*ConsensusEpochProposedIterator, error) {

	var activeEpochRule []interface{}
	for _, activeEpochItem := range activeEpoch {
		activeEpochRule = append(activeEpochRule, activeEpochItem)
	}
	var proposedEpochRule []interface{}
	for _, proposedEpochItem := range proposedEpoch {
		proposedEpochRule = append(proposedEpochRule, proposedEpochItem)
	}

	logs, sub, err := _Consensus.contract.FilterLogs(opts, "EpochProposed", activeEpochRule, proposedEpochRule)
	if err != nil {
		return nil, err
	}
	return &ConsensusEpochProposedIterator{contract: _Consensus.contract, event: "EpochProposed", logs: logs, sub: sub}, nil
}

// WatchEpochProposed is a free log subscription operation binding the contract event 0x6be947707aed4e645f9b39db04fcc849c9a98cc44427e3d4bfc40258b26b7b13.
//
// Solidity: event EpochProposed(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, bytes32 groupId, (uint256,uint256) groupKey)
func (_Consensus *ConsensusFilterer) WatchEpochProposed(opts *bind.WatchOpts, sink chan<- *ConsensusEpochProposed, activeEpoch []uint64, proposedEpoch []uint64) (event.Subscription, error) {

	var activeEpochRule []interface{}
	for _, activeEpochItem := range activeEpoch {
		activeEpochRule = append(activeEpochRule, activeEpochItem)
	}
	var proposedEpochRule []interface{}
	for _, proposedEpochItem := range proposedEpoch {
		proposedEpochRule = append(proposedEpochRule, proposedEpochItem)
	}

	logs, sub, err := _Consensus.contract.WatchLogs(opts, "EpochProposed", activeEpochRule, proposedEpochRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(ConsensusEpochProposed)
				if err := _Consensus.contract.UnpackLog(event, "EpochProposed", log); err != nil {
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

// ParseEpochProposed is a log parse operation binding the contract event 0x6be947707aed4e645f9b39db04fcc849c9a98cc44427e3d4bfc40258b26b7b13.
//
// Solidity: event EpochProposed(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, bytes32 groupId, (uint256,uint256) groupKey)
func (_Consensus *ConsensusFilterer) ParseEpochProposed(log types.Log) (*ConsensusEpochProposed, error) {
	event := new(ConsensusEpochProposed)
	if err := _Consensus.contract.UnpackLog(event, "EpochProposed", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// ConsensusEpochRolledOverIterator is returned from FilterEpochRolledOver and is used to iterate over the raw logs and unpacked data for EpochRolledOver events raised by the Consensus contract.
type ConsensusEpochRolledOverIterator struct {
	Event *ConsensusEpochRolledOver // Event containing the contract specifics and raw log

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
func (it *ConsensusEpochRolledOverIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(ConsensusEpochRolledOver)
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
		it.Event = new(ConsensusEpochRolledOver)
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
func (it *ConsensusEpochRolledOverIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *ConsensusEpochRolledOverIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// ConsensusEpochRolledOver represents a EpochRolledOver event raised by the Consensus contract.
type ConsensusEpochRolledOver struct {
	NewActiveEpoch uint64
	Raw            types.Log // Blockchain specific contextual infos
}

// FilterEpochRolledOver is a free log retrieval operation binding the contract event 0xa2b2d9304dd0889a2116f91f2f18de6604464e1f5908d758c2e4100d95b65c58.
//
// Solidity: event EpochRolledOver(uint64 indexed newActiveEpoch)
func (_Consensus *ConsensusFilterer) FilterEpochRolledOver(opts *bind.FilterOpts, newActiveEpoch []uint64) (*ConsensusEpochRolledOverIterator, error) {

	var newActiveEpochRule []interface{}
	for _, newActiveEpochItem := range newActiveEpoch {
		newActiveEpochRule = append(newActiveEpochRule, newActiveEpochItem)
	}

	logs, sub, err := _Consensus.contract.FilterLogs(opts, "EpochRolledOver", newActiveEpochRule)
	if err != nil {
		return nil, err
	}
	return &ConsensusEpochRolledOverIterator{contract: _Consensus.contract, event: "EpochRolledOver", logs: logs, sub: sub}, nil
}

// WatchEpochRolledOver is a free log subscription operation binding the contract event 0xa2b2d9304dd0889a2116f91f2f18de6604464e1f5908d758c2e4100d95b65c58.
//
// Solidity: event EpochRolledOver(uint64 indexed newActiveEpoch)
func (_Consensus *ConsensusFilterer) WatchEpochRolledOver(opts *bind.WatchOpts, sink chan<- *ConsensusEpochRolledOver, newActiveEpoch []uint64) (event.Subscription, error) {

	var newActiveEpochRule []interface{}
	for _, newActiveEpochItem := range newActiveEpoch {
		newActiveEpochRule = append(newActiveEpochRule, newActiveEpochItem)
	}

	logs, sub, err := _Consensus.contract.WatchLogs(opts, "EpochRolledOver", newActiveEpochRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(ConsensusEpochRolledOver)
				if err := _Consensus.contract.UnpackLog(event, "EpochRolledOver", log); err != nil {
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

// ParseEpochRolledOver is a log parse operation binding the contract event 0xa2b2d9304dd0889a2116f91f2f18de6604464e1f5908d758c2e4100d95b65c58.
//
// Solidity: event EpochRolledOver(uint64 indexed newActiveEpoch)
func (_Consensus *ConsensusFilterer) ParseEpochRolledOver(log types.Log) (*ConsensusEpochRolledOver, error) {
	event := new(ConsensusEpochRolledOver)
	if err := _Consensus.contract.UnpackLog(event, "EpochRolledOver", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// ConsensusEpochStagedIterator is returned from FilterEpochStaged and is used to iterate over the raw logs and unpacked data for EpochStaged events raised by the Consensus contract.
type ConsensusEpochStagedIterator struct {
	Event *ConsensusEpochStaged // Event containing the contract specifics and raw log

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
func (it *ConsensusEpochStagedIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(ConsensusEpochStaged)
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
		it.Event = new(ConsensusEpochStaged)
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
func (it *ConsensusEpochStagedIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *ConsensusEpochStagedIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// ConsensusEpochStaged represents a EpochStaged event raised by the Consensus contract.
type ConsensusEpochStaged struct {
	ActiveEpoch   uint64
	ProposedEpoch uint64
	RolloverBlock uint64
	GroupId       [32]byte
	GroupKey      Secp256k1Point
	SignatureId   [32]byte
	Attestation   FROSTSignature
	Raw           types.Log // Blockchain specific contextual infos
}

// FilterEpochStaged is a free log retrieval operation binding the contract event 0xd22757d0334b80219cf27dedaf82211008f84fc412a29a27e3000dc1c6160b86.
//
// Solidity: event EpochStaged(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, bytes32 groupId, (uint256,uint256) groupKey, bytes32 signatureId, ((uint256,uint256),uint256) attestation)
func (_Consensus *ConsensusFilterer) FilterEpochStaged(opts *bind.FilterOpts, activeEpoch []uint64, proposedEpoch []uint64) (*ConsensusEpochStagedIterator, error) {

	var activeEpochRule []interface{}
	for _, activeEpochItem := range activeEpoch {
		activeEpochRule = append(activeEpochRule, activeEpochItem)
	}
	var proposedEpochRule []interface{}
	for _, proposedEpochItem := range proposedEpoch {
		proposedEpochRule = append(proposedEpochRule, proposedEpochItem)
	}

	logs, sub, err := _Consensus.contract.FilterLogs(opts, "EpochStaged", activeEpochRule, proposedEpochRule)
	if err != nil {
		return nil, err
	}
	return &ConsensusEpochStagedIterator{contract: _Consensus.contract, event: "EpochStaged", logs: logs, sub: sub}, nil
}

// WatchEpochStaged is a free log subscription operation binding the contract event 0xd22757d0334b80219cf27dedaf82211008f84fc412a29a27e3000dc1c6160b86.
//
// Solidity: event EpochStaged(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, bytes32 groupId, (uint256,uint256) groupKey, bytes32 signatureId, ((uint256,uint256),uint256) attestation)
func (_Consensus *ConsensusFilterer) WatchEpochStaged(opts *bind.WatchOpts, sink chan<- *ConsensusEpochStaged, activeEpoch []uint64, proposedEpoch []uint64) (event.Subscription, error) {

	var activeEpochRule []interface{}
	for _, activeEpochItem := range activeEpoch {
		activeEpochRule = append(activeEpochRule, activeEpochItem)
	}
	var proposedEpochRule []interface{}
	for _, proposedEpochItem := range proposedEpoch {
		proposedEpochRule = append(proposedEpochRule, proposedEpochItem)
	}

	logs, sub, err := _Consensus.contract.WatchLogs(opts, "EpochStaged", activeEpochRule, proposedEpochRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(ConsensusEpochStaged)
				if err := _Consensus.contract.UnpackLog(event, "EpochStaged", log); err != nil {
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

// ParseEpochStaged is a log parse operation binding the contract event 0xd22757d0334b80219cf27dedaf82211008f84fc412a29a27e3000dc1c6160b86.
//
// Solidity: event EpochStaged(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, bytes32 groupId, (uint256,uint256) groupKey, bytes32 signatureId, ((uint256,uint256),uint256) attestation)
func (_Consensus *ConsensusFilterer) ParseEpochStaged(log types.Log) (*ConsensusEpochStaged, error) {
	event := new(ConsensusEpochStaged)
	if err := _Consensus.contract.UnpackLog(event, "EpochStaged", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// ConsensusTransactionAttestedIterator is returned from FilterTransactionAttested and is used to iterate over the raw logs and unpacked data for TransactionAttested events raised by the Consensus contract.
type ConsensusTransactionAttestedIterator struct {
	Event *ConsensusTransactionAttested // Event containing the contract specifics and raw log

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
func (it *ConsensusTransactionAttestedIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(ConsensusTransactionAttested)
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
		it.Event = new(ConsensusTransactionAttested)
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
func (it *ConsensusTransactionAttestedIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *ConsensusTransactionAttestedIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// ConsensusTransactionAttested represents a TransactionAttested event raised by the Consensus contract.
type ConsensusTransactionAttested struct {
	SafeTxHash  [32]byte
	ChainId     *big.Int
	Safe        common.Address
	Epoch       uint64
	SignatureId [32]byte
	Attestation FROSTSignature
	Raw         types.Log // Blockchain specific contextual infos
}

// FilterTransactionAttested is a free log retrieval operation binding the contract event 0x72272729e643703db011cc155474c30d652f1a68712d921cc263a881efd7bce6.
//
// Solidity: event TransactionAttested(bytes32 indexed safeTxHash, uint256 indexed chainId, address indexed safe, uint64 epoch, bytes32 signatureId, ((uint256,uint256),uint256) attestation)
func (_Consensus *ConsensusFilterer) FilterTransactionAttested(opts *bind.FilterOpts, safeTxHash [][32]byte, chainId []*big.Int, safe []common.Address) (*ConsensusTransactionAttestedIterator, error) {

	var safeTxHashRule []interface{}
	for _, safeTxHashItem := range safeTxHash {
		safeTxHashRule = append(safeTxHashRule, safeTxHashItem)
	}
	var chainIdRule []interface{}
	for _, chainIdItem := range chainId {
		chainIdRule = append(chainIdRule, chainIdItem)
	}
	var safeRule []interface{}
	for _, safeItem := range safe {
		safeRule = append(safeRule, safeItem)
	}

	logs, sub, err := _Consensus.contract.FilterLogs(opts, "TransactionAttested", safeTxHashRule, chainIdRule, safeRule)
	if err != nil {
		return nil, err
	}
	return &ConsensusTransactionAttestedIterator{contract: _Consensus.contract, event: "TransactionAttested", logs: logs, sub: sub}, nil
}

// WatchTransactionAttested is a free log subscription operation binding the contract event 0x72272729e643703db011cc155474c30d652f1a68712d921cc263a881efd7bce6.
//
// Solidity: event TransactionAttested(bytes32 indexed safeTxHash, uint256 indexed chainId, address indexed safe, uint64 epoch, bytes32 signatureId, ((uint256,uint256),uint256) attestation)
func (_Consensus *ConsensusFilterer) WatchTransactionAttested(opts *bind.WatchOpts, sink chan<- *ConsensusTransactionAttested, safeTxHash [][32]byte, chainId []*big.Int, safe []common.Address) (event.Subscription, error) {

	var safeTxHashRule []interface{}
	for _, safeTxHashItem := range safeTxHash {
		safeTxHashRule = append(safeTxHashRule, safeTxHashItem)
	}
	var chainIdRule []interface{}
	for _, chainIdItem := range chainId {
		chainIdRule = append(chainIdRule, chainIdItem)
	}
	var safeRule []interface{}
	for _, safeItem := range safe {
		safeRule = append(safeRule, safeItem)
	}

	logs, sub, err := _Consensus.contract.WatchLogs(opts, "TransactionAttested", safeTxHashRule, chainIdRule, safeRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(ConsensusTransactionAttested)
				if err := _Consensus.contract.UnpackLog(event, "TransactionAttested", log); err != nil {
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

// ParseTransactionAttested is a log parse operation binding the contract event 0x72272729e643703db011cc155474c30d652f1a68712d921cc263a881efd7bce6.
//
// Solidity: event TransactionAttested(bytes32 indexed safeTxHash, uint256 indexed chainId, address indexed safe, uint64 epoch, bytes32 signatureId, ((uint256,uint256),uint256) attestation)
func (_Consensus *ConsensusFilterer) ParseTransactionAttested(log types.Log) (*ConsensusTransactionAttested, error) {
	event := new(ConsensusTransactionAttested)
	if err := _Consensus.contract.UnpackLog(event, "TransactionAttested", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// ConsensusTransactionProposedIterator is returned from FilterTransactionProposed and is used to iterate over the raw logs and unpacked data for TransactionProposed events raised by the Consensus contract.
type ConsensusTransactionProposedIterator struct {
	Event *ConsensusTransactionProposed // Event containing the contract specifics and raw log

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
func (it *ConsensusTransactionProposedIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(ConsensusTransactionProposed)
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
		it.Event = new(ConsensusTransactionProposed)
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
func (it *ConsensusTransactionProposedIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *ConsensusTransactionProposedIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// ConsensusTransactionProposed represents a TransactionProposed event raised by the Consensus contract.
type ConsensusTransactionProposed struct {
	SafeTxHash  [32]byte
	ChainId     *big.Int
	Safe        common.Address
	Epoch       uint64
	Transaction SafeTransactionT
	Raw         types.Log // Blockchain specific contextual infos
}

// FilterTransactionProposed is a free log retrieval operation binding the contract event 0xe7427c304b80147290ec649ec1d8881f5fa455e85ba79ecb7dbfc58a56ea0906.
//
// Solidity: event TransactionProposed(bytes32 indexed safeTxHash, uint256 indexed chainId, address indexed safe, uint64 epoch, (uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256) transaction)
func (_Consensus *ConsensusFilterer) FilterTransactionProposed(opts *bind.FilterOpts, safeTxHash [][32]byte, chainId []*big.Int, safe []common.Address) (*ConsensusTransactionProposedIterator, error) {

	var safeTxHashRule []interface{}
	for _, safeTxHashItem := range safeTxHash {
		safeTxHashRule = append(safeTxHashRule, safeTxHashItem)
	}
	var chainIdRule []interface{}
	for _, chainIdItem := range chainId {
		chainIdRule = append(chainIdRule, chainIdItem)
	}
	var safeRule []interface{}
	for _, safeItem := range safe {
		safeRule = append(safeRule, safeItem)
	}

	logs, sub, err := _Consensus.contract.FilterLogs(opts, "TransactionProposed", safeTxHashRule, chainIdRule, safeRule)
	if err != nil {
		return nil, err
	}
	return &ConsensusTransactionProposedIterator{contract: _Consensus.contract, event: "TransactionProposed", logs: logs, sub: sub}, nil
}

// WatchTransactionProposed is a free log subscription operation binding the contract event 0xe7427c304b80147290ec649ec1d8881f5fa455e85ba79ecb7dbfc58a56ea0906.
//
// Solidity: event TransactionProposed(bytes32 indexed safeTxHash, uint256 indexed chainId, address indexed safe, uint64 epoch, (uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256) transaction)
func (_Consensus *ConsensusFilterer) WatchTransactionProposed(opts *bind.WatchOpts, sink chan<- *ConsensusTransactionProposed, safeTxHash [][32]byte, chainId []*big.Int, safe []common.Address) (event.Subscription, error) {

	var safeTxHashRule []interface{}
	for _, safeTxHashItem := range safeTxHash {
		safeTxHashRule = append(safeTxHashRule, safeTxHashItem)
	}
	var chainIdRule []interface{}
	for _, chainIdItem := range chainId {
		chainIdRule = append(chainIdRule, chainIdItem)
	}
	var safeRule []interface{}
	for _, safeItem := range safe {
		safeRule = append(safeRule, safeItem)
	}

	logs, sub, err := _Consensus.contract.WatchLogs(opts, "TransactionProposed", safeTxHashRule, chainIdRule, safeRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(ConsensusTransactionProposed)
				if err := _Consensus.contract.UnpackLog(event, "TransactionProposed", log); err != nil {
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

// ParseTransactionProposed is a log parse operation binding the contract event 0xe7427c304b80147290ec649ec1d8881f5fa455e85ba79ecb7dbfc58a56ea0906.
//
// Solidity: event TransactionProposed(bytes32 indexed safeTxHash, uint256 indexed chainId, address indexed safe, uint64 epoch, (uint256,address,address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256) transaction)
func (_Consensus *ConsensusFilterer) ParseTransactionProposed(log types.Log) (*ConsensusTransactionProposed, error) {
	event := new(ConsensusTransactionProposed)
	if err := _Consensus.contract.UnpackLog(event, "TransactionProposed", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}

// ConsensusValidatorStakerSetIterator is returned from FilterValidatorStakerSet and is used to iterate over the raw logs and unpacked data for ValidatorStakerSet events raised by the Consensus contract.
type ConsensusValidatorStakerSetIterator struct {
	Event *ConsensusValidatorStakerSet // Event containing the contract specifics and raw log

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
func (it *ConsensusValidatorStakerSetIterator) Next() bool {
	// If the iterator failed, stop iterating
	if it.fail != nil {
		return false
	}
	// If the iterator completed, deliver directly whatever's available
	if it.done {
		select {
		case log := <-it.logs:
			it.Event = new(ConsensusValidatorStakerSet)
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
		it.Event = new(ConsensusValidatorStakerSet)
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
func (it *ConsensusValidatorStakerSetIterator) Error() error {
	return it.fail
}

// Close terminates the iteration process, releasing any pending underlying
// resources.
func (it *ConsensusValidatorStakerSetIterator) Close() error {
	it.sub.Unsubscribe()
	return nil
}

// ConsensusValidatorStakerSet represents a ValidatorStakerSet event raised by the Consensus contract.
type ConsensusValidatorStakerSet struct {
	Validator common.Address
	Staker    common.Address
	Raw       types.Log // Blockchain specific contextual infos
}

// FilterValidatorStakerSet is a free log retrieval operation binding the contract event 0xf0a005469b11e09422034ffe29da663a2969918f3663a27fae9fd5eed38778bb.
//
// Solidity: event ValidatorStakerSet(address indexed validator, address staker)
func (_Consensus *ConsensusFilterer) FilterValidatorStakerSet(opts *bind.FilterOpts, validator []common.Address) (*ConsensusValidatorStakerSetIterator, error) {

	var validatorRule []interface{}
	for _, validatorItem := range validator {
		validatorRule = append(validatorRule, validatorItem)
	}

	logs, sub, err := _Consensus.contract.FilterLogs(opts, "ValidatorStakerSet", validatorRule)
	if err != nil {
		return nil, err
	}
	return &ConsensusValidatorStakerSetIterator{contract: _Consensus.contract, event: "ValidatorStakerSet", logs: logs, sub: sub}, nil
}

// WatchValidatorStakerSet is a free log subscription operation binding the contract event 0xf0a005469b11e09422034ffe29da663a2969918f3663a27fae9fd5eed38778bb.
//
// Solidity: event ValidatorStakerSet(address indexed validator, address staker)
func (_Consensus *ConsensusFilterer) WatchValidatorStakerSet(opts *bind.WatchOpts, sink chan<- *ConsensusValidatorStakerSet, validator []common.Address) (event.Subscription, error) {

	var validatorRule []interface{}
	for _, validatorItem := range validator {
		validatorRule = append(validatorRule, validatorItem)
	}

	logs, sub, err := _Consensus.contract.WatchLogs(opts, "ValidatorStakerSet", validatorRule)
	if err != nil {
		return nil, err
	}
	return event.NewSubscription(func(quit <-chan struct{}) error {
		defer sub.Unsubscribe()
		for {
			select {
			case log := <-logs:
				// New log arrived, parse the event and forward to the user
				event := new(ConsensusValidatorStakerSet)
				if err := _Consensus.contract.UnpackLog(event, "ValidatorStakerSet", log); err != nil {
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

// ParseValidatorStakerSet is a log parse operation binding the contract event 0xf0a005469b11e09422034ffe29da663a2969918f3663a27fae9fd5eed38778bb.
//
// Solidity: event ValidatorStakerSet(address indexed validator, address staker)
func (_Consensus *ConsensusFilterer) ParseValidatorStakerSet(log types.Log) (*ConsensusValidatorStakerSet, error) {
	event := new(ConsensusValidatorStakerSet)
	if err := _Consensus.contract.UnpackLog(event, "ValidatorStakerSet", log); err != nil {
		return nil, err
	}
	event.Raw = log
	return event, nil
}
