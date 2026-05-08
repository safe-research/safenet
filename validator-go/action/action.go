package action

import (
	"context"
	"crypto/ecdsa"
	"fmt"
	"math/big"

	"github.com/ethereum/go-ethereum/accounts/abi/bind"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/ethclient"
	"github.com/safe-research/safenet/validator-go/contracts/coordinator"
)

// Action is a marker interface for the three genesis keygen on-chain calls.
type Action interface {
	isAction()
}

// KeyGenAndCommit initiates the genesis DKG ceremony and commits the round-1
// payload. Emitted when a matching KeyGen coordinator event is received.
type KeyGenAndCommit struct {
	Participants [32]byte   // Merkle root of the participant set
	Count        uint16     // Total number of participants
	Threshold    uint16     // Minimum signers required
	Context      [32]byte   // Genesis salt (DKG context)
	Proof        [][32]byte // Merkle proof for this validator's participant entry
	Commitment   coordinator.FROSTCoordinatorKeyGenCommitment
}

func (KeyGenAndCommit) isAction() {}

// KeyGenSecretShare broadcasts the round-2 ECDH-encrypted secret shares.
// Emitted once all participants' commitments have been collected.
type KeyGenSecretShare struct {
	GroupID [32]byte
	Share   coordinator.FROSTCoordinatorKeyGenSecretShare
}

func (KeyGenSecretShare) isAction() {}

// KeyGenConfirm confirms that the validator's signing share is ready.
// Emitted once all participants' secret shares have been collected.
type KeyGenConfirm struct {
	GroupID [32]byte
}

func (KeyGenConfirm) isAction() {}

// coordinatorTransactor is the write-only subset of *coordinator.FROSTCoordinator
// used by the Handler. Defined as an interface to allow test substitution.
type coordinatorTransactor interface {
	KeyGenAndCommit(*bind.TransactOpts, [32]byte, uint16, uint16, [32]byte, [][32]byte, coordinator.FROSTCoordinatorKeyGenCommitment) (*types.Transaction, error)
	KeyGenSecretShare(*bind.TransactOpts, [32]byte, coordinator.FROSTCoordinatorKeyGenSecretShare) (*types.Transaction, error)
	KeyGenConfirm(*bind.TransactOpts, [32]byte) (*types.Transaction, error)
}

// Handler builds, signs, and broadcasts EIP-1559 transactions for the three
// genesis keygen actions.
type Handler struct {
	contract coordinatorTransactor
	signer   bind.TransactOpts
}

// NewHandler creates a Handler that sends transactions from the address derived
// from privateKey against the coordinator contract at coordAddr on chainID.
func NewHandler(client *ethclient.Client, coordAddr common.Address, privateKey *ecdsa.PrivateKey, chainID *big.Int) (*Handler, error) {
	contract, err := coordinator.NewFROSTCoordinator(coordAddr, client)
	if err != nil {
		return nil, fmt.Errorf("bind coordinator: %w", err)
	}
	signer, err := bind.NewKeyedTransactorWithChainID(privateKey, chainID)
	if err != nil {
		return nil, fmt.Errorf("create signer: %w", err)
	}
	return &Handler{contract: contract, signer: *signer}, nil
}

// Send builds, signs, and broadcasts a the action as an EIP-1559 transaction.
// Returns the transaction hash on success.
func (h *Handler) Send(ctx context.Context, a Action) (common.Hash, error) {
	opts := h.signer // copy; sets fresh context without mutating the base opts
	opts.Context = ctx

	var (
		tx  *types.Transaction
		err error
	)
	switch a := a.(type) {
	case KeyGenAndCommit:
		tx, err = h.contract.KeyGenAndCommit(&opts, a.Participants, a.Count, a.Threshold, a.Context, a.Proof, a.Commitment)
	case KeyGenSecretShare:
		tx, err = h.contract.KeyGenSecretShare(&opts, a.GroupID, a.Share)
	case KeyGenConfirm:
		tx, err = h.contract.KeyGenConfirm(&opts, a.GroupID)
	default:
		return common.Hash{}, fmt.Errorf("unknown action type %T", a)
	}
	if err != nil {
		return common.Hash{}, err
	}
	return tx.Hash(), nil
}

// From returns the Ethereum address that signs and pays for actions.
func (h *Handler) From() common.Address {
	return h.signer.From
}
