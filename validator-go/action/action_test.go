package action

import (
	"context"
	"errors"
	"math/big"
	"testing"

	"github.com/ethereum/go-ethereum/accounts/abi/bind"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/crypto"
	"github.com/safe-research/safenet/validator-go/contracts/coordinator"
)

// mockCoordinator implements coordinatorTransactor, recording the last method
// called and optionally returning a configured error.
type mockCoordinator struct {
	lastMethod string
	returnErr  error
}

func (m *mockCoordinator) KeyGenAndCommit(_ *bind.TransactOpts, _ [32]byte, _ uint16, _ uint16, _ [32]byte, _ [][32]byte, _ coordinator.FROSTCoordinatorKeyGenCommitment) (*types.Transaction, error) {
	m.lastMethod = "KeyGenAndCommit"
	if m.returnErr != nil {
		return nil, m.returnErr
	}
	return types.NewTx(&types.DynamicFeeTx{}), nil
}

func (m *mockCoordinator) KeyGenSecretShare(_ *bind.TransactOpts, _ [32]byte, _ coordinator.FROSTCoordinatorKeyGenSecretShare) (*types.Transaction, error) {
	m.lastMethod = "KeyGenSecretShare"
	if m.returnErr != nil {
		return nil, m.returnErr
	}
	return types.NewTx(&types.DynamicFeeTx{}), nil
}

func (m *mockCoordinator) KeyGenConfirm(_ *bind.TransactOpts, _ [32]byte) (*types.Transaction, error) {
	m.lastMethod = "KeyGenConfirm"
	if m.returnErr != nil {
		return nil, m.returnErr
	}
	return types.NewTx(&types.DynamicFeeTx{}), nil
}

// newTestHandler builds a Handler with a mock contract and a freshly generated key.
func newTestHandler(t *testing.T, mock *mockCoordinator) *Handler {
	t.Helper()
	key, err := crypto.GenerateKey()
	if err != nil {
		t.Fatalf("generate key: %v", err)
	}
	signer, err := bind.NewKeyedTransactorWithChainID(key, big.NewInt(31337))
	if err != nil {
		t.Fatalf("create signer: %v", err)
	}
	return &Handler{contract: mock, signer: *signer}
}

func TestSendKeyGenAndCommit(t *testing.T) {
	mock := &mockCoordinator{}
	h := newTestHandler(t, mock)

	if _, err := h.Send(context.Background(), KeyGenAndCommit{Count: 3, Threshold: 2}); err != nil {
		t.Fatalf("Send: %v", err)
	}
	if mock.lastMethod != "KeyGenAndCommit" {
		t.Errorf("expected KeyGenAndCommit, got %q", mock.lastMethod)
	}
}

func TestSendKeyGenSecretShare(t *testing.T) {
	mock := &mockCoordinator{}
	h := newTestHandler(t, mock)

	if _, err := h.Send(context.Background(), KeyGenSecretShare{}); err != nil {
		t.Fatalf("Send: %v", err)
	}
	if mock.lastMethod != "KeyGenSecretShare" {
		t.Errorf("expected KeyGenSecretShare, got %q", mock.lastMethod)
	}
}

func TestSendKeyGenConfirm(t *testing.T) {
	mock := &mockCoordinator{}
	h := newTestHandler(t, mock)

	if _, err := h.Send(context.Background(), KeyGenConfirm{}); err != nil {
		t.Fatalf("Send: %v", err)
	}
	if mock.lastMethod != "KeyGenConfirm" {
		t.Errorf("expected KeyGenConfirm, got %q", mock.lastMethod)
	}
}

func TestSendNilActionReturnsError(t *testing.T) {
	h := newTestHandler(t, &mockCoordinator{})
	var a Action // nil interface value — hits the default branch
	if _, err := h.Send(context.Background(), a); err == nil {
		t.Error("expected error for nil action, got nil")
	}
}

func TestSendPropagatesContractError(t *testing.T) {
	want := errors.New("contract error")
	mock := &mockCoordinator{returnErr: want}
	h := newTestHandler(t, mock)

	_, err := h.Send(context.Background(), KeyGenConfirm{})
	if !errors.Is(err, want) {
		t.Errorf("Send: got %v, want %v", err, want)
	}
}

func TestFromAddressDerivedFromKey(t *testing.T) {
	key, err := crypto.GenerateKey()
	if err != nil {
		t.Fatalf("generate key: %v", err)
	}
	signer, err := bind.NewKeyedTransactorWithChainID(key, big.NewInt(1))
	if err != nil {
		t.Fatalf("signer: %v", err)
	}
	h := &Handler{contract: &mockCoordinator{}, signer: *signer}

	if h.From() == (common.Address{}) {
		t.Error("From() returned zero address")
	}
	if h.From() != signer.From {
		t.Errorf("From() = %s, want %s", h.From(), signer.From)
	}
}
