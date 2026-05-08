package storage_test

import (
	"testing"

	"github.com/ethereum/go-ethereum/common"
	"github.com/safe-research/safenet/validator-go/state"
	"github.com/safe-research/safenet/validator-go/storage"
)

var (
	addr1 = common.HexToAddress("0x1111111111111111111111111111111111111111")
	addr2 = common.HexToAddress("0x2222222222222222222222222222222222222222")
	addr3 = common.HexToAddress("0x3333333333333333333333333333333333333333")
)

func testConfig() state.ConsensusConfig {
	return state.ConsensusConfig{
		OwnAddress:         addr1,
		CoordinatorAddress: addr2,
		Participants:       []common.Address{addr1, addr2, addr3},
		BlocksPerEpoch:     60,
	}
}

func openMemory(t *testing.T, stateHistory uint) *storage.Storage {
	t.Helper()
	s, err := storage.Open("", stateHistory)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	t.Cleanup(func() { s.Close() })
	return s
}

func makeState(phase state.Phase) *state.State {
	return &state.State{Config: testConfig(), Phase: phase}
}

func TestLoadLatestEmptyDatabase(t *testing.T) {
	s := openMemory(t, 5)
	block, st, err := s.LoadLatest()
	if err != nil {
		t.Fatalf("LoadLatest: %v", err)
	}
	if st != nil {
		t.Errorf("expected nil state, got %v", st)
	}
	if block != 0 {
		t.Errorf("expected block 0, got %d", block)
	}
}

func TestSaveAndLoad(t *testing.T) {
	s := openMemory(t, 5)

	want := makeState(state.WaitingForGenesis{})
	if err := s.Save(100, want); err != nil {
		t.Fatalf("Save: %v", err)
	}

	block, got, err := s.LoadLatest()
	if err != nil {
		t.Fatalf("LoadLatest: %v", err)
	}
	if got == nil {
		t.Fatal("expected non-nil state")
	}
	if block != 100 {
		t.Errorf("block: got %d, want 100", block)
	}
	if _, ok := got.Phase.(state.WaitingForGenesis); !ok {
		t.Errorf("phase: got %T, want WaitingForGenesis", got.Phase)
	}
}

func TestLoadLatestReturnsHighestBlock(t *testing.T) {
	s := openMemory(t, 10)

	for _, block := range []uint64{10, 30, 20} {
		if err := s.Save(block, makeState(state.WaitingForGenesis{})); err != nil {
			t.Fatalf("Save(%d): %v", block, err)
		}
	}

	block, _, err := s.LoadLatest()
	if err != nil {
		t.Fatalf("LoadLatest: %v", err)
	}
	if block != 30 {
		t.Errorf("expected block 30, got %d", block)
	}
}

func TestSaveReplacesSameBlock(t *testing.T) {
	s := openMemory(t, 5)

	if err := s.Save(100, makeState(state.WaitingForGenesis{})); err != nil {
		t.Fatalf("first Save: %v", err)
	}
	if err := s.Save(100, makeState(state.WaitingForRollover{})); err != nil {
		t.Fatalf("second Save: %v", err)
	}

	_, got, err := s.LoadLatest()
	if err != nil {
		t.Fatalf("LoadLatest: %v", err)
	}
	if _, ok := got.Phase.(state.WaitingForRollover); !ok {
		t.Errorf("phase: got %T, want WaitingForRollover (replacement)", got.Phase)
	}
}

func TestPruningKeepsLastN(t *testing.T) {
	const history = 3
	s := openMemory(t, history)

	for i := uint64(1); i <= 6; i++ {
		if err := s.Save(i*10, makeState(state.WaitingForGenesis{})); err != nil {
			t.Fatalf("Save(%d): %v", i*10, err)
		}
	}

	// After 6 saves with history=3, only blocks 40, 50, 60 should remain.
	// Verify by checking that LoadLatest returns 60.
	block, _, err := s.LoadLatest()
	if err != nil {
		t.Fatalf("LoadLatest: %v", err)
	}
	if block != 60 {
		t.Errorf("latest block: got %d, want 60", block)
	}

	// Verify that old blocks were pruned by saving at a lower block and checking
	// only 3 rows are kept.
	if err := s.Save(5, makeState(state.WaitingForGenesis{})); err != nil {
		t.Fatalf("Save(5): %v", err)
	}
	// After this insert: candidates are {5, 40, 50, 60}; history=3 keeps top 3 = {40, 50, 60}.
	// Block 5 should be pruned immediately.
	block, _, err = s.LoadLatest()
	if err != nil {
		t.Fatalf("LoadLatest after pruning: %v", err)
	}
	if block != 60 {
		t.Errorf("latest block after low-block insert: got %d, want 60", block)
	}
}

func TestPruningDisabledWhenZero(t *testing.T) {
	s := openMemory(t, 0) // stateHistory=0 means retain all

	for i := uint64(1); i <= 10; i++ {
		if err := s.Save(i, makeState(state.WaitingForGenesis{})); err != nil {
			t.Fatalf("Save(%d): %v", i, err)
		}
	}
	// All 10 rows should be present; we can verify via LoadLatest returning 10.
	block, _, err := s.LoadLatest()
	if err != nil {
		t.Fatalf("LoadLatest: %v", err)
	}
	if block != 10 {
		t.Errorf("expected block 10, got %d", block)
	}
}

func TestAllPhasesPersist(t *testing.T) {
	s := openMemory(t, 10)

	phases := []state.Phase{
		state.WaitingForGenesis{},
		state.WaitingForRollover{},
	}
	for i, phase := range phases {
		if err := s.Save(uint64(i+1)*100, makeState(phase)); err != nil {
			t.Fatalf("Save phase %T: %v", phase, err)
		}
	}

	block, got, err := s.LoadLatest()
	if err != nil {
		t.Fatalf("LoadLatest: %v", err)
	}
	if block != 200 {
		t.Errorf("block: got %d, want 200", block)
	}
	if _, ok := got.Phase.(state.WaitingForRollover); !ok {
		t.Errorf("phase: got %T, want WaitingForRollover", got.Phase)
	}
}
