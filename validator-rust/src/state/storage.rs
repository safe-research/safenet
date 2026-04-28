use std::{num::NonZeroU64, path::Path};

use anyhow::{Context as _, Result};
use rusqlite::{Connection, params};

use super::ValidatorState;

const DEFAULT_STATE_HISTORY: NonZeroU64 = NonZeroU64::new(5).unwrap();
const DEFAULT_STORAGE_FILE: &str = ":memory:";

pub struct Storage {
    conn: Connection,
    history: NonZeroU64,
}

impl Storage {
    pub fn open(path: Option<&Path>, history: Option<NonZeroU64>) -> Result<Self> {
        let conn = Connection::open(path.unwrap_or(Path::new(DEFAULT_STORAGE_FILE)))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS validator_state (
                block_number INTEGER PRIMARY KEY,
                state_json   TEXT NOT NULL
            );",
        )?;
        Ok(Self {
            conn,
            history: history.unwrap_or(DEFAULT_STATE_HISTORY),
        })
    }

    pub fn save(&self, block_number: u64, state: &ValidatorState) -> Result<()> {
        let json = serde_json::to_string(state)?;
        self.conn.execute(
            "INSERT INTO validator_state (block_number, state_json) VALUES (?1, ?2)",
            params![i64::try_from(block_number)?, json],
        )?;
        if let Some(cleanup) = block_number.checked_sub(self.history.get()) {
            self.conn.execute(
                "DELETE FROM validator_state WHERE block_number <= ?1",
                params![i64::try_from(cleanup)?],
            )?;
        }
        Ok(())
    }

    pub fn load_latest(&self) -> Result<Option<ValidatorState>> {
        let result = self.conn.query_row(
            "SELECT state_json FROM validator_state ORDER BY block_number DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(json) => Ok(Some(
                serde_json::from_str(&json).context("failed to deserialize validator state")?,
            )),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
