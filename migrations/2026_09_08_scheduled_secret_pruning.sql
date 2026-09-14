-- Temporary, one-time manual migration: scheduled secret pruning.
--
-- Applies ONLY to a validator database created before scheduled secret pruning
-- was introduced, i.e. one whose `keygen_secrets` and `nonces_chunks` tables
-- have no `delete_after` column and which has no `group_secret_reconciliation`
-- table. It exists for the long-running dev network, whose database predates
-- the change and holds secrets worth keeping. A recreated database already gets
-- the new schema from `SecretStore::new`
-- (crates/validator/src/secrets/store.rs), and the validator never discovers or
-- runs this file.
--
-- Check whether it has already been applied:
--
--     sqlite3 <validator-database> \
--         "SELECT COUNT(*) FROM pragma_table_info('keygen_secrets')
--          WHERE name = 'delete_after';"
--
-- 1 means it has, 0 means it has not. SQLite has no
-- `ALTER TABLE ... ADD COLUMN IF NOT EXISTS`, so applying this twice cannot be
-- made a silent no-op. `.bail on` and the surrounding transaction make the
-- second attempt stop on the first `duplicate column name` error with nothing
-- committed, rather than half-applying.
--
-- Stop the validator, then apply it to the database file directly:
--
--     sqlite3 <validator-database> < migrations/2026_09_08_scheduled_secret_pruning.sql
--
-- Existing secret rows are preserved, their deletion deadlines start as NULL
-- (nothing scheduled for deletion), and the reconciliation marker starts empty
-- (no reconciliation accepted yet), so the updated validator schedules and
-- collects from its next accepted reconciliation onwards.

.bail on

BEGIN;

ALTER TABLE keygen_secrets ADD COLUMN delete_after INTEGER;
ALTER TABLE nonces_chunks ADD COLUMN delete_after INTEGER;

CREATE TABLE IF NOT EXISTS group_secret_reconciliation (
    id    INTEGER PRIMARY KEY CHECK (id = 0),
    block INTEGER NOT NULL
);

COMMIT;
