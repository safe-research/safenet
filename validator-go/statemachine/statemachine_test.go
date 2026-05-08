package statemachine_test

import (
	"testing"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/crypto"
	"github.com/safe-research/safenet/validator-go/action"
	"github.com/safe-research/safenet/validator-go/contracts/coordinator"
	"github.com/safe-research/safenet/validator-go/participants"
	"github.com/safe-research/safenet/validator-go/state"
	"github.com/safe-research/safenet/validator-go/statemachine"
)

func generateAddress(t *testing.T) common.Address {
	t.Helper()
	key, err := crypto.GenerateKey()
	if err != nil {
		t.Fatalf("generate key: %v", err)
	}
	return crypto.PubkeyToAddress(key.PublicKey)
}

// buildConfig creates a ConsensusConfig with n randomly-generated participant
// addresses. OwnAddress is always addrs[0].
func buildConfig(t *testing.T, n int) (state.ConsensusConfig, []common.Address) {
	t.Helper()
	addrs := make([]common.Address, n)
	for i := range addrs {
		addrs[i] = generateAddress(t)
	}
	return state.ConsensusConfig{OwnAddress: addrs[0], Participants: addrs}, addrs
}

// keyGenEvent returns a KeyGen event that matches cfg's genesis group ID.
func keyGenEvent(cfg state.ConsensusConfig) coordinator.FROSTCoordinatorKeyGen {
	count := uint16(len(cfg.Participants))
	threshold := count/2 + 1
	return coordinator.FROSTCoordinatorKeyGen{
		Gid:          participants.CalcGenesisGroupID(cfg.Participants, cfg.GenesisSalt),
		Participants: participants.CalcParticipantsRoot(cfg.Participants),
		Count:        count,
		Threshold:    threshold,
	}
}

// --- HandleKeyGen ---

