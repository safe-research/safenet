package network

import (
	"context"
	"fmt"
	"math/big"

	"github.com/ethereum/go-ethereum/accounts/abi/bind"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/ethclient"
	"github.com/safe-research/safenet/validator-go/contracts/consensus"
)

// Chain represents a supported EVM network.
type Chain int

const (
	Gnosis  Chain = iota
	Sepolia Chain = iota
	Anvil   Chain = iota
)

func (c Chain) String() string {
	switch c {
	case Gnosis:
		return "Gnosis"
	case Sepolia:
		return "Sepolia"
	case Anvil:
		return "Anvil"
	default:
		return fmt.Sprintf("Chain(%d)", int(c))
	}
}

// DefaultBlocksPerEpoch returns the canonical epoch length for the chain.
func (c Chain) DefaultBlocksPerEpoch() uint64 {
	switch c {
	case Gnosis:
		return 1440
	case Sepolia:
		return 600
	case Anvil:
		return 60
	default:
		panic(fmt.Sprintf("unknown chain %d", int(c)))
	}
}

// chainFromID maps an EVM chain ID to a supported Chain.
func chainFromID(id *big.Int) (Chain, error) {
	switch id.Int64() {
	case 100:
		return Gnosis, nil
	case 11155111:
		return Sepolia, nil
	case 31337:
		return Anvil, nil
	default:
		return 0, fmt.Errorf("unsupported chain ID %s", id)
	}
}

// Addresses holds the resolved on-chain addresses and chain parameters.
type Addresses struct {
	Chain              Chain
	BlocksPerEpoch     uint64
	ConsensusAddress   common.Address
	CoordinatorAddress common.Address
}

// Resolve connects to the RPC, detects the chain, and queries the consensus
// contract for the coordinator address.
func Resolve(ctx context.Context, rpcURL string, consensusAddr common.Address, blocksPerEpochOverride *uint64) (*Addresses, error) {
	client, err := ethclient.DialContext(ctx, rpcURL)
	if err != nil {
		return nil, fmt.Errorf("dial %s: %w", rpcURL, err)
	}
	defer client.Close()

	chainID, err := client.ChainID(ctx)
	if err != nil {
		return nil, fmt.Errorf("eth_chainId: %w", err)
	}

	chain, err := chainFromID(chainID)
	if err != nil {
		return nil, err
	}

	coordinator, err := callGetCoordinator(ctx, client, consensusAddr)
	if err != nil {
		return nil, fmt.Errorf("getCoordinator: %w", err)
	}

	bpe := chain.DefaultBlocksPerEpoch()
	if blocksPerEpochOverride != nil {
		bpe = *blocksPerEpochOverride
	}

	return &Addresses{
		Chain:              chain,
		BlocksPerEpoch:     bpe,
		ConsensusAddress:   consensusAddr,
		CoordinatorAddress: coordinator,
	}, nil
}

func callGetCoordinator(ctx context.Context, client *ethclient.Client, consensusAddr common.Address) (common.Address, error) {
	caller, err := consensus.NewConsensusCaller(consensusAddr, client)
	if err != nil {
		return common.Address{}, err
	}
	return caller.GetCoordinator(&bind.CallOpts{Context: ctx})
}
