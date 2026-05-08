package state

import (
	"encoding/json"
	"fmt"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/common/hexutil"
	"github.com/safe-research/safenet/validator-go/contracts/coordinator"
	"github.com/safe-research/safenet/validator-go/frost"
	"github.com/safe-research/safenet/validator-go/secret"
)

// ConsensusConfig holds the parameters used by the genesis state machine.
type ConsensusConfig struct {
	OwnAddress         common.Address
	CoordinatorAddress common.Address
	Participants       []common.Address
	GenesisSalt        [32]byte
	BlocksPerEpoch     uint64
}

type consensusConfigJSON struct {
	OwnAddress         common.Address   `json:"own_address"`
	CoordinatorAddress common.Address   `json:"coordinator_address"`
	Participants       []common.Address `json:"participants"`
	GenesisSalt        hexutil.Bytes    `json:"genesis_salt"`
	BlocksPerEpoch     uint64           `json:"blocks_per_epoch"`
}

func (c ConsensusConfig) MarshalJSON() ([]byte, error) {
	return json.Marshal(consensusConfigJSON{
		OwnAddress:         c.OwnAddress,
		CoordinatorAddress: c.CoordinatorAddress,
		Participants:       c.Participants,
		GenesisSalt:        c.GenesisSalt[:],
		BlocksPerEpoch:     c.BlocksPerEpoch,
	})
}

func (c *ConsensusConfig) UnmarshalJSON(data []byte) error {
	var j consensusConfigJSON
	if err := json.Unmarshal(data, &j); err != nil {
		return err
	}
	if len(j.GenesisSalt) != 32 {
		return fmt.Errorf("genesis_salt: expected 32 bytes, got %d", len(j.GenesisSalt))
	}
	c.OwnAddress = j.OwnAddress
	c.CoordinatorAddress = j.CoordinatorAddress
	c.Participants = j.Participants
	copy(c.GenesisSalt[:], j.GenesisSalt)
	c.BlocksPerEpoch = j.BlocksPerEpoch
	return nil
}

// Phase is the current state machine phase (sum type).
type Phase interface {
	phaseName() string
}

// WaitingForGenesis is the initial phase: idle until a matching KeyGen event.
type WaitingForGenesis struct{}

func (WaitingForGenesis) phaseName() string { return "WaitingForGenesis" }

// CollectingCommitments accumulates KeyGenCommitted events from all participants.
type CollectingCommitments struct {
	Round1Secret  *frost.Round1SecretPackage
	EncryptionKey *secret.EncryptionKey
	Commitments   map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment
}

func (CollectingCommitments) phaseName() string { return "CollectingCommitments" }

// CollectingShares accumulates KeyGenSecretShared events from all participants.
type CollectingShares struct {
	Round2Secret  *frost.Round2SecretPackage
	EncryptionKey *secret.EncryptionKey
	Commitments   map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment
	Shares        map[common.Address]coordinator.FROSTCoordinatorKeyGenSecretShare
}

func (CollectingShares) phaseName() string { return "CollectingShares" }

// GenesisComplete holds the validator's FROST signing share after DKG.
type GenesisComplete struct {
	KeyPackage *frost.KeyPackage
}

func (GenesisComplete) phaseName() string { return "GenesisComplete" }

// WaitingForRollover is the post-genesis idle phase (not yet implemented).
type WaitingForRollover struct{}

func (WaitingForRollover) phaseName() string { return "WaitingForRollover" }

// State is the full persisted validator state: consensus config plus current phase.
type State struct {
	Config ConsensusConfig
	Phase  Phase
}

type stateJSON struct {
	Config ConsensusConfig `json:"config"`
	Phase  string          `json:"phase"`
	Data   json.RawMessage `json:"data,omitempty"`
}

func (s State) MarshalJSON() ([]byte, error) {
	data, err := marshalPhaseData(s.Phase)
	if err != nil {
		return nil, fmt.Errorf("marshal phase: %w", err)
	}
	return json.Marshal(stateJSON{
		Config: s.Config,
		Phase:  s.Phase.phaseName(),
		Data:   data,
	})
}

func (s *State) UnmarshalJSON(raw []byte) error {
	var j stateJSON
	if err := json.Unmarshal(raw, &j); err != nil {
		return err
	}
	s.Config = j.Config
	phase, err := unmarshalPhase(j.Phase, j.Data)
	if err != nil {
		return fmt.Errorf("unmarshal phase: %w", err)
	}
	s.Phase = phase
	return nil
}

// marshalPhaseData returns the JSON encoding of the phase-specific data fields.
// Returns nil for phases with no data fields.
func marshalPhaseData(p Phase) (json.RawMessage, error) {
	switch p := p.(type) {
	case WaitingForGenesis, WaitingForRollover:
		return nil, nil
	case CollectingCommitments:
		return marshalCollectingCommitments(p)
	case CollectingShares:
		return marshalCollectingShares(p)
	case GenesisComplete:
		return marshalGenesisComplete(p)
	default:
		return nil, fmt.Errorf("unknown phase type %T", p)
	}
}

