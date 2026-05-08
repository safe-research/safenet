package state_test

import (
	"bytes"
	"encoding/json"
	"math/big"
	"strings"
	"testing"

	secp256k1 "github.com/decred/dcrd/dcrec/secp256k1/v4"
	"github.com/ethereum/go-ethereum/common"
	"github.com/safe-research/safenet/validator-go/contracts/coordinator"
	"github.com/safe-research/safenet/validator-go/frost"
	"github.com/safe-research/safenet/validator-go/secret"
	"github.com/safe-research/safenet/validator-go/state"
)

// Test participants.
var (
	addr1 = common.HexToAddress("0x1111111111111111111111111111111111111111")
	addr2 = common.HexToAddress("0x2222222222222222222222222222222222222222")
	addr3 = common.HexToAddress("0x3333333333333333333333333333333333333333")
)

// testConfig returns a ConsensusConfig for testing.
func testConfig() state.ConsensusConfig {
	var salt [32]byte
	for i := range salt {
		salt[i] = byte(i + 1)
	}
	return state.ConsensusConfig{
		OwnAddress:         addr1,
		CoordinatorAddress: addr2,
		Participants:       []common.Address{addr1, addr2, addr3},
		GenesisSalt:        salt,
		BlocksPerEpoch:     1440,
	}
}

// deterministicReader returns a repeating-byte reader large enough for test DKG rounds.
func deterministicReader(seed byte) *bytes.Reader {
	b := bytes.Repeat([]byte{seed + 1}, 256)
	return bytes.NewReader(b)
}

// buildRound1 runs DKG round 1 for participant addr.
func buildRound1(t *testing.T, addr common.Address, seed byte) *frost.Round1 {
	t.Helper()
	r, err := frost.GenerateRound1(addr, 3, 2, deterministicReader(seed))
	if err != nil {
		t.Fatalf("GenerateRound1: %v", err)
	}
	return r
}

// buildDKG runs a complete 2-of-3 DKG ceremony and returns the key package for addr1.
func buildDKG(t *testing.T) *frost.KeyPackage {
	t.Helper()

	r1s := [3]*frost.Round1{
		buildRound1(t, addr1, 0x01),
		buildRound1(t, addr2, 0x02),
		buildRound1(t, addr3, 0x03),
	}

	addrs := [3]common.Address{addr1, addr2, addr3}
	allCommitments := map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment{
		addr1: r1s[0].Commitment,
		addr2: r1s[1].Commitment,
		addr3: r1s[2].Commitment,
	}

	r2s := make([]*frost.Round2, 3)
	for i, r1 := range r1s {
		r2, err := frost.GenerateRound2(r1.EncryptionKey, r1.SecretPackage, allCommitments)
		if err != nil {
			t.Fatalf("GenerateRound2[%d]: %v", i, err)
		}
		r2s[i] = r2
	}

	allShares := map[common.Address]coordinator.FROSTCoordinatorKeyGenSecretShare{
		addrs[0]: r2s[0].Share,
		addrs[1]: r2s[1].Share,
		addrs[2]: r2s[2].Share,
	}

	r3, err := frost.GenerateRound3(r1s[0].EncryptionKey, r2s[0].SecretPackage, allCommitments, allShares)
	if err != nil {
		t.Fatalf("GenerateRound3: %v", err)
	}
	return r3.KeyPackage
}

// roundTrip marshals s to JSON and unmarshals it back.
func roundTrip(t *testing.T, s state.State) state.State {
	t.Helper()
	data, err := json.Marshal(s)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var got state.State
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatalf("unmarshal: %v\njson: %s", err, data)
	}
	return got
}

