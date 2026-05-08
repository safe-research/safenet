package statemachine

import (
	"fmt"
	"io"

	"github.com/ethereum/go-ethereum/common"
	"github.com/safe-research/safenet/validator-go/action"
	"github.com/safe-research/safenet/validator-go/contracts/coordinator"
	"github.com/safe-research/safenet/validator-go/frost"
	"github.com/safe-research/safenet/validator-go/participants"
	"github.com/safe-research/safenet/validator-go/state"
)

// HandleKeyGen processes a KeyGen coordinator event.
// Transitions WaitingForGenesis → CollectingCommitments when the event group ID
// matches CalcGenesisGroupID for this validator's participant set. Emits a
// KeyGenAndCommit action containing the round-1 DKG payload.
// Returns the state unchanged for any other phase or non-matching group ID.
func HandleKeyGen(st state.State, ev coordinator.FROSTCoordinatorKeyGen, r io.Reader) (state.State, []action.Action, error) {
	if _, ok := st.Phase.(state.WaitingForGenesis); !ok {
		return st, nil, nil
	}
	cfg := st.Config
	if ev.Gid != participants.CalcGenesisGroupID(cfg.Participants, cfg.GenesisSalt) {
		return st, nil, nil
	}
	proof, ok := participants.GenerateParticipantProof(cfg.Participants, cfg.OwnAddress)
	if !ok {
		return st, nil, fmt.Errorf("own address %s is not a participant", cfg.OwnAddress)
	}
	round1, err := frost.GenerateRound1(cfg.OwnAddress, ev.Count, ev.Threshold, r)
	if err != nil {
		return st, nil, fmt.Errorf("DKG round 1: %w", err)
	}
	newSt := state.State{
		Config: cfg,
		Phase: state.CollectingCommitments{
			Round1Secret:  round1.SecretPackage,
			EncryptionKey: round1.EncryptionKey,
			Commitments:   make(map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment),
		},
	}
	acts := []action.Action{action.KeyGenAndCommit{
		Participants: ev.Participants,
		Count:        ev.Count,
		Threshold:    ev.Threshold,
		Context:      ev.Context,
		Proof:        proof,
		Commitment:   round1.Commitment,
	}}
	return newSt, acts, nil
}

// HandleKeyGenCommitted processes a KeyGenCommitted coordinator event.
// Accumulates participant commitments in the CollectingCommitments phase. When
// all participants have committed, runs DKG round 2, transitions to
// CollectingShares, and emits a KeyGenSecretShare action.
// Returns the state unchanged when not in CollectingCommitments or ev.Committed is false.
func HandleKeyGenCommitted(st state.State, ev coordinator.FROSTCoordinatorKeyGenCommitted) (state.State, []action.Action, error) {
	phase, ok := st.Phase.(state.CollectingCommitments)
	if !ok {
		return st, nil, nil
	}
	if !ev.Committed {
		return st, nil, nil
	}

	commitments := cloneCommitmentsMap(phase.Commitments)
	commitments[ev.Participant] = ev.Commitment

	if len(commitments) < len(st.Config.Participants) {
		return state.State{
			Config: st.Config,
			Phase: state.CollectingCommitments{
				Round1Secret:  phase.Round1Secret,
				EncryptionKey: phase.EncryptionKey,
				Commitments:   commitments,
			},
		}, nil, nil
	}

	round2, err := frost.GenerateRound2(phase.EncryptionKey, phase.Round1Secret, commitments)
	if err != nil {
		return st, nil, fmt.Errorf("DKG round 2: %w", err)
	}
	newSt := state.State{
		Config: st.Config,
		Phase: state.CollectingShares{
			Round2Secret:  round2.SecretPackage,
			EncryptionKey: phase.EncryptionKey,
			Commitments:   commitments,
			Shares:        make(map[common.Address]coordinator.FROSTCoordinatorKeyGenSecretShare),
		},
	}
	acts := []action.Action{action.KeyGenSecretShare{
		GroupID: ev.Gid,
		Share:   round2.Share,
	}}
	return newSt, acts, nil
}

// HandleKeyGenSecretShared processes a KeyGenSecretShared coordinator event.
// Accumulates participant secret shares in the CollectingShares phase. When all
// participants have shared, runs DKG round 3, transitions to GenesisComplete,
// and emits a KeyGenConfirm action.
// Returns the state unchanged when not in CollectingShares or ev.Shared is false.
func HandleKeyGenSecretShared(st state.State, ev coordinator.FROSTCoordinatorKeyGenSecretShared) (state.State, []action.Action, error) {
	phase, ok := st.Phase.(state.CollectingShares)
	if !ok {
		return st, nil, nil
	}
	if !ev.Shared {
		return st, nil, nil
	}

	shares := cloneSharesMap(phase.Shares)
	shares[ev.Participant] = ev.Share

	if len(shares) < len(st.Config.Participants) {
		return state.State{
			Config: st.Config,
			Phase: state.CollectingShares{
				Round2Secret:  phase.Round2Secret,
				EncryptionKey: phase.EncryptionKey,
				Commitments:   phase.Commitments,
				Shares:        shares,
			},
		}, nil, nil
	}

	round3, err := frost.GenerateRound3(phase.EncryptionKey, phase.Round2Secret, phase.Commitments, shares)
	if err != nil {
		return st, nil, fmt.Errorf("DKG round 3: %w", err)
	}
	newSt := state.State{
		Config: st.Config,
		Phase:  state.GenesisComplete{KeyPackage: round3.KeyPackage},
	}
	acts := []action.Action{action.KeyGenConfirm{GroupID: ev.Gid}}
	return newSt, acts, nil
}

func cloneCommitmentsMap(m map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment) map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment {
	out := make(map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment, len(m)+1)
	for k, v := range m {
		out[k] = v
	}
	return out
}

func cloneSharesMap(m map[common.Address]coordinator.FROSTCoordinatorKeyGenSecretShare) map[common.Address]coordinator.FROSTCoordinatorKeyGenSecretShare {
	out := make(map[common.Address]coordinator.FROSTCoordinatorKeyGenSecretShare, len(m)+1)
	for k, v := range m {
		out[k] = v
	}
	return out
}