// unmarshalPhase reconstructs a Phase from its name and raw data.
func unmarshalPhase(name string, data json.RawMessage) (Phase, error) {
	switch name {
	case "WaitingForGenesis":
		return WaitingForGenesis{}, nil
	case "WaitingForRollover":
		return WaitingForRollover{}, nil
	case "CollectingCommitments":
		return unmarshalCollectingCommitments(data)
	case "CollectingShares":
		return unmarshalCollectingShares(data)
	case "GenesisComplete":
		return unmarshalGenesisComplete(data)
	default:
		return nil, fmt.Errorf("unknown phase %q", name)
	}
}

// JSON helpers for CollectingCommitments.

type collectingCommitmentsJSON struct {
	Round1Secret  *frost.Round1SecretPackage                                      `json:"round1_secret"`
	EncryptionKey hexutil.Bytes                                                   `json:"encryption_key"`
	Commitments   map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment `json:"commitments"`
}

func marshalCollectingCommitments(p CollectingCommitments) (json.RawMessage, error) {
	keyBytes := p.EncryptionKey.Bytes()
	return json.Marshal(collectingCommitmentsJSON{
		Round1Secret:  p.Round1Secret,
		EncryptionKey: keyBytes[:],
		Commitments:   p.Commitments,
	})
}

func unmarshalCollectingCommitments(data json.RawMessage) (CollectingCommitments, error) {
	var j collectingCommitmentsJSON
	if err := json.Unmarshal(data, &j); err != nil {
		return CollectingCommitments{}, err
	}
	if len(j.EncryptionKey) != 32 {
		return CollectingCommitments{}, fmt.Errorf("encryption_key: expected 32 bytes, got %d", len(j.EncryptionKey))
	}
	var keyBytes [32]byte
	copy(keyBytes[:], j.EncryptionKey)
	encKey, err := secret.EncryptionKeyFromBytes(keyBytes)
	if err != nil {
		return CollectingCommitments{}, fmt.Errorf("encryption_key: %w", err)
	}
	return CollectingCommitments{
		Round1Secret:  j.Round1Secret,
		EncryptionKey: encKey,
		Commitments:   j.Commitments,
	}, nil
}

// JSON helpers for CollectingShares.

type collectingSharesJSON struct {
	Round2Secret  *frost.Round2SecretPackage                                      `json:"round2_secret"`
	EncryptionKey hexutil.Bytes                                                   `json:"encryption_key"`
	Commitments   map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment `json:"commitments"`
	Shares        map[common.Address]coordinator.FROSTCoordinatorKeyGenSecretShare `json:"shares"`
}

func marshalCollectingShares(p CollectingShares) (json.RawMessage, error) {
	keyBytes := p.EncryptionKey.Bytes()
	return json.Marshal(collectingSharesJSON{
		Round2Secret:  p.Round2Secret,
		EncryptionKey: keyBytes[:],
		Commitments:   p.Commitments,
		Shares:        p.Shares,
	})
}

func unmarshalCollectingShares(data json.RawMessage) (CollectingShares, error) {
	var j collectingSharesJSON
	if err := json.Unmarshal(data, &j); err != nil {
		return CollectingShares{}, err
	}
	if len(j.EncryptionKey) != 32 {
		return CollectingShares{}, fmt.Errorf("encryption_key: expected 32 bytes, got %d", len(j.EncryptionKey))
	}
	var keyBytes [32]byte
	copy(keyBytes[:], j.EncryptionKey)
	encKey, err := secret.EncryptionKeyFromBytes(keyBytes)
	if err != nil {
		return CollectingShares{}, fmt.Errorf("encryption_key: %w", err)
	}
	return CollectingShares{
		Round2Secret:  j.Round2Secret,
		EncryptionKey: encKey,
		Commitments:   j.Commitments,
		Shares:        j.Shares,
	}, nil
}

// JSON helpers for GenesisComplete.

type genesisCompleteJSON struct {
	KeyPackage *frost.KeyPackage `json:"key_package"`
}

func marshalGenesisComplete(p GenesisComplete) (json.RawMessage, error) {
	return json.Marshal(genesisCompleteJSON{KeyPackage: p.KeyPackage})
}

func unmarshalGenesisComplete(data json.RawMessage) (GenesisComplete, error) {
	var j genesisCompleteJSON
	if err := json.Unmarshal(data, &j); err != nil {
		return GenesisComplete{}, err
	}
	return GenesisComplete{KeyPackage: j.KeyPackage}, nil
}
