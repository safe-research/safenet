package storage

import (
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"

	_ "modernc.org/sqlite" // register the "sqlite" driver

	"github.com/safe-research/safenet/validator-go/state"
)

const schema = `
CREATE TABLE IF NOT EXISTS validator_state (
    block_number INTEGER NOT NULL PRIMARY KEY,
    state        TEXT    NOT NULL
);`

// Storage persists validator state snapshots in a SQLite database, retaining
// at most stateHistory entries. Zero means retain all.
type Storage struct {
	db           *sql.DB
	stateHistory uint
}

// Open opens (or creates) a SQLite database at path. An empty path uses an
// in-memory database. stateHistory controls how many snapshots are retained;
// zero keeps all snapshots.
func Open(path string, stateHistory uint) (*Storage, error) {
	dsn := path
	if path == "" {
		dsn = ":memory:"
	}
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, fmt.Errorf("open database: %w", err)
	}
	if _, err := db.Exec(schema); err != nil {
		db.Close()
		return nil, fmt.Errorf("create schema: %w", err)
	}
	return &Storage{db: db, stateHistory: stateHistory}, nil
}

// Save persists st at the given blockNumber. If stateHistory > 0, all but the
// most recent stateHistory rows are pruned within the same transaction.
func (s *Storage) Save(blockNumber uint64, st *state.State) error {
	data, err := json.Marshal(st)
	if err != nil {
		return fmt.Errorf("marshal state: %w", err)
	}

	tx, err := s.db.Begin()
	if err != nil {
		return fmt.Errorf("begin transaction: %w", err)
	}
	defer tx.Rollback() //nolint:errcheck

	if _, err := tx.Exec(
		"INSERT OR REPLACE INTO validator_state (block_number, state) VALUES (?, ?)",
		blockNumber, string(data),
	); err != nil {
		return fmt.Errorf("insert state: %w", err)
	}

	if s.stateHistory > 0 {
		if _, err := tx.Exec(`
			DELETE FROM validator_state
			WHERE block_number NOT IN (
				SELECT block_number FROM validator_state
				ORDER BY block_number DESC
				LIMIT ?
			)`, s.stateHistory,
		); err != nil {
			return fmt.Errorf("prune state: %w", err)
		}
	}

	return tx.Commit()
}

// LoadLatest returns the block number and state of the most recently saved
// snapshot. Returns (0, nil, nil) when the database is empty.
func (s *Storage) LoadLatest() (uint64, *state.State, error) {
	var blockNumber uint64
	var raw string

	err := s.db.QueryRow(
		"SELECT block_number, state FROM validator_state ORDER BY block_number DESC LIMIT 1",
	).Scan(&blockNumber, &raw)
	if errors.Is(err, sql.ErrNoRows) {
		return 0, nil, nil
	}
	if err != nil {
		return 0, nil, fmt.Errorf("query state: %w", err)
	}

	var st state.State
	if err := json.Unmarshal([]byte(raw), &st); err != nil {
		return 0, nil, fmt.Errorf("unmarshal state at block %d: %w", blockNumber, err)
	}
	return blockNumber, &st, nil
}

// Close releases all resources held by the storage.
func (s *Storage) Close() error {
	return s.db.Close()
}