// assertConfigEqual checks two ConsensusConfig values are equal.
func assertConfigEqual(t *testing.T, got, want state.ConsensusConfig) {
	t.Helper()
	if got.OwnAddress != want.OwnAddress {
		t.Errorf("OwnAddress: got %s, want %s", got.OwnAddress, want.OwnAddress)
	}
	if got.CoordinatorAddress != want.CoordinatorAddress {
		t.Errorf("CoordinatorAddress: got %s, want %s", got.CoordinatorAddress, want.CoordinatorAddress)
	}
	if len(got.Participants) != len(want.Participants) {
		t.Errorf("Participants length: got %d, want %d", len(got.Participants), len(want.Participants))
	} else {
		for i := range got.Participants {
			if got.Participants[i] != want.Participants[i] {
				t.Errorf("Participants[%d]: got %s, want %s", i, got.Participants[i], want.Participants[i])
			}
		}
	}
	if got.GenesisSalt != want.GenesisSalt {
		t.Errorf("GenesisSalt: got %x, want %x", got.GenesisSalt, want.GenesisSalt)
	}
	if got.BlocksPerEpoch != want.BlocksPerEpoch {
		t.Errorf("BlocksPerEpoch: got %d, want %d", got.BlocksPerEpoch, want.BlocksPerEpoch)
	}
}

func TestConsensusConfigRoundTrip(t *testing.T) {
	cfg := testConfig()
	data, err := json.Marshal(cfg)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var got state.ConsensusConfig
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	assertConfigEqual(t, got, cfg)
}

func TestConsensusConfigJSONShape(t *testing.T) {
	cfg := testConfig()
	data, err := json.Marshal(cfg)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	// genesis_salt must appear as a "0x..." hex string, not base64 or an array.
	if !strings.Contains(string(data), `"genesis_salt":"0x`) {
		t.Errorf("genesis_salt not hex-encoded: %s", data)
	}
}

func TestWaitingForGenesisRoundTrip(t *testing.T) {
	s := state.State{Config: testConfig(), Phase: state.WaitingForGenesis{}}
	got := roundTrip(t, s)
	assertConfigEqual(t, got.Config, s.Config)
	if _, ok := got.Phase.(state.WaitingForGenesis); !ok {
		t.Errorf("phase: got %T, want WaitingForGenesis", got.Phase)
	}
}

func TestWaitingForRolloverRoundTrip(t *testing.T) {
	s := state.State{Config: testConfig(), Phase: state.WaitingForRollover{}}
	got := roundTrip(t, s)
	assertConfigEqual(t, got.Config, s.Config)
	if _, ok := got.Phase.(state.WaitingForRollover); !ok {
		t.Errorf("phase: got %T, want WaitingForRollover", got.Phase)
	}
}

func TestCollectingCommitmentsRoundTrip(t *testing.T) {
	r1 := buildRound1(t, addr1, 0x01)

	commitments := map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment{
		addr1: r1.Commitment,
	}

	orig := state.State{
		Config: testConfig(),
		Phase: state.CollectingCommitments{
			Round1Secret:  r1.SecretPackage,
			EncryptionKey: r1.EncryptionKey,
			Commitments:   commitments,
		},
	}
	got := roundTrip(t, orig)
	assertConfigEqual(t, got.Config, orig.Config)

	p, ok := got.Phase.(state.CollectingCommitments)
	if !ok {
		t.Fatalf("phase: got %T, want CollectingCommitments", got.Phase)
	}

	assertRound1SecretEqual(t, p.Round1Secret, r1.SecretPackage)
	assertEncryptionKeyEqual(t, p.EncryptionKey, r1.EncryptionKey)
	assertCommitmentsEqual(t, p.Commitments, commitments)
}

