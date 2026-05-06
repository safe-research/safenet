package config

import (
	"strings"
	"testing"

	"github.com/ethereum/go-ethereum/common"
)

const (
	validRPCURL           = "http://127.0.0.1:8545"
	validPrivateKey       = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
	validConsensusAddress = "0x1234567890AbcdEF1234567890aBcdef12345678"
	validParticipant      = "0xAbcdEF1234567890abcdef1234567890AbCdEf12"
	validStakerAddress    = "0xDeaDbeefdEAdbeefdEadbEEFdeadbeEFdEaDbeeF"
	validGenesisSalt      = "0x0000000000000000000000000000000000000000000000000000000000000001"
)

// buildTOML produces a config string with optional top-level fields inserted
// before the [[participants]] section (required by TOML table scoping rules).
func buildTOML(extras ...string) string {
	lines := []string{
		`rpc_url = "` + validRPCURL + `"`,
		`private_key = "` + validPrivateKey + `"`,
		`consensus_address = "` + validConsensusAddress + `"`,
	}
	lines = append(lines, extras...)
	lines = append(lines,
		`[[participants]]`,
		`address = "` + validParticipant + `"`,
	)
	return strings.Join(lines, "\n")
}

func parse(t *testing.T, toml string) (*Config, error) {
	t.Helper()
	return Parse(strings.NewReader(toml))
}

func mustParse(t *testing.T, toml string) *Config {
	t.Helper()
	cfg, err := parse(t, toml)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	return cfg
}

func TestRequiredFields(t *testing.T) {
	cfg := mustParse(t, buildTOML())

	if cfg.RPCURL != validRPCURL {
		t.Errorf("RPCURL = %q, want %q", cfg.RPCURL, validRPCURL)
	}
	if cfg.PrivateKey != validPrivateKey {
		t.Errorf("PrivateKey = %q, want %q", cfg.PrivateKey, validPrivateKey)
	}
	wantConsensus := common.HexToAddress(validConsensusAddress)
	if cfg.ConsensusAddress != wantConsensus {
		t.Errorf("ConsensusAddress = %s, want %s", cfg.ConsensusAddress.Hex(), wantConsensus.Hex())
	}
	if len(cfg.Participants) != 1 {
		t.Fatalf("len(Participants) = %d, want 1", len(cfg.Participants))
	}
	wantParticipant := common.HexToAddress(validParticipant)
	if cfg.Participants[0].Address != wantParticipant {
		t.Errorf("Participants[0].Address = %s, want %s", cfg.Participants[0].Address.Hex(), wantParticipant.Hex())
	}

	// Optional fields should be nil / zero.
	if cfg.StorageFile != "" {
		t.Errorf("StorageFile = %q, want empty", cfg.StorageFile)
	}
	if cfg.StakerAddress != nil {
		t.Errorf("StakerAddress should be nil")
	}
	if cfg.GenesisSalt != nil {
		t.Errorf("GenesisSalt should be nil")
	}
	if cfg.BlocksPerEpoch != nil {
		t.Errorf("BlocksPerEpoch should be nil")
	}
	if cfg.BlockTimeOverride != nil {
		t.Errorf("BlockTimeOverride should be nil")
	}
	if cfg.StateHistory != nil {
		t.Errorf("StateHistory should be nil")
	}
}

func TestAllOptionalFields(t *testing.T) {
	cfg := mustParse(t, buildTOML(
		`storage_file = "validator.sqlite"`,
		`staker_address = "`+validStakerAddress+`"`,
		`genesis_salt = "`+validGenesisSalt+`"`,
		`blocks_per_epoch = 1440`,
		`block_time_override = 5`,
		`state_history = 5`,
	))

	if cfg.StorageFile != "validator.sqlite" {
		t.Errorf("StorageFile = %q", cfg.StorageFile)
	}
	if cfg.StakerAddress == nil {
		t.Fatal("StakerAddress is nil")
	}
	wantStaker := common.HexToAddress(validStakerAddress)
	if *cfg.StakerAddress != wantStaker {
		t.Errorf("StakerAddress = %s, want %s", cfg.StakerAddress.Hex(), wantStaker.Hex())
	}
	if cfg.GenesisSalt == nil {
		t.Fatal("GenesisSalt is nil")
	}
	if cfg.BlocksPerEpoch == nil || *cfg.BlocksPerEpoch != 1440 {
		t.Errorf("BlocksPerEpoch = %v", cfg.BlocksPerEpoch)
	}
	if cfg.BlockTimeOverride == nil || *cfg.BlockTimeOverride != 5 {
		t.Errorf("BlockTimeOverride = %v", cfg.BlockTimeOverride)
	}
	if cfg.StateHistory == nil || *cfg.StateHistory != 5 {
		t.Errorf("StateHistory = %v", cfg.StateHistory)
	}
}

