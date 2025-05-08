use std::{path::PathBuf, sync::Arc};

use anyhow::{bail, Result};
use rand::Rng;
use sqlite::State;
use tasm_lib::twenty_first;

pub mod router;

#[derive(Clone)]
pub struct PoolState {
    db: Arc<sqlite::ConnectionThreadSafe>,
}

impl PoolState {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        #[cfg(debug_assertions)]
        let path = ":memory:";

        #[cfg(not(debug_assertions))]
        let path = {
            if !path.exists() {
                std::fs::create_dir_all(&path)?;
            }
            path.join("transactions.db")
        };

        let db = sqlite::Connection::open_thread_safe(path)?;
        let db = Arc::new(db);
        let s = Self { db };
        s.create_db()?;
        Ok(s)
    }

    fn create_db(&self) -> Result<(), sqlite::Error> {
        self.db.execute(
            "CREATE TABLE IF NOT EXISTS transactions (
                id TEXT PRIMARY KEY,
                rawtx BLOB NOT NULL,
                fee BIGINT NOT NULL,
                height INTEGER DEFAULT 0,
                queue_time INTEGER DEFAULT 0,
                finished_at INTEGER DEFAULT 0,
                revoke_key TEXT NOT NULL,
            )",
        )?;
        self.db
            .execute("CREATE INDEX IF NOT EXISTS idx_transactions_fee ON transactions (fee)")?;

        Ok(())
    }

    pub fn add_transaction(&self, id: &str, transaction: &[u8], fee: i128) -> Result<String> {
        // generate a random key
        let mut rng = rand::rng();
        let mut revoke_key = vec![];
        for _ in 0..32 {
            revoke_key.push(rng.random_range('a'..='z'));
        }
        let revoke_key = String::from_iter(revoke_key);

        let mut stmt = self
            .db
            .prepare("INSERT INTO transactions (id,rawtx,fee) VALUES (?,?,?)")?;
        stmt.bind((1, id))?;
        stmt.bind((2, transaction))?;

        if fee < 400000000000000000000000000000 {
            bail!("fee is too low")
        }

        let fee = fee_to_i64(fee);
        stmt.bind((3, fee))?;
        stmt.next()?;
        Ok(revoke_key)
    }

    pub fn get_most_worth_transaction(&self) -> Result<Option<Vec<u8>>, sqlite::Error> {
        let mut stmt = self
            .db
            .prepare("SELECT * FROM transactions ORDER BY FEE DESC LIMIT 1")?;

        while let Ok(State::Row) = stmt.next() {
            let raw_tx = stmt.read::<Vec<u8>, _>("rawtx").unwrap();
            let id = stmt.read::<String, _>("id").unwrap();
            let fee = stmt.read::<i64, _>("fee").unwrap();
            let mut stmt = self
                .db
                .prepare("INSERT INTO executing (id,rawtx,fee) VALUES (?,?,?)")?;
            stmt.bind((1, id.as_str()))?;
            stmt.bind((2, raw_tx.as_slice()))?;
            stmt.bind((3, fee))?;
            stmt.next()?;

            let mut stmt = self.db.prepare("DELETE FROM transactions WHERE id=?")?;
            stmt.bind((1, id.as_str()))?;
            stmt.next()?;

            return Ok(Some(raw_tx));
        }

        Ok(None)
    }

    pub fn get_executing_transaction(&self, id: &str) -> Result<Option<(Vec<u8>, u64, u64)>> {
        let mut stmt = self.db.prepare("SELECT * FROM executing WHERE id=?")?;
        stmt.bind((1, id))?;
        while let Ok(State::Row) = stmt.next() {
            let raw_tx = stmt.read::<Vec<u8>, _>("rawtx").unwrap();
            let created_at = stmt.read::<i64, _>("created_at").unwrap();
            let finished_at = stmt.read::<i64, _>("finished_at").unwrap();
            return Ok(Some((raw_tx, created_at as u64, finished_at as u64)));
        }

        Ok(None)
    }

    pub fn get_pending_transaction(&self, id: &str) -> Result<Option<String>> {
        let mut stmt = self.db.prepare("SELECT * FROM transactions WHERE id=?")?;
        stmt.bind((1, id))?;
        while let Ok(State::Row) = stmt.next() {
            let txid = stmt.read::<String, _>("id").unwrap();
            return Ok(Some(txid));
        }

        Ok(None)
    }

    pub fn drop_old_transaction(&self) -> Result<()> {
        let mut stmt = self.db.prepare("DELETE transactions")?;
        stmt.next()?;
        Ok(())
    }

    pub fn finish_transaction(&self, id: &str) -> anyhow::Result<()> {
        let mut stmt = self
            .db
            .prepare("UPDATE executing SET finished_at=strftime('%s', 'now') WHERE id=?")?;
        stmt.bind((1, id))?;
        stmt.next()?;
        Ok(())
    }
}

// 转为小数点10位
fn fee_to_i64(fee: i128) -> i64 {
    let fee = fee >> 2;
    let fee = fee / 10i128.pow(20);
    return fee as i64;
}

#[cfg(test)]
mod tests {

    use crate::models::blockchain::type_scripts::native_currency_amount::NativeCurrencyAmount;

    use super::*;
    #[test]
    fn test_fee_to_i64() {
        let fee = NativeCurrencyAmount::coins_from_str("0.1").unwrap();
        let fee = fee.to_nau();
        println!("{} {}", fee, fee >> 2);
        let fee = fee_to_i64(fee);
        println!("{}", fee);
    }

    #[test]
    fn test_tx_insert() {
        let state = PoolState::new(PathBuf::new()).unwrap();
        let tx = vec![1, 2, 3];
        state
            .add_transaction("1", &tx, 100000000000000000000000000000 << 2)
            .unwrap();
        state
            .add_transaction("2", &tx, 200000000000000000000000000000 << 2)
            .unwrap();
        state
            .add_transaction("3", &tx, 300000000000000000000000000000 << 2)
            .unwrap();
        state
            .add_transaction("4", &tx, 400000000000000000000000000000 << 2)
            .unwrap();
        state
            .add_transaction("5", &vec![3, 2, 3], 500000000000000000000000000000 << 2)
            .unwrap();

        let time = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let tx = state.get_most_worth_transaction().unwrap().unwrap();
        assert_eq!(tx, vec![3, 2, 3]);

        let status = state.get_executing_transaction("5").unwrap().unwrap();
        assert!(status.1 >= time);
        assert!(status.1 < time + 10);
    }
}