func TestCollectingSharesRoundTrip(t *testing.T) {
	r1 := buildRound1(t, addr1, 0x01)

	allCommitments := map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment{
		addr1: buildRound1(t, addr1, 0x01).Commitment,
		addr2: buildRound1(t, addr2, 0x02).Commitment,
		addr3: buildRound1(t, addr3, 0x03).Commitment,
	}
	r2, err := frost.GenerateRound2(r1.EncryptionKey, r1.SecretPackage, allCommitments)
	if err != nil {
		t.Fatalf("GenerateRound2: %v", err)
	}

	shares := map[common.Address]coordinator.FROSTCoordinatorKeyGenSecretShare{
		addr1: r2.Share,
	}

	orig := state.State{
		Config: testConfig(),
		Phase: state.CollectingShares{
			Round2Secret:  r2.SecretPackage,
			EncryptionKey: r1.EncryptionKey,
			Commitments:   allCommitments,
			Shares:        shares,
		},
	}
	got := roundTrip(t, orig)
	assertConfigEqual(t, got.Config, orig.Config)

	p, ok := got.Phase.(state.CollectingShares)
	if !ok {
		t.Fatalf("phase: got %T, want CollectingShares", got.Phase)
	}

	assertRound2SecretEqual(t, p.Round2Secret, r2.SecretPackage)
	assertEncryptionKeyEqual(t, p.EncryptionKey, r1.EncryptionKey)
	assertCommitmentsEqual(t, p.Commitments, allCommitments)
	assertSharesEqual(t, p.Shares, shares)
}

func TestGenesisCompleteRoundTrip(t *testing.T) {
	kp := buildDKG(t)

	orig := state.State{
		Config: testConfig(),
		Phase:  state.GenesisComplete{KeyPackage: kp},
	}
	got := roundTrip(t, orig)
	assertConfigEqual(t, got.Config, orig.Config)

	p, ok := got.Phase.(state.GenesisComplete)
	if !ok {
		t.Fatalf("phase: got %T, want GenesisComplete", got.Phase)
	}
	assertKeyPackageEqual(t, p.KeyPackage, kp)
}

func TestUnknownPhaseError(t *testing.T) {
	raw := `{"config":{"own_address":"0x0000000000000000000000000000000000000000","coordinator_address":"0x0000000000000000000000000000000000000000","participants":[],"genesis_salt":"0x0000000000000000000000000000000000000000000000000000000000000000","blocks_per_epoch":0},"phase":"UnknownPhase"}`
	var s state.State
	if err := json.Unmarshal([]byte(raw), &s); err == nil {
		t.Error("expected error for unknown phase name")
	}
}

// ---- comparison helpers ----

func assertScalarEqual(t *testing.T, label string, got, want *secp256k1.ModNScalar) {
	t.Helper()
	if !got.Equals(want) {
		t.Errorf("%s: scalars differ: got %x, want %x", label, got.Bytes(), want.Bytes())
	}
}

func assertPubKeyEqual(t *testing.T, label string, got, want *secp256k1.PublicKey) {
	t.Helper()
	if !got.IsEqual(want) {
		t.Errorf("%s: public keys differ", label)
	}
}

func assertEncryptionKeyEqual(t *testing.T, got, want *secret.EncryptionKey) {
	t.Helper()
	gotBytes := got.Bytes()
	wantBytes := want.Bytes()
	if gotBytes != wantBytes {
		t.Errorf("encryption key: got %x, want %x", gotBytes, wantBytes)
	}
}

func assertRound1SecretEqual(t *testing.T, got, want *frost.Round1SecretPackage) {
	t.Helper()
	assertScalarEqual(t, "round1.Identifier", got.Identifier, want.Identifier)
	if len(got.Coefficients) != len(want.Coefficients) {
		t.Fatalf("round1.Coefficients length: got %d, want %d", len(got.Coefficients), len(want.Coefficients))
	}
	for i := range got.Coefficients {
		assertScalarEqual(t, "round1.Coefficients[%d]", got.Coefficients[i], want.Coefficients[i])
	}
	if len(got.Commitments) != len(want.Commitments) {
		t.Fatalf("round1.Commitments length: got %d, want %d", len(got.Commitments), len(want.Commitments))
	}
	for i := range got.Commitments {
		assertPubKeyEqual(t, "round1.Commitments[%d]", got.Commitments[i], want.Commitments[i])
	}
}

