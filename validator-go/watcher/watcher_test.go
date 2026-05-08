package watcher_test

import (
	"context"
	"math/big"
	"testing"

	"github.com/ethereum/go-ethereum"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/safe-research/safenet/validator-go/watcher"
)

// mockSub is a trivial ethereum.Subscription that never errors.
type mockSub struct{ errCh chan error }

func newMockSub() *mockSub          { return &mockSub{errCh: make(chan error)} }
func (s *mockSub) Unsubscribe()     {}
func (s *mockSub) Err() <-chan error { return s.errCh }

// mockClient implements watcher.ChainClient with configurable history logs
// and an optional stream of live block headers.
type mockClient struct {
	currentHead uint64
	logs        []types.Log
	liveHeaders []*types.Header
}

func (c *mockClient) SubscribeNewHead(ctx context.Context, ch chan<- *types.Header) (ethereum.Subscription, error) {
	go func() {
		for _, h := range c.liveHeaders {
			select {
			case ch <- h:
			case <-ctx.Done():
				return
			}
		}
	}()
	return newMockSub(), nil
}

func (c *mockClient) BlockNumber(_ context.Context) (uint64, error) {
	return c.currentHead, nil
}

func (c *mockClient) FilterLogs(_ context.Context, q ethereum.FilterQuery) ([]types.Log, error) {
	from := q.FromBlock.Uint64()
	to := q.ToBlock.Uint64()
	var out []types.Log
	for _, l := range c.logs {
		if l.BlockNumber >= from && l.BlockNumber <= to {
			out = append(out, l)
		}
	}
	return out, nil
}

// makeHeader constructs a types.Header with just the block number set.
func makeHeader(n uint64) *types.Header {
	return &types.Header{Number: new(big.Int).SetUint64(n)}
}

// runUntilBlock runs the watcher and cancels the context once stopBlock has
// been delivered. Returns the collected updates.
func runUntilBlock(t *testing.T, w *watcher.Watcher, startBlock, stopBlock uint64) []watcher.BlockUpdate {
	t.Helper()
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	var updates []watcher.BlockUpdate
	done := make(chan struct{})
	go func() {
		defer close(done)
		w.Run(ctx, startBlock, func(u watcher.BlockUpdate) { //nolint:errcheck
			updates = append(updates, u)
			if u.BlockNumber >= stopBlock {
				cancel()
			}
		})
	}()
	<-done
	return updates
}

func blockNumbers(updates []watcher.BlockUpdate) []uint64 {
	ns := make([]uint64, len(updates))
	for i, u := range updates {
		ns[i] = u.BlockNumber
	}
	return ns
}

func TestReplayDeliversEveryBlockInOrder(t *testing.T) {
	// Blocks 10–12; logs present only in 10 and 12 (block 11 is empty).
	client := &mockClient{
		currentHead: 12,
		logs: []types.Log{
			{BlockNumber: 10, Index: 0},
			{BlockNumber: 12, Index: 0},
		},
	}
	updates := runUntilBlock(t, watcher.New(client, common.Address{}), 10, 12)

	if len(updates) != 3 {
		t.Fatalf("expected 3 updates (blocks 10-12), got %d", len(updates))
	}
	for i, u := range updates {
		if want := uint64(10 + i); u.BlockNumber != want {
			t.Errorf("updates[%d].BlockNumber = %d, want %d", i, u.BlockNumber, want)
		}
	}
	if len(updates[1].Logs) != 0 {
		t.Errorf("block 11: expected 0 logs, got %d", len(updates[1].Logs))
	}
	if len(updates[0].Logs) != 1 {
		t.Errorf("block 10: expected 1 log, got %d", len(updates[0].Logs))
	}
	if len(updates[2].Logs) != 1 {
		t.Errorf("block 12: expected 1 log, got %d", len(updates[2].Logs))
	}
}

func TestReplayLogsAreSortedByIndex(t *testing.T) {
	client := &mockClient{
		currentHead: 5,
		logs: []types.Log{
			{BlockNumber: 5, Index: 3},
			{BlockNumber: 5, Index: 1},
			{BlockNumber: 5, Index: 2},
		},
	}
	updates := runUntilBlock(t, watcher.New(client, common.Address{}), 5, 5)

	if len(updates) == 0 {
		t.Fatal("expected at least one update")
	}
	logs := updates[len(updates)-1].Logs
	if len(logs) != 3 {
		t.Fatalf("expected 3 logs, got %d", len(logs))
	}
	for i := range logs {
		if wantIdx := uint(i + 1); logs[i].Index != wantIdx {
			t.Errorf("log[%d].Index = %d, want %d", i, logs[i].Index, wantIdx)
		}
	}
}

func TestReplaySkipsWhenStartBlockAheadOfHead(t *testing.T) {
	client := &mockClient{currentHead: 5}
	ctx, cancel := context.WithCancel(context.Background())
	cancel() // cancel immediately — Run should exit without calling onBlock

	var called int
	watcher.New(client, common.Address{}).Run(ctx, 10, func(_ watcher.BlockUpdate) { //nolint:errcheck
		called++
	})
	if called != 0 {
		t.Errorf("expected 0 updates when startBlock > currentHead, got %d", called)
	}
}

func TestLiveBlocksDeliveredAfterReplay(t *testing.T) {
	// History: block 1. Live: blocks 2 and 3.
	client := &mockClient{
		currentHead: 1,
		logs:        []types.Log{{BlockNumber: 1, Index: 0}},
		liveHeaders: []*types.Header{makeHeader(2), makeHeader(3)},
	}
	updates := runUntilBlock(t, watcher.New(client, common.Address{}), 1, 3)

	if len(updates) < 3 {
		t.Fatalf("expected at least 3 updates (blocks 1-3), got %d", len(updates))
	}
	for i, u := range updates[:3] {
		if want := uint64(1 + i); u.BlockNumber != want {
			t.Errorf("updates[%d].BlockNumber = %d, want %d", i, u.BlockNumber, want)
		}
	}
}

func TestLiveBlocksSkipAlreadyReplayedBlock(t *testing.T) {
	// History covers block 5; subscription also delivers block 5 as a duplicate.
	client := &mockClient{
		currentHead: 5,
		logs:        []types.Log{{BlockNumber: 5, Index: 0}},
		liveHeaders: []*types.Header{makeHeader(5), makeHeader(6)},
	}
	updates := runUntilBlock(t, watcher.New(client, common.Address{}), 5, 6)

	if len(updates) != 2 {
		t.Fatalf("expected 2 updates (blocks 5 and 6), got %d: %v", len(updates), blockNumbers(updates))
	}
	if updates[0].BlockNumber != 5 || updates[1].BlockNumber != 6 {
		t.Errorf("unexpected block numbers: %v", blockNumbers(updates))
	}
}

func TestLiveGapFilling(t *testing.T) {
	// History: block 1. Subscription jumps to block 4 (blocks 2 and 3 missing).
	client := &mockClient{
		currentHead: 1,
		liveHeaders: []*types.Header{makeHeader(4)},
	}
	updates := runUntilBlock(t, watcher.New(client, common.Address{}), 1, 4)

	if len(updates) < 4 {
		t.Fatalf("expected 4 updates (blocks 1-4), got %d: %v", len(updates), blockNumbers(updates))
	}
	for i, u := range updates[:4] {
		if want := uint64(1 + i); u.BlockNumber != want {
			t.Errorf("updates[%d].BlockNumber = %d, want %d", i, u.BlockNumber, want)
		}
	}
}
