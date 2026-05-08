package frost

import (
	"encoding/json"
	"fmt"

	secp256k1 "github.com/decred/dcrd/dcrec/secp256k1/v4"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/common/hexutil"
)

// JSON marshaling for Round1SecretPackage.

type round1SecretPackageJSON struct {
	Identifier   string   `json:"identifier"`
	Coefficients []string `json:"coefficients"`
	Commitments  []string `json:"commitments"`
}

func (p *Round1SecretPackage) MarshalJSON() ([]byte, error) {
	j := round1SecretPackageJSON{
		Identifier:   marshalScalar(p.Identifier),
		Coefficients: make([]string, len(p.Coefficients)),
		Commitments:  make([]string, len(p.Commitments)),
	}
	for i, c := range p.Coefficients {
		j.Coefficients[i] = marshalScalar(c)
	}
	for i, c := range p.Commitments {
		j.Commitments[i] = marshalPubKey(c)
	}
	return json.Marshal(j)
}

func (p *Round1SecretPackage) UnmarshalJSON(data []byte) error {
	var j round1SecretPackageJSON
	if err := json.Unmarshal(data, &j); err != nil {
		return err
	}
	var err error
	p.Identifier, err = unmarshalScalar(j.Identifier)
	if err != nil {
		return fmt.Errorf("identifier: %w", err)
	}
	p.Coefficients = make([]*secp256k1.ModNScalar, len(j.Coefficients))
	for i, s := range j.Coefficients {
		p.Coefficients[i], err = unmarshalScalar(s)
		if err != nil {
			return fmt.Errorf("coefficient %d: %w", i, err)
		}
	}
	p.Commitments = make([]*secp256k1.PublicKey, len(j.Commitments))
	for i, s := range j.Commitments {
		p.Commitments[i], err = unmarshalPubKey(s)
		if err != nil {
			return fmt.Errorf("commitment %d: %w", i, err)
		}
	}
	return nil
}

// JSON marshaling for Round2SecretPackage.

type round2SecretPackageJSON struct {
	Identifier      string `json:"identifier"`
	OwnSigningShare string `json:"own_signing_share"`
}

func (p *Round2SecretPackage) MarshalJSON() ([]byte, error) {
	return json.Marshal(round2SecretPackageJSON{
		Identifier:      marshalScalar(p.Identifier),
		OwnSigningShare: marshalScalar(p.OwnSigningShare),
	})
}

func (p *Round2SecretPackage) UnmarshalJSON(data []byte) error {
	var j round2SecretPackageJSON
	if err := json.Unmarshal(data, &j); err != nil {
		return err
	}
	var err error
	p.Identifier, err = unmarshalScalar(j.Identifier)
	if err != nil {
		return fmt.Errorf("identifier: %w", err)
	}
	p.OwnSigningShare, err = unmarshalScalar(j.OwnSigningShare)
	if err != nil {
		return fmt.Errorf("own_signing_share: %w", err)
	}
	return nil
}

// JSON marshaling for KeyPackage.

type keyPackageJSON struct {
	Identifier               string            `json:"identifier"`
	SigningShare             string            `json:"signing_share"`
	VerifyingShare          string            `json:"verifying_share"`
	GroupPublicKey          string            `json:"group_public_key"`
	ParticipantVerifyingKeys map[string]string `json:"participant_verifying_keys"`
}

func (p *KeyPackage) MarshalJSON() ([]byte, error) {
	j := keyPackageJSON{
		Identifier:               marshalScalar(p.Identifier),
		SigningShare:             marshalScalar(p.SigningShare),
		VerifyingShare:          marshalPubKey(p.VerifyingShare),
		GroupPublicKey:          marshalPubKey(p.GroupPublicKey),
		ParticipantVerifyingKeys: make(map[string]string, len(p.ParticipantVerifyingKeys)),
	}
	for addr, key := range p.ParticipantVerifyingKeys {
		j.ParticipantVerifyingKeys[addr.Hex()] = marshalPubKey(key)
	}
	return json.Marshal(j)
}

func (p *KeyPackage) UnmarshalJSON(data []byte) error {
	var j keyPackageJSON
	if err := json.Unmarshal(data, &j); err != nil {
		return err
	}
	var err error
	p.Identifier, err = unmarshalScalar(j.Identifier)
	if err != nil {
		return fmt.Errorf("identifier: %w", err)
	}
	p.SigningShare, err = unmarshalScalar(j.SigningShare)
	if err != nil {
		return fmt.Errorf("signing_share: %w", err)
	}
	p.VerifyingShare, err = unmarshalPubKey(j.VerifyingShare)
	if err != nil {
		return fmt.Errorf("verifying_share: %w", err)
	}
	p.GroupPublicKey, err = unmarshalPubKey(j.GroupPublicKey)
	if err != nil {
		return fmt.Errorf("group_public_key: %w", err)
	}
	p.ParticipantVerifyingKeys = make(map[common.Address]*secp256k1.PublicKey, len(j.ParticipantVerifyingKeys))
	for addrStr, keyStr := range j.ParticipantVerifyingKeys {
		key, err := unmarshalPubKey(keyStr)
		if err != nil {
			return fmt.Errorf("participant key for %s: %w", addrStr, err)
		}
		p.ParticipantVerifyingKeys[common.HexToAddress(addrStr)] = key
	}
	return nil
}

func marshalScalar(s *secp256k1.ModNScalar) string {
	b := s.Bytes()
	return hexutil.Encode(b[:])
}

func unmarshalScalar(s string) (*secp256k1.ModNScalar, error) {
	b, err := hexutil.Decode(s)
	if err != nil {
		return nil, err
	}
	if len(b) != 32 {
		return nil, fmt.Errorf("expected 32 bytes, got %d", len(b))
	}
	var arr [32]byte
	copy(arr[:], b)
	sc := new(secp256k1.ModNScalar)
	sc.SetBytes(&arr)
	return sc, nil
}

func marshalPubKey(k *secp256k1.PublicKey) string {
	return hexutil.Encode(k.SerializeCompressed())
}

func unmarshalPubKey(s string) (*secp256k1.PublicKey, error) {
	b, err := hexutil.Decode(s)
	if err != nil {
		return nil, err
	}
	return secp256k1.ParsePubKey(b)
}