func assertRound2SecretEqual(t *testing.T, got, want *frost.Round2SecretPackage) {
	t.Helper()
	assertScalarEqual(t, "round2.Identifier", got.Identifier, want.Identifier)
	assertScalarEqual(t, "round2.OwnSigningShare", got.OwnSigningShare, want.OwnSigningShare)
}

func assertKeyPackageEqual(t *testing.T, got, want *frost.KeyPackage) {
	t.Helper()
	assertScalarEqual(t, "kp.Identifier", got.Identifier, want.Identifier)
	assertScalarEqual(t, "kp.SigningShare", got.SigningShare, want.SigningShare)
	assertPubKeyEqual(t, "kp.VerifyingShare", got.VerifyingShare, want.VerifyingShare)
	assertPubKeyEqual(t, "kp.GroupPublicKey", got.GroupPublicKey, want.GroupPublicKey)
	if len(got.ParticipantVerifyingKeys) != len(want.ParticipantVerifyingKeys) {
		t.Errorf("kp.ParticipantVerifyingKeys length: got %d, want %d",
			len(got.ParticipantVerifyingKeys), len(want.ParticipantVerifyingKeys))
		return
	}
	for addr, wantKey := range want.ParticipantVerifyingKeys {
		gotKey, ok := got.ParticipantVerifyingKeys[addr]
		if !ok {
			t.Errorf("kp.ParticipantVerifyingKeys missing %s", addr)
			continue
		}
		assertPubKeyEqual(t, "kp.ParticipantVerifyingKeys["+addr.Hex()+"]", gotKey, wantKey)
	}
}

func assertCommitmentsEqual(t *testing.T, got, want map[common.Address]coordinator.FROSTCoordinatorKeyGenCommitment) {
	t.Helper()
	if len(got) != len(want) {
		t.Errorf("commitments length: got %d, want %d", len(got), len(want))
		return
	}
	for addr, wantC := range want {
		gotC, ok := got[addr]
		if !ok {
			t.Errorf("commitments missing %s", addr)
			continue
		}
		assertPointEqual(t, "Q", gotC.Q, wantC.Q)
		assertPointEqual(t, "R", gotC.R, wantC.R)
		assertBigIntEqual(t, "Mu", gotC.Mu, wantC.Mu)
		if len(gotC.C) != len(wantC.C) {
			t.Errorf("commitment C length for %s: got %d, want %d", addr, len(gotC.C), len(wantC.C))
			continue
		}
		for i := range gotC.C {
			assertPointEqual(t, "C[i]", gotC.C[i], wantC.C[i])
		}
	}
}

func assertSharesEqual(t *testing.T, got, want map[common.Address]coordinator.FROSTCoordinatorKeyGenSecretShare) {
	t.Helper()
	if len(got) != len(want) {
		t.Errorf("shares length: got %d, want %d", len(got), len(want))
		return
	}
	for addr, wantS := range want {
		gotS, ok := got[addr]
		if !ok {
			t.Errorf("shares missing %s", addr)
			continue
		}
		assertPointEqual(t, "Y", gotS.Y, wantS.Y)
		if len(gotS.F) != len(wantS.F) {
			t.Errorf("share F length for %s: got %d, want %d", addr, len(gotS.F), len(wantS.F))
			continue
		}
		for i := range gotS.F {
			assertBigIntEqual(t, "F[i]", gotS.F[i], wantS.F[i])
		}
	}
}

func assertPointEqual(t *testing.T, label string, got, want coordinator.Secp256k1Point) {
	t.Helper()
	if got.X.Cmp(want.X) != 0 || got.Y.Cmp(want.Y) != 0 {
		t.Errorf("point %s: got (%s,%s), want (%s,%s)", label, got.X, got.Y, want.X, want.Y)
	}
}

func assertBigIntEqual(t *testing.T, label string, got, want *big.Int) {
	t.Helper()
	if got.Cmp(want) != 0 {
		t.Errorf("big.Int %s: got %s, want %s", label, got, want)
	}
}