func TestMultipleParticipants(t *testing.T) {
	toml := buildTOML() + "\n[[participants]]\naddress = \"" + validStakerAddress + "\""
	cfg := mustParse(t, toml)
	if len(cfg.Participants) != 2 {
		t.Errorf("len(Participants) = %d, want 2", len(cfg.Participants))
	}
}

func TestMissingRequired(t *testing.T) {
	cases := []struct {
		name string
		toml string
		want string
	}{
		{
			name: "missing rpc_url",
			toml: strings.Join([]string{
				`private_key = "` + validPrivateKey + `"`,
				`consensus_address = "` + validConsensusAddress + `"`,
				`[[participants]]`,
				`address = "` + validParticipant + `"`,
			}, "\n"),
			want: "rpc_url is required",
		},
		{
			name: "missing private_key",
			toml: strings.Join([]string{
				`rpc_url = "` + validRPCURL + `"`,
				`consensus_address = "` + validConsensusAddress + `"`,
				`[[participants]]`,
				`address = "` + validParticipant + `"`,
			}, "\n"),
			want: "private_key is required",
		},
		{
			name: "missing consensus_address",
			toml: strings.Join([]string{
				`rpc_url = "` + validRPCURL + `"`,
				`private_key = "` + validPrivateKey + `"`,
				`[[participants]]`,
				`address = "` + validParticipant + `"`,
			}, "\n"),
			want: "consensus_address is required",
		},
		{
			name: "missing participants",
			toml: strings.Join([]string{
				`rpc_url = "` + validRPCURL + `"`,
				`private_key = "` + validPrivateKey + `"`,
				`consensus_address = "` + validConsensusAddress + `"`,
			}, "\n"),
			want: "at least one [[participants]] entry is required",
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			_, err := parse(t, tc.toml)
			if err == nil {
				t.Fatal("expected error, got nil")
			}
			if !strings.Contains(err.Error(), tc.want) {
				t.Errorf("error = %q, want to contain %q", err.Error(), tc.want)
			}
		})
	}
}

func TestInvalidAddresses(t *testing.T) {
	cases := []struct {
		name string
		toml string
		want string
	}{
		{
			name: "invalid consensus_address",
			toml: strings.Join([]string{
				`rpc_url = "` + validRPCURL + `"`,
				`private_key = "` + validPrivateKey + `"`,
				`consensus_address = "notanaddress"`,
				`[[participants]]`,
				`address = "` + validParticipant + `"`,
			}, "\n"),
			want: "consensus_address",
		},
		{
			name: "invalid participant address",
			toml: strings.Join([]string{
				`rpc_url = "` + validRPCURL + `"`,
				`private_key = "` + validPrivateKey + `"`,
				`consensus_address = "` + validConsensusAddress + `"`,
				`[[participants]]`,
				`address = "notanaddress"`,
			}, "\n"),
			want: "participants[0].address",
		},
		{
			name: "invalid staker_address",
			// staker_address must come before [[participants]] to be in the root table.
			toml: buildTOML(`staker_address = "notanaddress"`),
			want: "staker_address",
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			_, err := parse(t, tc.toml)
			if err == nil {
				t.Fatal("expected error, got nil")
			}
			if !strings.Contains(err.Error(), tc.want) {
				t.Errorf("error = %q, want to contain %q", err.Error(), tc.want)
			}
		})
	}
}

func TestInvalidPrivateKey(t *testing.T) {
	cases := []struct {
		name string
		key  string
	}{
		{"not hex", "0xZZZZ"},
		{"too short", "0xdeadbeef"},
		{"too long", "0x" + strings.Repeat("ab", 33)},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			toml := strings.Join([]string{
				`rpc_url = "` + validRPCURL + `"`,
				`private_key = "` + tc.key + `"`,
				`consensus_address = "` + validConsensusAddress + `"`,
				`[[participants]]`,
				`address = "` + validParticipant + `"`,
			}, "\n")
			_, err := parse(t, toml)
			if err == nil {
				t.Fatal("expected error, got nil")
			}
			if !strings.Contains(err.Error(), "private_key") {
				t.Errorf("error = %q, want to contain \"private_key\"", err.Error())
			}
		})
	}
}

func TestInvalidGenesisSalt(t *testing.T) {
	// genesis_salt must come before [[participants]] to be in the root table.
	toml := buildTOML(`genesis_salt = "0xdeadbeef"`)
	_, err := parse(t, toml)
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if !strings.Contains(err.Error(), "genesis_salt") {
		t.Errorf("error = %q, want to contain \"genesis_salt\"", err.Error())
	}
}