func TestHandleKeyGenIgnoresWrongPhase(t *testing.T) {
	cfg, _ := buildConfig(t, 2)
	st := state.State{Config: cfg, Phase: state.WaitingForRollover{}}
	got, acts, err := statemachine.HandleKeyGen(st, keyGenEvent(cfg), nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(acts) != 0 {
		t.Errorf("expected no actions, got %d", len(acts))
	}
	if _, ok := got.Phase.(state.WaitingForRollover); !ok {
		t.Errorf("expected WaitingForRollover, got %T", got.Phase)
	}
}

func TestHandleKeyGenIgnoresNonMatchingGID(t *testing.T) {
	cfg, _ := buildConfig(t, 2)
	st := state.State{Config: cfg, Phase: state.WaitingForGenesis{}}
	ev := keyGenEvent(cfg)
	ev.Gid = [32]byte{} // zero GID will not match
	got, acts, err := statemachine.HandleKeyGen(st, ev, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(acts) != 0 {
		t.Errorf("expected no actions, got %d", len(acts))
	}
	if _, ok := got.Phase.(state.WaitingForGenesis); !ok {
		t.Errorf("expected WaitingForGenesis, got %T", got.Phase)
	}
}

func TestHandleKeyGenRejectsNonMember(t *testing.T) {
	cfg, _ := buildConfig(t, 2)
	cfg.OwnAddress = generateAddress(t) // not in participants
	st := state.State{Config: cfg, Phase: state.WaitingForGenesis{}}
	_, _, err := statemachine.HandleKeyGen(st, keyGenEvent(cfg), nil)
	if err == nil {
		t.Error("expected error for non-member own address, got nil")
	}
}

func TestHandleKeyGenTransitions(t *testing.T) {
	cfg, _ := buildConfig(t, 3)
	st := state.State{Config: cfg, Phase: state.WaitingForGenesis{}}
	got, acts, err := statemachine.HandleKeyGen(st, keyGenEvent(cfg), nil)
	if err != nil {
		t.Fatalf("HandleKeyGen: %v", err)
	}
	if len(acts) != 1 {
		t.Fatalf("expected 1 action, got %d", len(acts))
	}
	if _, ok := acts[0].(action.KeyGenAndCommit); !ok {
		t.Errorf("expected KeyGenAndCommit action, got %T", acts[0])
	}
	if _, ok := got.Phase.(state.CollectingCommitments); !ok {
		t.Errorf("expected CollectingCommitments, got %T", got.Phase)
	}
}

// --- HandleKeyGenCommitted ---

func TestHandleKeyGenCommittedIgnoresWrongPhase(t *testing.T) {
	cfg, _ := buildConfig(t, 2)
	st := state.State{Config: cfg, Phase: state.WaitingForGenesis{}}
	ev := coordinator.FROSTCoordinatorKeyGenCommitted{Committed: true, Participant: cfg.Participants[0]}
	got, acts, err := statemachine.HandleKeyGenCommitted(st, ev)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(acts) != 0 {
		t.Errorf("expected no actions, got %d", len(acts))
	}
	if _, ok := got.Phase.(state.WaitingForGenesis); !ok {
		t.Errorf("expected WaitingForGenesis, got %T", got.Phase)
	}
}

func TestHandleKeyGenCommittedSkipsNotCommitted(t *testing.T) {
	cfg, _ := buildConfig(t, 2)
	// CollectingCommitments with an empty commitments map; fields are unused
	// because the handler returns before accessing them when Committed is false.
	st := state.State{
		Config: cfg,
		Phase:  state.CollectingCommitments{},
	}
	ev := coordinator.FROSTCoordinatorKeyGenCommitted{Committed: false, Participant: cfg.Participants[0]}
	got, acts, err := statemachine.HandleKeyGenCommitted(st, ev)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(acts) != 0 {
		t.Errorf("expected no actions, got %d", len(acts))
	}
	phase, ok := got.Phase.(state.CollectingCommitments)
	if !ok {
		t.Fatalf("expected CollectingCommitments, got %T", got.Phase)
	}
	if len(phase.Commitments) != 0 {
		t.Errorf("expected 0 commitments, got %d", len(phase.Commitments))
	}
}

// --- HandleKeyGenSecretShared ---

func TestHandleKeyGenSecretSharedIgnoresWrongPhase(t *testing.T) {
	cfg, _ := buildConfig(t, 2)
	st := state.State{Config: cfg, Phase: state.WaitingForGenesis{}}
	ev := coordinator.FROSTCoordinatorKeyGenSecretShared{Shared: true, Participant: cfg.Participants[0]}
	got, acts, err := statemachine.HandleKeyGenSecretShared(st, ev)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(acts) != 0 {
		t.Errorf("expected no actions, got %d", len(acts))
	}
	if _, ok := got.Phase.(state.WaitingForGenesis); !ok {
		t.Errorf("expected WaitingForGenesis, got %T", got.Phase)
	}
}

func TestHandleKeyGenSecretSharedSkipsNotShared(t *testing.T) {
	cfg, _ := buildConfig(t, 2)
	// CollectingShares with nil fields; handler returns before accessing them
	// when Shared is false.
	st := state.State{
		Config: cfg,
		Phase:  state.CollectingShares{},
	}
	ev := coordinator.FROSTCoordinatorKeyGenSecretShared{Shared: false, Participant: cfg.Participants[0]}
	got, acts, err := statemachine.HandleKeyGenSecretShared(st, ev)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(acts) != 0 {
		t.Errorf("expected no actions, got %d", len(acts))
	}
	phase, ok := got.Phase.(state.CollectingShares)
	if !ok {
		t.Fatalf("expected CollectingShares, got %T", got.Phase)
	}
	if len(phase.Shares) != 0 {
		t.Errorf("expected 0 shares, got %d", len(phase.Shares))
	}
}

// --- Full genesis ceremony ---

// TestFullGenesisCeremony drives three participants through the complete happy
// path and asserts that every participant ends in GenesisComplete.
func TestFullGenesisCeremony(t *testing.T) {
	const n = 3

	addrs := make([]common.Address, n)
	for i := range addrs {
		addrs[i] = generateAddress(t)
	}

	gid := participants.CalcGenesisGroupID(addrs, [32]byte{})
	ev0 := coordinator.FROSTCoordinatorKeyGen{
		Gid:          gid,
		Participants: participants.CalcParticipantsRoot(addrs),
		Count:        uint16(n),
		Threshold:    uint16(n)/2 + 1,
	}

	// Phase 0: each participant processes the KeyGen event and runs DKG round 1.
	states := make([]state.State, n)
	commitments := make([]coordinator.FROSTCoordinatorKeyGenCommitment, n)
	for i := range states {
		states[i] = state.State{
			Config: state.ConsensusConfig{OwnAddress: addrs[i], Participants: addrs},
			Phase:  state.WaitingForGenesis{},
		}
		var acts []action.Action
		var err error
		states[i], acts, err = statemachine.HandleKeyGen(states[i], ev0, nil)
		if err != nil {
			t.Fatalf("participant %d HandleKeyGen: %v", i, err)
		}
		if len(acts) != 1 {
			t.Fatalf("participant %d: expected 1 action, got %d", i, len(acts))
		}
		commitments[i] = acts[0].(action.KeyGenAndCommit).Commitment
	}

	// Phase 1: each participant receives all KeyGenCommitted events and runs DKG round 2.
	secretShares := make([]coordinator.FROSTCoordinatorKeyGenSecretShare, n)
	for i := range states {
		for j, addr := range addrs {
			ev := coordinator.FROSTCoordinatorKeyGenCommitted{
				Gid:        gid,
				Participant: addr,
				Commitment:  commitments[j],
				Committed:   true,
			}
			var acts []action.Action
			var err error
			states[i], acts, err = statemachine.HandleKeyGenCommitted(states[i], ev)
			if err != nil {
				t.Fatalf("participant %d, committed from %d: %v", i, j, err)
			}
			if len(acts) > 0 {
				// Transition fired; capture this participant's round-2 output.
				secretShares[i] = acts[0].(action.KeyGenSecretShare).Share
			}
		}
		if _, ok := states[i].Phase.(state.CollectingShares); !ok {
			t.Fatalf("participant %d: expected CollectingShares after all commits, got %T", i, states[i].Phase)
		}
	}

	// Phase 2: each participant receives all KeyGenSecretShared events and runs DKG round 3.
	for i := range states {
		for j, addr := range addrs {
			ev := coordinator.FROSTCoordinatorKeyGenSecretShared{
				Gid:        gid,
				Participant: addr,
				Share:      secretShares[j],
				Shared:     true,
			}
			var acts []action.Action
			var err error
			states[i], acts, err = statemachine.HandleKeyGenSecretShared(states[i], ev)
			if err != nil {
				t.Fatalf("participant %d, shared from %d: %v", i, j, err)
			}
			if len(acts) > 0 {
				if _, ok := acts[0].(action.KeyGenConfirm); !ok {
					t.Errorf("participant %d: expected KeyGenConfirm action, got %T", i, acts[0])
				}
			}
		}
		if _, ok := states[i].Phase.(state.GenesisComplete); !ok {
			t.Errorf("participant %d: expected GenesisComplete, got %T", i, states[i].Phase)
		}
	}
}

// TestHandleKeyGenCommittedAccumulatesBeforeTransition verifies that receiving
// fewer than n commitments keeps the state in CollectingCommitments without
// emitting any action.
func TestHandleKeyGenCommittedAccumulatesBeforeTransition(t *testing.T) {
	cfg, addrs := buildConfig(t, 3)
	st := state.State{Config: cfg, Phase: state.WaitingForGenesis{}}

	// Transition to CollectingCommitments via a KeyGen event.
	var err error
	st, _, err = statemachine.HandleKeyGen(st, keyGenEvent(cfg), nil)
	if err != nil {
		t.Fatalf("HandleKeyGen: %v", err)
	}

	// Add one commitment (only 1 of 3): should stay in CollectingCommitments.
	ev := coordinator.FROSTCoordinatorKeyGenCommitted{
		Gid:        participants.CalcGenesisGroupID(addrs, [32]byte{}),
		Participant: addrs[1],
		Committed:   true,
	}
	got, acts, err := statemachine.HandleKeyGenCommitted(st, ev)
	if err != nil {
		t.Fatalf("HandleKeyGenCommitted: %v", err)
	}
	if len(acts) != 0 {
		t.Errorf("expected no actions after partial commit, got %d", len(acts))
	}
	phase, ok := got.Phase.(state.CollectingCommitments)
	if !ok {
		t.Fatalf("expected CollectingCommitments, got %T", got.Phase)
	}
	if len(phase.Commitments) != 1 {
		t.Errorf("expected 1 commitment, got %d", len(phase.Commitments))
	}
}

// TestHandleKeyGenSecretSharedAccumulatesBeforeTransition verifies that
// receiving fewer than n shares keeps the state in CollectingShares without
// emitting any action.
func TestHandleKeyGenSecretSharedAccumulatesBeforeTransition(t *testing.T) {
	_, addrs := buildConfig(t, 3)

	// Run phase 0 and phase 1 for participant 0 to reach CollectingShares.
	gid := participants.CalcGenesisGroupID(addrs, [32]byte{})
	ev0 := coordinator.FROSTCoordinatorKeyGen{
		Gid:          gid,
		Participants: participants.CalcParticipantsRoot(addrs),
		Count:        3,
		Threshold:    2,
	}

	// Each participant runs round 1.
	states := make([]state.State, 3)
	commitments := make([]coordinator.FROSTCoordinatorKeyGenCommitment, 3)
	for i := range states {
		states[i] = state.State{
			Config: state.ConsensusConfig{OwnAddress: addrs[i], Participants: addrs},
			Phase:  state.WaitingForGenesis{},
		}
		var acts []action.Action
		var err error
		states[i], acts, err = statemachine.HandleKeyGen(states[i], ev0, nil)
		if err != nil {
			t.Fatalf("participant %d HandleKeyGen: %v", i, err)
		}
		commitments[i] = acts[0].(action.KeyGenAndCommit).Commitment
	}

	// Participant 0 processes all commitments → reaches CollectingShares.
	st := states[0]
	for j, addr := range addrs {
		ev := coordinator.FROSTCoordinatorKeyGenCommitted{
			Gid:        gid,
			Participant: addr,
			Commitment:  commitments[j],
			Committed:   true,
		}
		var err error
		st, _, err = statemachine.HandleKeyGenCommitted(st, ev)
		if err != nil {
			t.Fatalf("committed from %d: %v", j, err)
		}
	}
	if _, ok := st.Phase.(state.CollectingShares); !ok {
		t.Fatalf("expected CollectingShares, got %T", st.Phase)
	}

	// Add one share (only 1 of 3): should stay in CollectingShares.
	ev := coordinator.FROSTCoordinatorKeyGenSecretShared{
		Gid:        gid,
		Participant: addrs[1],
		Shared:     true,
	}
	got, acts, err := statemachine.HandleKeyGenSecretShared(st, ev)
	if err != nil {
		t.Fatalf("HandleKeyGenSecretShared: %v", err)
	}
	if len(acts) != 0 {
		t.Errorf("expected no actions after partial share, got %d", len(acts))
	}
	phase, ok := got.Phase.(state.CollectingShares)
	if !ok {
		t.Fatalf("expected CollectingShares, got %T", got.Phase)
	}
	if len(phase.Shares) != 1 {
		t.Errorf("expected 1 share, got %d", len(phase.Shares))
	}
}
