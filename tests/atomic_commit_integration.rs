#![forbid(unsafe_code)]

use aurion::codec::CanonicalEncode;
use aurion::consensus::bft::{
    BftEngine, Block, CommitCertificate, ValidatorSet, Vote, PHASE_PRECOMMIT,
};
use aurion::consensus::header::BlockHeader;
use aurion::core::{Address, Hash256};
use aurion::genesis::ceremony::{CanonicalCeremonyKeypairs, CeremonyTranscript};
use aurion::state::account::Account;
use aurion::state::chain::{ChainError, ChainLedger};
use aurion::state::monetary::calculate_block_subsidy;
use aurion::state::smt::compute_accounts_state_root;
use aurion::storage::{RedbStorageEngine, StateStore, StorageError};
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::NamedTempFile;

fn certificate_and_block(
    genesis: &aurion::genesis::builder::GenesisInitialization,
    keys: &CanonicalCeremonyKeypairs,
) -> (Block, CommitCertificate, ValidatorSet, Address) {
    let validator_set = genesis.validator_set.clone();
    let previous = Block::new(genesis.header.clone(), Vec::new(), None);
    let proposer = BftEngine::select_proposer(&validator_set, 1, 0, &previous.hash());
    let miner = validator_set.get_validator(proposer).unwrap().validator_id;
    let mut accounts = genesis.accounts.clone();
    let subsidy = calculate_block_subsidy(1);
    accounts.entry(miner).or_default().balance = accounts
        .get(&miner)
        .map(|account| account.balance)
        .unwrap_or_default()
        .checked_add(subsidy)
        .unwrap();
    let block = Block::new(
        BlockHeader {
            version: 1,
            height: 1,
            round: 0,
            timestamp: previous.header.timestamp + 1,
            prev_block_hash: previous.hash(),
            tx_merkle_root: Hash256::ZERO,
            state_root: compute_accounts_state_root(&accounts),
        },
        Vec::new(),
        None,
    );
    let precommits = (0..3)
        .map(|index| {
            Vote::new_signed(
                &keys.validators[index],
                PHASE_PRECOMMIT,
                1,
                0,
                block.hash(),
                index as u32,
            )
            .unwrap()
        })
        .collect();
    let certificate = CommitCertificate {
        block_hash: block.hash(),
        height: 1,
        round: 0,
        precommits,
    };
    (block, certificate, validator_set, miner)
}

#[test]
fn test_atomic_commit_includes_certificate_and_metadata() {
    let path = NamedTempFile::new().unwrap();
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let genesis = CeremonyTranscript::canonical_mainnet_genesis();
    let (mut block, certificate, validator_set, miner) = certificate_and_block(&genesis, &keys);
    certificate.verify(&validator_set).unwrap();
    block.commit_certificate = Some(certificate.clone());

    let store = Arc::new(RedbStorageEngine::open_or_create(path.path()).unwrap());
    let mut ledger = ChainLedger::from_genesis_with_store(genesis, store.clone()).unwrap();
    ledger.apply_block(block.clone(), &miner).unwrap();

    assert_eq!(
        store.get_block_by_height(1).unwrap().unwrap().hash(),
        block.hash()
    );
    assert_eq!(
        store
            .get_block_by_hash(&block.hash())
            .unwrap()
            .unwrap()
            .hash(),
        block.hash()
    );
    assert_eq!(store.get_certificate(1).unwrap().unwrap(), certificate);
    assert_eq!(store.get_latest_height().unwrap(), Some(1));
    assert_eq!(
        store.get_metadata("latest_block_hash").unwrap(),
        Some(block.hash().as_bytes().to_vec())
    );
    assert_eq!(
        store.get_metadata("latest_state_root").unwrap(),
        Some(block.header.state_root.as_bytes().to_vec())
    );
}

#[test]
fn test_crash_recovery_reconstructs_certificate_and_state_root() {
    let path = NamedTempFile::new().unwrap();
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let genesis = CeremonyTranscript::canonical_mainnet_genesis();
    let (mut block, certificate, validator_set, miner) = certificate_and_block(&genesis, &keys);
    block.commit_certificate = Some(certificate.clone());
    let block_hash = block.hash();
    let state_root = block.header.state_root;
    {
        let store = Arc::new(RedbStorageEngine::open_or_create(path.path()).unwrap());
        let mut ledger = ChainLedger::from_genesis_with_store(genesis.clone(), store).unwrap();
        ledger.apply_block(block, &miner).unwrap();
    }
    let store = Arc::new(RedbStorageEngine::open_or_create(path.path()).unwrap());
    let recovered = ChainLedger::from_genesis_with_store(genesis, store.clone()).unwrap();
    let recovered_block = recovered.latest_block();
    let recovered_certificate = store.get_certificate(1).unwrap().unwrap();
    recovered_certificate.verify(&validator_set).unwrap();
    assert_eq!(recovered_block.hash(), block_hash);
    assert_eq!(recovered.compute_current_state_root(), state_root);
    assert_eq!(recovered_certificate.to_canonical_bytes().len(), 403);
}

struct FailingStore;

impl StateStore for FailingStore {
    fn get_account(&self, _: &Address) -> Result<Option<Account>, StorageError> {
        Ok(None)
    }
    fn get_all_accounts(&self) -> Result<HashMap<Address, Account>, StorageError> {
        Ok(HashMap::new())
    }
    fn get_block_by_height(&self, _: u64) -> Result<Option<Block>, StorageError> {
        Ok(None)
    }
    fn get_block_by_hash(&self, _: &Hash256) -> Result<Option<Block>, StorageError> {
        Ok(None)
    }
    fn get_certificate(&self, _: u64) -> Result<Option<CommitCertificate>, StorageError> {
        Ok(None)
    }
    fn get_latest_height(&self) -> Result<Option<u64>, StorageError> {
        Ok(None)
    }
    fn get_metadata(&self, _: &str) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(None)
    }
    fn commit_block_atomic(
        &self,
        _: &Block,
        _: &CommitCertificate,
        _: &[(Address, Account)],
    ) -> Result<(), StorageError> {
        Err(StorageError::Database(
            "simulated pre-commit I/O failure".into(),
        ))
    }
}

#[test]
fn test_atomic_rollback_on_simulated_failure() {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let genesis = CeremonyTranscript::canonical_mainnet_genesis();
    let (mut block, certificate, _, miner) = certificate_and_block(&genesis, &keys);
    block.commit_certificate = Some(certificate);
    let mut ledger = ChainLedger::from_genesis(genesis);
    ledger.store = Some(Arc::new(FailingStore));
    assert!(matches!(
        ledger.apply_block(block, &miner),
        Err(ChainError::Storage(_))
    ));
    assert_eq!(ledger.latest_height(), 0);
    assert_eq!(ledger.blocks.len(), 1);
}
