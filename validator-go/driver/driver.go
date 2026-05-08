package driver

import (
	"context"
	"crypto/ecdsa"
	"encoding/hex"
	"errors"
	"fmt"
	"log"
	"strings"

	"github.com/ethereum/go-ethereum/accounts/abi"
	"github.com/ethereum/go-ethereum/common"
	ethcrypto "github.com/ethereum/go-ethereum/crypto"
	"github.com/ethereum/go-ethereum/ethclient"
	"github.com/safe-research/safenet/validator-go/action"
	"github.com/safe-research/safenet/validator-go/config"
	coordbindings "github.com/safe-research/safenet/validator-go/contracts/coordinator"
	"github.com/safe-research/safenet/validator-go/network"
	"github.com/safe-research/safenet/validator-go/state"
	"github.com/safe-research/safenet/validator-go/statemachine"
	"github.com/safe-research/safenet/validator-go/storage"
	"github.com/safe-research/safenet/validator-go/watcher"
)

// Run starts the validator for the given config. It blocks until ctx is
// cancelled or a fatal error occurs.
func Run(ctx context.Context, cfg *config.Config) error {
	// Derive own address from private key.
	privKey, err := parsePrivateKey(cfg.PrivateKey)
	if err != nil {
		return fmt.Errorf("parse private key: %w", err)
	}
	ownAddress := ethcrypto.PubkeyToAddress(privKey.PublicKey)

	// Connect to RPC (kept open for the lifetime of the validator).
	client, err := ethclient.DialContext(ctx, cfg.RPCURL)
	if err != nil {
		return fmt.Errorf("dial %s: %w", cfg.RPCURL, err)
	}
	defer client.Close()

	// Detect chain and resolve coordinator address using the same client.
	addrs, err := network.ResolveWithClient(ctx, client, cfg.ConsensusAddress, cfg.BlocksPerEpoch)
	if err != nil {
		return fmt.Errorf("resolve addresses: %w", err)
	}
	log.Printf("chain=%s blocks_per_epoch=%d own=%s coordinator=%s",
		addrs.Chain, addrs.BlocksPerEpoch, ownAddress.Hex(), addrs.CoordinatorAddress.Hex())

	// Build participant list and genesis salt.
	participants := make([]common.Address, len(cfg.Participants))
	for i, p := range cfg.Participants {
		participants[i] = p.Address
	}
	var genesisSalt [32]byte
	if cfg.GenesisSalt != nil {
		genesisSalt = *cfg.GenesisSalt
	}
	consensusCfg := state.ConsensusConfig{
		OwnAddress:         ownAddress,
		CoordinatorAddress: addrs.CoordinatorAddress,
		Participants:       participants,
		GenesisSalt:        genesisSalt,
		BlocksPerEpoch:     addrs.BlocksPerEpoch,
	}

	// Open storage and restore or initialise state.
	var stateHistory uint = 5
	if cfg.StateHistory != nil {
		stateHistory = *cfg.StateHistory
	}
	store, err := storage.Open(cfg.StorageFile, stateHistory)
	if err != nil {
		return fmt.Errorf("open storage: %w", err)
	}
	savedBlock, savedState, err := store.LoadLatest()
	if err != nil {
		return fmt.Errorf("load state: %w", err)
	}
	var current state.State
	var startBlock uint64
	if savedState != nil {
		current = *savedState
		startBlock = savedBlock + 1
		log.Printf("restored state from block %d (phase=%T)", savedBlock, current.Phase)
	} else {
		current = state.State{Config: consensusCfg, Phase: state.WaitingForGenesis{}}
		log.Printf("initialised fresh state (phase=WaitingForGenesis)")
	}

	// Build action handler.
	chainID, err := client.ChainID(ctx)
	if err != nil {
		return fmt.Errorf("get chain ID: %w", err)
	}
	handler, err := action.NewHandler(client, addrs.CoordinatorAddress, privKey, chainID)
	if err != nil {
		return fmt.Errorf("create action handler: %w", err)
	}

	// Drain actions asynchronously; blocking in the watcher callback would
	// stall block processing while waiting for transaction confirmations.
	actionCh := make(chan action.Action, 64)
	go func() {
		for act := range actionCh {
			if _, err := handler.Send(ctx, act); err != nil && !errors.Is(err, context.Canceled) {
				log.Printf("action send error: %v", err)
			}
		}
	}()

	// Build coordinator log parser.
	filterer, err := coordbindings.NewFROSTCoordinatorFilterer(addrs.CoordinatorAddress, client)
	if err != nil {
		return fmt.Errorf("create coordinator filterer: %w", err)
	}
	topics, err := parseEventTopics()
	if err != nil {
		return fmt.Errorf("parse event topics: %w", err)
	}

	// Run watcher; callback is invoked once per block in ascending order.
	w := watcher.New(client, addrs.CoordinatorAddress)
	return w.Run(ctx, startBlock, func(u watcher.BlockUpdate) {
		for _, lg := range u.Logs {
			if len(lg.Topics) == 0 {
				continue
			}
			var (
				acts []action.Action
				smErr error
			)
			switch lg.Topics[0] {
			case topics.keyGen:
				ev, err := filterer.ParseKeyGen(lg)
				if err != nil {
					log.Printf("block %d: parse KeyGen: %v", u.BlockNumber, err)
					continue
				}
				current, acts, smErr = statemachine.HandleKeyGen(current, *ev, nil)
			case topics.keyGenCommitted:
				ev, err := filterer.ParseKeyGenCommitted(lg)
				if err != nil {
					log.Printf("block %d: parse KeyGenCommitted: %v", u.BlockNumber, err)
					continue
				}
				current, acts, smErr = statemachine.HandleKeyGenCommitted(current, *ev)
			case topics.keyGenSecretShared:
				ev, err := filterer.ParseKeyGenSecretShared(lg)
				if err != nil {
					log.Printf("block %d: parse KeyGenSecretShared: %v", u.BlockNumber, err)
					continue
				}
				current, acts, smErr = statemachine.HandleKeyGenSecretShared(current, *ev)
			default:
				continue
			}
			if smErr != nil {
				log.Printf("block %d: state machine error: %v", u.BlockNumber, smErr)
				continue
			}
			for _, a := range acts {
				select {
				case actionCh <- a:
				default:
					log.Printf("block %d: action channel full, dropping %T", u.BlockNumber, a)
				}
			}
		}
		if err := store.Save(u.BlockNumber, &current); err != nil {
			log.Printf("block %d: save state: %v", u.BlockNumber, err)
		}
	})
}

type eventTopics struct {
	keyGen             common.Hash
	keyGenCommitted    common.Hash
	keyGenSecretShared common.Hash
}

func parseEventTopics() (eventTopics, error) {
	parsedABI, err := abi.JSON(strings.NewReader(coordbindings.FROSTCoordinatorABI))
	if err != nil {
		return eventTopics{}, fmt.Errorf("parse coordinator ABI: %w", err)
	}
	return eventTopics{
		keyGen:             parsedABI.Events["KeyGen"].ID,
		keyGenCommitted:    parsedABI.Events["KeyGenCommitted"].ID,
		keyGenSecretShared: parsedABI.Events["KeyGenSecretShared"].ID,
	}, nil
}

func parsePrivateKey(hexKey string) (*ecdsa.PrivateKey, error) {
	b, err := hex.DecodeString(strings.TrimPrefix(hexKey, "0x"))
	if err != nil {
		return nil, err
	}
	return ethcrypto.ToECDSA(b)
}
