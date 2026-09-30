-- Temporary, one-time manual migration: 7702 executor transaction batching.
--
-- Applies ONLY to a database created before the transaction queue moved the
-- state of allocated nonces into an `allocated_nonces` table, i.e. one whose
-- `transactions` table still has `submitted_at` and `executed_at` columns. It
-- exists for the long-running test network, whose database predates the
-- change and holds transactions still in flight. A recreated database already
-- gets the new schema from `TransactionStorage::new`
-- (crates/core/src/tx/storage.rs), and no service discovers or runs this file.
--
-- Check whether it has already been applied:
--
--     sqlite3 <database> \
--         "SELECT COUNT(*) FROM pragma_table_info('transactions')
--          WHERE name = 'submitted_at';"
--
-- 0 means it has, 1 means it has not. Do not check for the `allocated_nonces`
-- table instead: a service started before the migration creates it (empty)
-- itself, which is why it is created with `IF NOT EXISTS` below. `.bail on` and
-- the surrounding transaction make a second attempt stop on the first
-- `no such column: submitted_at` error with nothing committed, rather than
-- half-applying.
--
-- Stop the service, then apply it to the database file directly:
--
--     sqlite3 <database> < migrations/2026_09_23_safenet_7702_executor_tx_batching.sql
--
-- Every allocated transaction keeps its nonce, the fees of its last submission
-- and its submission and execution blocks, now on its `allocated_nonces` row.
-- Queued transactions are preserved as they are.

.bail on

BEGIN;

CREATE TABLE IF NOT EXISTS allocated_nonces (
    nonce        INTEGER PRIMARY KEY,
    request      TEXT    DEFAULT NULL,
    delegate     TEXT    DEFAULT NULL,
    submitted_at INTEGER DEFAULT NULL,
    executed_at  INTEGER DEFAULT NULL,
    CHECK (request IS NOT NULL OR delegate IS NOT NULL)
);

-- The nonce's `request` is the transaction sent at it, with the fees of its
-- last submission, exactly as the old row stored it.
INSERT INTO allocated_nonces (nonce, request, submitted_at, executed_at)
SELECT nonce, request, submitted_at, executed_at
FROM transactions
WHERE nonce IS NOT NULL;

CREATE TABLE transactions_new (
    id         INTEGER PRIMARY KEY,
    request    TEXT    NOT NULL,
    expires_at INTEGER DEFAULT NULL,
    nonce      INTEGER DEFAULT NULL REFERENCES allocated_nonces (nonce)
);

-- The queued transaction's `request` is the transaction as it was enqueued,
-- without fees.
INSERT INTO transactions_new (id, request, expires_at, nonce)
SELECT id, json_remove(request, '$.maxFeePerGas', '$.maxPriorityFeePerGas'), expires_at, nonce
FROM transactions;

DROP TABLE transactions;
ALTER TABLE transactions_new RENAME TO transactions;

COMMIT;
