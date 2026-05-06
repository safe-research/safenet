package config

import (
	"encoding/hex"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/BurntSushi/toml"
	"github.com/ethereum/go-ethereum/common"
)

// Participant is a network participant identified by their Ethereum address.
type Participant struct {
	Address common.Address
}

// Config holds the validated validator configuration.
type Config struct {
	// Required fields.
	RPCURL           string
	PrivateKey       string
	ConsensusAddress common.Address
	Participants     []Participant

	// Optional fields. Nil means "use default".
	StorageFile       string          // empty = in-memory SQLite
	StakerAddress     *common.Address
	GenesisSalt       *[32]byte
	BlocksPerEpoch    *uint64
	BlockTimeOverride *uint64
	StateHistory      *uint
}

// Load reads and validates a TOML config from the given file path.
func Load(path string) (*Config, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("open: %w", err)
	}
	defer f.Close()
	return Parse(f)
}

// Parse reads and validates a TOML config from r.
func Parse(r io.Reader) (*Config, error) {
	var raw rawConfig
	if _, err := toml.NewDecoder(r).Decode(&raw); err != nil {
		return nil, fmt.Errorf("decode: %w", err)
	}
	return validate(&raw)
}

type rawParticipant struct {
	Address string `toml:"address"`
}

type rawConfig struct {
	RPCURL            string           `toml:"rpc_url"`
	PrivateKey        string           `toml:"private_key"`
	ConsensusAddress  string           `toml:"consensus_address"`
	Participants      []rawParticipant `toml:"participants"`
	StorageFile       string           `toml:"storage_file"`
	StakerAddress     string           `toml:"staker_address"`
	GenesisSalt       string           `toml:"genesis_salt"`
	BlocksPerEpoch    uint64           `toml:"blocks_per_epoch"`
	BlockTimeOverride uint64           `toml:"block_time_override"`
	StateHistory      uint             `toml:"state_history"`
}

func validate(raw *rawConfig) (*Config, error) {
	if raw.RPCURL == "" {
		return nil, fmt.Errorf("rpc_url is required")
	}
	if raw.PrivateKey == "" {
		return nil, fmt.Errorf("private_key is required")
	}
	if err := validatePrivateKey(raw.PrivateKey); err != nil {
		return nil, fmt.Errorf("private_key: %w", err)
	}
	if raw.ConsensusAddress == "" {
		return nil, fmt.Errorf("consensus_address is required")
	}
	if !common.IsHexAddress(raw.ConsensusAddress) {
		return nil, fmt.Errorf("consensus_address: invalid Ethereum address %q", raw.ConsensusAddress)
	}
	if len(raw.Participants) == 0 {
		return nil, fmt.Errorf("at least one [[participants]] entry is required")
	}

	cfg := &Config{
		RPCURL:           raw.RPCURL,
		PrivateKey:       raw.PrivateKey,
		ConsensusAddress: common.HexToAddress(raw.ConsensusAddress),
		StorageFile:      raw.StorageFile,
	}

	for i, p := range raw.Participants {
		if !common.IsHexAddress(p.Address) {
			return nil, fmt.Errorf("participants[%d].address: invalid Ethereum address %q", i, p.Address)
		}
		cfg.Participants = append(cfg.Participants, Participant{
			Address: common.HexToAddress(p.Address),
		})
	}

	if raw.StakerAddress != "" {
		if !common.IsHexAddress(raw.StakerAddress) {
			return nil, fmt.Errorf("staker_address: invalid Ethereum address %q", raw.StakerAddress)
		}
		addr := common.HexToAddress(raw.StakerAddress)
		cfg.StakerAddress = &addr
	}
	if raw.GenesisSalt != "" {
		salt, err := parseBytes32(raw.GenesisSalt)
		if err != nil {
			return nil, fmt.Errorf("genesis_salt: %w", err)
		}
		cfg.GenesisSalt = &salt
	}
	if raw.BlocksPerEpoch != 0 {
		v := raw.BlocksPerEpoch
		cfg.BlocksPerEpoch = &v
	}
	if raw.BlockTimeOverride != 0 {
		v := raw.BlockTimeOverride
		cfg.BlockTimeOverride = &v
	}
	if raw.StateHistory != 0 {
		v := raw.StateHistory
		cfg.StateHistory = &v
	}

	return cfg, nil
}

func validatePrivateKey(key string) error {
	b, err := decodeHex(key)
	if err != nil {
		return fmt.Errorf("invalid hex: %w", err)
	}
	if len(b) != 32 {
		return fmt.Errorf("must be 32 bytes, got %d", len(b))
	}
	return nil
}

func parseBytes32(s string) ([32]byte, error) {
	var out [32]byte
	b, err := decodeHex(s)
	if err != nil {
		return out, fmt.Errorf("invalid hex: %w", err)
	}
	if len(b) != 32 {
		return out, fmt.Errorf("must be 32 bytes, got %d", len(b))
	}
	copy(out[:], b)
	return out, nil
}

func decodeHex(s string) ([]byte, error) {
	return hex.DecodeString(strings.TrimPrefix(s, "0x"))
}
