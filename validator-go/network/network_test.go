package network

import (
	"math/big"
	"testing"
)

func TestChainFromID(t *testing.T) {
	cases := []struct {
		id      int64
		want    Chain
		wantErr bool
	}{
		{100, Gnosis, false},
		{11155111, Sepolia, false},
		{31337, Anvil, false},
		{1, 0, true},
		{0, 0, true},
	}

	for _, tc := range cases {
		chain, err := chainFromID(big.NewInt(tc.id))
		if tc.wantErr {
			if err == nil {
				t.Errorf("chainFromID(%d): expected error, got nil", tc.id)
			}
			continue
		}
		if err != nil {
			t.Errorf("chainFromID(%d): unexpected error: %v", tc.id, err)
			continue
		}
		if chain != tc.want {
			t.Errorf("chainFromID(%d) = %v, want %v", tc.id, chain, tc.want)
		}
	}
}

func TestDefaultBlocksPerEpoch(t *testing.T) {
	cases := []struct {
		chain Chain
		want  uint64
	}{
		{Gnosis, 1440},
		{Sepolia, 600},
		{Anvil, 60},
	}

	for _, tc := range cases {
		if got := tc.chain.DefaultBlocksPerEpoch(); got != tc.want {
			t.Errorf("%v.DefaultBlocksPerEpoch() = %d, want %d", tc.chain, got, tc.want)
		}
	}
}

func TestChainString(t *testing.T) {
	if got := Gnosis.String(); got != "Gnosis" {
		t.Errorf("Gnosis.String() = %q", got)
	}
	if got := Sepolia.String(); got != "Sepolia" {
		t.Errorf("Sepolia.String() = %q", got)
	}
	if got := Anvil.String(); got != "Anvil" {
		t.Errorf("Anvil.String() = %q", got)
	}
}
