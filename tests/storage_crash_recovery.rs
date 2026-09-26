#![forbid(unsafe_code)]

use redb::{Database, ReadableDatabase, TableDefinition};
use tempfile::NamedTempFile;

const BLOCKS_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("blocks");
const ACCOUNTS_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("accounts");
const TX_INDEX_TABLE: TableDefinition<&[u8], u64> = TableDefinition::new("tx_index");

#[test]
fn test_redb_write_transaction_abort_leaves_prior_state_intact() {
    let temp = NamedTempFile::new().expect("failed to create temp db");
    let db_path = temp.path().to_path_buf();

    {
        let db = Database::create(&db_path).expect("failed to create redb database");

        {
            let write_txn = db.begin_write().expect("begin_write for baseline state");
            {
                let mut blocks = write_txn
                    .open_table(BLOCKS_TABLE)
                    .expect("open blocks table");
                blocks
                    .insert(0, b"genesis".as_slice())
                    .expect("insert genesis block");
            }
            {
                let mut accounts = write_txn
                    .open_table(ACCOUNTS_TABLE)
                    .expect("open accounts table");
                accounts
                    .insert(b"alice".as_slice(), b"1000".as_slice())
                    .expect("insert baseline account");
            }
            {
                let _tx_index = write_txn
                    .open_table(TX_INDEX_TABLE)
                    .expect("open tx_index table");
            }
            write_txn.commit().expect("commit baseline state");
        }

        {
            let write_txn = db.begin_write().expect("begin_write for aborted mutation");
            {
                let mut blocks = write_txn
                    .open_table(BLOCKS_TABLE)
                    .expect("open blocks table");
                blocks
                    .insert(1, b"post-crash".as_slice())
                    .expect("insert next block in aborted txn");
            }
            {
                let mut accounts = write_txn
                    .open_table(ACCOUNTS_TABLE)
                    .expect("open accounts table");
                accounts
                    .insert(b"alice".as_slice(), b"1500".as_slice())
                    .expect("mutate account in aborted txn");
            }
            {
                let mut tx_index = write_txn
                    .open_table(TX_INDEX_TABLE)
                    .expect("open tx_index table");
                tx_index
                    .insert(b"tx-1".as_slice(), 1)
                    .expect("insert tx index in aborted txn");
            }

            drop(write_txn);
        }
    }

    let reopened = Database::open(&db_path).expect("reopen database after aborted write");
    let read_txn = reopened.begin_read().expect("begin read after abort");

    let blocks = read_txn
        .open_table(BLOCKS_TABLE)
        .expect("open blocks for verification");
    assert_eq!(
        blocks.get(0).unwrap().map(|v| v.value().to_vec()),
        Some(b"genesis".to_vec())
    );
    assert!(
        blocks.get(1).unwrap().is_none(),
        "aborted block write leaked into persisted state"
    );

    let accounts = read_txn
        .open_table(ACCOUNTS_TABLE)
        .expect("open accounts for verification");
    assert_eq!(
        accounts
            .get(b"alice".as_slice())
            .unwrap()
            .map(|v| v.value().to_vec()),
        Some(b"1000".to_vec()),
        "aborted account mutation leaked into persisted state"
    );

    let tx_index = read_txn
        .open_table(TX_INDEX_TABLE)
        .expect("open tx_index for verification");
    assert!(
        tx_index.get(b"tx-1".as_slice()).unwrap().is_none(),
        "aborted tx index leak into persisted state"
    );
}
