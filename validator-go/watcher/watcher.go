package watcher

import (
	"context"
	"fmt"
	"math/big"
	"sort"

	"github.com/ethereum/go-ethereum"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"
)

// ChainClient is the subset of ethclient.Client that the Watcher uses.
type ChainClient interface {
	SubscribeNewHead(ctx context.Context, ch chan<- *types.Header) (ethereum.Subscription, error)
	BlockNumber(ctx context.Context) (uint64, error)
	FilterLogs(ctx context.Context, q ethereum.FilterQuery) ([]types.Log, error)
}

// BlockUpdate contains a block number and all coordinator contract logs for
// that block, sorted in ascending order by log index.
type BlockUpdate struct {
	BlockNumber uint64
	Logs        []types.Log
}

// Watcher subscribes to coordinator contract logs and delivers one BlockUpdate
// per block to the caller's callback.
type Watcher struct {
	client    ChainClient
	coordAddr common.Address
}

// New creates a Watcher that watches the coordinator contract at coordAddr.
func New(client ChainClient, coordAddr common.Address) *Watcher {
	return &Watcher{client: client, coordAddr: coordAddr}
}

// Run starts the watcher from startBlock. The live subscription is established
// before history replay so no blocks are missed. onBlock is called once for
// every block from startBlock onwards (including blocks with no logs). Blocks
// are delivered in strictly ascending order. Run blocks until ctx is cancelled
// or a subscription error occurs.
func (w *Watcher) Run(ctx context.Context, startBlock uint64, onBlock func(BlockUpdate)) error {
	// Establish the live subscription FIRST to avoid a gap between replay and
	// the live feed.
	headers := make(chan *types.Header, 128)
	sub, err := w.client.SubscribeNewHead(ctx, headers)
	if err != nil {
		return fmt.Errorf("subscribe new head: %w", err)
	}
	defer sub.Unsubscribe()

	// Snapshot the current chain tip; replay covers [startBlock, currentHead].
	currentHead, err := w.client.BlockNumber(ctx)
	if err != nil {
		return fmt.Errorf("get block number: %w", err)
	}

	if startBlock <= currentHead {
		if err := w.replay(ctx, startBlock, currentHead, onBlock); err != nil {
			return fmt.Errorf("history replay: %w", err)
		}
	}

	// Process live blocks. Any header ≤ currentHead was already delivered
	// by replay and is silently skipped to enforce monotonicity.
	lastBlock := currentHead
	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case err := <-sub.Err():
			return fmt.Errorf("subscription: %w", err)
		case header := <-headers:
			n := header.Number.Uint64()
			if n <= lastBlock {
				continue
			}
			// Fill any gap caused by a missed subscription delivery.
			for b := lastBlock + 1; b < n; b++ {
				if err := w.processBlock(ctx, b, onBlock); err != nil {
					return fmt.Errorf("process block %d: %w", b, err)
				}
				lastBlock = b
			}
			if err := w.processBlock(ctx, n, onBlock); err != nil {
				return fmt.Errorf("process block %d: %w", n, err)
			}
			lastBlock = n
		}
	}
}

// replay fetches all coordinator logs in [from, to] via eth_getLogs, groups
// them by block, and emits one BlockUpdate per block in ascending order.
func (w *Watcher) replay(ctx context.Context, from, to uint64, onBlock func(BlockUpdate)) error {
	logs, err := w.filterLogs(ctx, from, to)
	if err != nil {
		return err
	}

	byBlock := make(map[uint64][]types.Log, len(logs))
	for _, l := range logs {
		byBlock[l.BlockNumber] = append(byBlock[l.BlockNumber], l)
	}

	for b := from; b <= to; b++ {
		ls := byBlock[b]
		sort.Slice(ls, func(i, j int) bool { return ls[i].Index < ls[j].Index })
		onBlock(BlockUpdate{BlockNumber: b, Logs: ls})
	}
	return nil
}

// processBlock fetches coordinator logs for a single block and emits one
// BlockUpdate with logs sorted by index.
func (w *Watcher) processBlock(ctx context.Context, blockNumber uint64, onBlock func(BlockUpdate)) error {
	logs, err := w.filterLogs(ctx, blockNumber, blockNumber)
	if err != nil {
		return err
	}
	sort.Slice(logs, func(i, j int) bool { return logs[i].Index < logs[j].Index })
	onBlock(BlockUpdate{BlockNumber: blockNumber, Logs: logs})
	return nil
}

func (w *Watcher) filterLogs(ctx context.Context, from, to uint64) ([]types.Log, error) {
	return w.client.FilterLogs(ctx, ethereum.FilterQuery{
		FromBlock: new(big.Int).SetUint64(from),
		ToBlock:   new(big.Int).SetUint64(to),
		Addresses: []common.Address{w.coordAddr},
	})
}
