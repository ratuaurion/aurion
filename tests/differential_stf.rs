#![forbid(unsafe_code)]

//! VER-003: Differential Testing Engine untuk State Transition Function (STF) Aurion
//! Menguji implementasi kanonikal Aurion (ChainLedger, apply_transaction, SMT state root, emisi mining)
//! melawan Model Referensi Independen (ReferenceSTF) menggunakan generator transaksi acak,
//! edge-case boundary, dan injeksi adversarial.

use aurion::consensus::bft::engine::BftEngine;
use aurion::consensus::block::Block;
use aurion::consensus::certificate::{ValidatorEntry, ValidatorSet};
use aurion::consensus::vote::{Vote, PHASE_PRECOMMIT, PHASE_PREVOTE};
use aurion::core::{Address, Quantum, Signature, BLOCK_REWARD_QUANTA};
use aurion::crypto::{blake3_hash, derive_address_from_pubkey, Keypair};
use aurion::genesis::builder::build_genesis;
use aurion::mempool::MempoolEngine;
use aurion::state::chain::ChainLedger;
use aurion::transaction::types::{Transaction, TxType};
use std::collections::BTreeMap;

// ==============================================================================
// 1. DETERMINISTIC BLAKE3 PRNG ENGINE (Zero Float, Zero External Crate)
// ==============================================================================

struct Blake3Prng {
    state: [u8; 32],
    counter: u64,
}

impl Blake3Prng {
    fn new(seed: &[u8]) -> Self {
        let digest = blake3_hash(seed);
        Self {
            state: digest.0,
            counter: 0,
        }
    }

    fn next_bytes(&mut self) -> [u8; 32] {
        self.counter += 1;
        let mut input = Vec::with_capacity(40);
        input.extend_from_slice(&self.state);
        input.extend_from_slice(&self.counter.to_be_bytes());
        let digest = blake3_hash(&input);
        self.state = digest.0;
        digest.0
    }

    fn next_u64(&mut self) -> u64 {
        let bytes = self.next_bytes();
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&bytes[..8]);
        u64::from_be_bytes(buf)
    }

    fn next_range(&mut self, min: u64, max: u64) -> u64 {
        if min >= max {
            return min;
        }
        min + (self.next_u64() % (max - min + 1))
    }
}

// ==============================================================================
// 2. INDEPENDENT REFERENCE STATE MACHINE (Model Pembanding Terisolasi)
// ==============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReferenceAccount {
    balance: u128,
    nonce: u64,
}

struct ReferenceSTF {
    accounts: BTreeMap<Address, ReferenceAccount>,
    total_issued: u128,
    total_burned: u128,
}

impl ReferenceSTF {
    fn new_genesis(creator: Address, dev: Address) -> Self {
        let mut accounts = BTreeMap::new();
        // Alokasi Genesis Kanonikal: 100% Pasokan (66M AUR pada skala 10^9) ke Master Treasury
        let creator_quanta: u128 = 66_000_000 * 1_000_000_000;
        let dev_quanta: u128 = 0;

        accounts.insert(
            creator,
            ReferenceAccount {
                balance: creator_quanta,
                nonce: 0,
            },
        );
        if dev != creator {
            accounts.insert(
                dev,
                ReferenceAccount {
                    balance: dev_quanta,
                    nonce: 0,
                },
            );
        }

        Self {
            accounts,
            total_issued: creator_quanta,
            total_burned: 0,
        }
    }

    fn apply_block_subsidy(&mut self, height: u64, miner: &Address) {
        if height == 0 {
            return;
        }
        let subsidy = BLOCK_REWARD_QUANTA;
        self.total_issued += subsidy;

        let miner_acct = self.accounts.entry(*miner).or_insert(ReferenceAccount {
            balance: 0,
            nonce: 0,
        });
        miner_acct.balance += subsidy;
    }

    fn apply_tx(&mut self, miner: &Address, tx: &Transaction) -> Result<(), String> {
        let sender_acct = self
            .accounts
            .get_mut(&tx.sender)
            .ok_or_else(|| "Sender not found in reference state".to_string())?;

        if tx.nonce != sender_acct.nonce {
            return Err(format!(
                "Nonce mismatch: expected {}, got {}",
                sender_acct.nonce, tx.nonce
            ));
        }

        let amount_u128 = tx.amount.as_u128();
        let fee_u128 = tx.fee.as_u128();

        let total_cost = amount_u128
            .checked_add(fee_u128)
            .ok_or_else(|| "Cost overflow".to_string())?;

        if sender_acct.balance < total_cost {
            return Err("Insufficient balance".to_string());
        }

        // Debet saldo pengirim & tingkatkan nonce
        sender_acct.balance -= total_cost;
        sender_acct.nonce += 1;

        // Alokasi fee transaksi: 100% dialokasikan ke validator pembuat blok (0% burn)
        let validator_reward_quanta = fee_u128;

        // Kredit miner
        let miner_acct = self.accounts.entry(*miner).or_insert(ReferenceAccount {
            balance: 0,
            nonce: 0,
        });
        miner_acct.balance += validator_reward_quanta;

        // Kredit recipient
        let recipient_acct = self
            .accounts
            .entry(tx.recipient)
            .or_insert(ReferenceAccount {
                balance: 0,
                nonce: 0,
            });
        recipient_acct.balance += amount_u128;

        Ok(())
    }

    fn circulating_supply(&self) -> u128 {
        self.total_issued - self.total_burned
    }

    fn get_balance(&self, addr: &Address) -> u128 {
        self.accounts.get(addr).map(|a| a.balance).unwrap_or(0)
    }

    fn get_nonce(&self, addr: &Address) -> u64 {
        self.accounts.get(addr).map(|a| a.nonce).unwrap_or(0)
    }
}

// ==============================================================================
// 3. HELPER PEMBENTUKAN COMMIT CERTIFICATE BFT KUORUM > 2/3
// ==============================================================================

fn produce_test_block(
    bft: &BftEngine,
    ledger: &mut ChainLedger,
    mempool: &MempoolEngine,
    val_keys: &[Keypair],
    val_set: &ValidatorSet,
    miner: &Address,
    timestamp: u64,
) -> Block {
    let candidate_block = bft.assemble_block_proposal(
        ledger,
        mempool,
        0,
        timestamp,
        miner,
        1024 * 1024,
    );

    let block_hash = candidate_block.hash();
    let height = candidate_block.height();

    let mut precommits = Vec::new();
    for (idx, key) in val_keys.iter().enumerate().take(3) {
        let prevote = Vote::new_signed(key, PHASE_PREVOTE, height, 0, block_hash, idx as u32).unwrap();
        prevote.verify(val_set).unwrap();
        let precommit = Vote::new_signed(key, PHASE_PRECOMMIT, height, 0, block_hash, idx as u32).unwrap();
        precommit.verify(val_set).unwrap();
        precommits.push(precommit);
    }

    let cert = bft
        .create_commit_certificate(val_set, block_hash, height, 0, precommits)
        .unwrap();

    let block = Block::new(candidate_block.header, candidate_block.transactions, Some(cert));
    ledger.apply_block(block.clone(), miner).expect("Block apply failed");
    block
}

// ==============================================================================
// 4. TEST 1: MASSIVE DIFFERENTIAL STF SIMULATION (Multi-Block Multi-User)
// ==============================================================================

#[test]
fn test_differential_stf_massive_multi_block_simulation() {
    let mut prng = Blake3Prng::new(b"AURION-DIFFERENTIAL-TEST-SEED-V1");

    // Setup 4 Validator BFT
    let val_keys: Vec<Keypair> = (0..4).map(|_| Keypair::generate()).collect();
    let val_entries: Vec<ValidatorEntry> = val_keys
        .iter()
        .map(|kp| {
            let addr = derive_address_from_pubkey(&kp.public_key_bytes());
            ValidatorEntry {
                validator_id: addr,
                consensus_pubkey: kp.public_key_bytes(),
                voting_weight: 25,
            }
        })
        .collect();

    let creator_key = Keypair::generate();
    let creator_addr = derive_address_from_pubkey(&creator_key.public_key_bytes());

    let dev_key = Keypair::generate();
    let dev_addr = derive_address_from_pubkey(&dev_key.public_key_bytes());

    let genesis = build_genesis(creator_addr, dev_addr, val_entries.clone());
    let validator_set = ValidatorSet::new(val_entries);
    let mut ledger = ChainLedger::from_genesis(genesis);

    let mut ref_stf = ReferenceSTF::new_genesis(creator_addr, dev_addr);

    let bft = BftEngine::new(Some(val_keys[0].clone()), Some(0));

    // Setup 8 Akun Pengguna Tambahan
    let users: Vec<Keypair> = (0..8).map(|_| Keypair::generate()).collect();
    let user_addrs: Vec<Address> = users
        .iter()
        .map(|k| derive_address_from_pubkey(&k.public_key_bytes()))
        .collect();

    // Miner address
    let miner_key = Keypair::generate();
    let miner_addr = derive_address_from_pubkey(&miner_key.public_key_bytes());

    let mut current_time: u64 = 1773532800;

    // --- FASE 1: Distribusi Dana Awal dari Creator ke 8 Pengguna (Blok 1) ---
    {
        current_time += 60;
        let mut mempool = MempoolEngine::new(1024 * 1024, 3600);

        for (idx, user_addr) in user_addrs.iter().enumerate() {
            let amount = Quantum::from_aur(10_000).unwrap();
            let fee = Quantum::from_aur(1).unwrap();

            let mut tx = Transaction {
                version: 1,
                chain_id: 1001,
                tx_type: TxType::Transfer,
                flags: 0,
                sender: creator_addr,
                recipient: *user_addr,
                nonce: idx as u64,
                amount,
                fee,
                valid_until: 1800000000,
                payload: Vec::new(),
                signature: Signature::from_bytes([0u8; 64]),
            };
            tx.signature = creator_key.sign(&tx.signing_preimage());

            mempool
                .submit_transaction(
                    tx.clone(),
                    &creator_key.public_key_bytes(),
                    current_time,
                    ledger.get_account(&creator_addr).unwrap(),
                )
                .expect("Seed submit failed");
        }

        // Eksekusi Blok 1 di Canonical Ledger
        let block1 = produce_test_block(
            &bft,
            &mut ledger,
            &mempool,
            &val_keys,
            &validator_set,
            &miner_addr,
            current_time,
        );

        // Terapkan transaksi yang benar-benar masuk blok kanonikal ke reference model
        for tx in &block1.transactions {
            ref_stf.apply_tx(&miner_addr, tx).expect("Ref seed apply failed");
        }

        // Terapkan subsidi blok 1 ke reference
        ref_stf.apply_block_subsidy(1, &miner_addr);

        // Asersi diferensial Blok 1
        assert_eq!(
            ledger.get_balance(&creator_addr).as_u128(),
            ref_stf.get_balance(&creator_addr)
        );
        assert_eq!(
            ledger.get_balance(&miner_addr).as_u128(),
            ref_stf.get_balance(&miner_addr)
        );
        for user_addr in &user_addrs {
            assert_eq!(
                ledger.get_balance(user_addr).as_u128(),
                ref_stf.get_balance(user_addr)
            );
        }
    }

    // --- FASE 2: Simulasi 30 Blok dengan Ratusan Transaksi Pseudo-Random Acak ---
    let mut total_tx_simulated = 8;

    for height in 2..=30 {
        current_time += 60;
        let mut mempool = MempoolEngine::new(1024 * 1024, 3600);

        // Hasilkan antara 5 s/d 15 transaksi per blok
        let tx_count = prng.next_range(5, 15);
        let mut pending_nonces: BTreeMap<Address, u64> = BTreeMap::new();

        for _ in 0..tx_count {
            let sender_idx = prng.next_range(0, users.len() as u64 - 1) as usize;
            let recipient_idx = prng.next_range(0, users.len() as u64 - 1) as usize;
            if sender_idx == recipient_idx {
                continue;
            }

            let sender_key = &users[sender_idx];
            let sender_addr = user_addrs[sender_idx];
            let recipient_addr = user_addrs[recipient_idx];

            let sender_bal = ref_stf.get_balance(&sender_addr);
            if sender_bal < 100_000_000 {
                continue;
            }

            let max_send = (sender_bal / 4).min(500_000_000);
            let send_quanta = prng.next_range(1, max_send as u64);
            let fee_quanta = prng.next_range(100, 1_000_000);

            let amount = Quantum::new(send_quanta as u128);
            let fee = Quantum::new(fee_quanta as u128);

            let current_nonce = *pending_nonces
                .entry(sender_addr)
                .or_insert_with(|| ledger.get_nonce(&sender_addr));

            let mut tx = Transaction {
                version: 1,
                chain_id: 1001,
                tx_type: TxType::Transfer,
                flags: 0,
                sender: sender_addr,
                recipient: recipient_addr,
                nonce: current_nonce,
                amount,
                fee,
                valid_until: 1800000000,
                payload: Vec::new(),
                signature: Signature::from_bytes([0u8; 64]),
            };
            tx.signature = sender_key.sign(&tx.signing_preimage());

            // Validasi & masukkan ke mempool
            if mempool
                .submit_transaction(
                    tx.clone(),
                    &sender_key.public_key_bytes(),
                    current_time,
                    ledger.get_account(&sender_addr).unwrap(),
                )
                .is_ok()
            {
                pending_nonces.insert(sender_addr, current_nonce + 1);
            }
        }

        // Eksekusi Blok di Canonical Ledger
        let block = produce_test_block(
            &bft,
            &mut ledger,
            &mempool,
            &val_keys,
            &validator_set,
            &miner_addr,
            current_time,
        );

        // Terapkan transaksi yang benar-benar masuk blok kanonikal ke reference model
        for tx in &block.transactions {
            ref_stf
                .apply_tx(&miner_addr, tx)
                .expect("Ref tx execution must succeed for tx in canonical block");
            total_tx_simulated += 1;
        }

        // Terapkan subsidi blok ke reference model
        ref_stf.apply_block_subsidy(height, &miner_addr);

        // Asersi Diferensial Ketat di Setiap Blok:
        assert_eq!(
            ledger.monetary.total_issued.as_u128(),
            ref_stf.total_issued,
            "Total issued mismatch at block {height}"
        );
        assert_eq!(
            ledger.monetary.total_burned.as_u128(),
            ref_stf.total_burned,
            "Total burned mismatch at block {height}"
        );
        assert_eq!(
            ledger.monetary.circulating_supply().unwrap().as_u128(),
            ref_stf.circulating_supply(),
            "Circulating supply mismatch at block {height}"
        );

        assert_eq!(
            ledger.get_balance(&miner_addr).as_u128(),
            ref_stf.get_balance(&miner_addr),
            "Miner balance mismatch at block {height}"
        );

        for (idx, user_addr) in user_addrs.iter().enumerate() {
            assert_eq!(
                ledger.get_balance(user_addr).as_u128(),
                ref_stf.get_balance(user_addr),
                "User {idx} balance mismatch at block {height}"
            );
            assert_eq!(
                ledger.get_nonce(user_addr),
                ref_stf.get_nonce(user_addr),
                "User {idx} nonce mismatch at block {height}"
            );
        }
    }

    assert!(
        total_tx_simulated >= 50,
        "Simulated tx count ({total_tx_simulated}) should be >= 50"
    );

    // Asersi Konservasi Tertinggi [INV-06]: Sum(Balances) == CirculatingSupply
    let mut canonical_sum: u128 = 0;
    canonical_sum += ledger.get_balance(&creator_addr).as_u128();
    canonical_sum += ledger.get_balance(&dev_addr).as_u128();
    canonical_sum += ledger.get_balance(&miner_addr).as_u128();
    for user_addr in &user_addrs {
        canonical_sum += ledger.get_balance(user_addr).as_u128();
    }

    assert_eq!(
        canonical_sum,
        ledger.monetary.circulating_supply().unwrap().as_u128(),
        "Conservation invariant [INV-06] violated!"
    );
}

// ==============================================================================
// 5. TEST 2: ADVERSARIAL REJECTION DIFFERENTIAL TEST
// ==============================================================================

#[test]
fn test_differential_adversarial_rejection_consistency() {
    let val_keys: Vec<Keypair> = (0..4).map(|_| Keypair::generate()).collect();
    let val_entries: Vec<ValidatorEntry> = val_keys
        .iter()
        .map(|kp| {
            let addr = derive_address_from_pubkey(&kp.public_key_bytes());
            ValidatorEntry {
                validator_id: addr,
                consensus_pubkey: kp.public_key_bytes(),
                voting_weight: 25,
            }
        })
        .collect();

    let creator_key = Keypair::generate();
    let creator_addr = derive_address_from_pubkey(&creator_key.public_key_bytes());
    let dev_key = Keypair::generate();
    let dev_addr = derive_address_from_pubkey(&dev_key.public_key_bytes());

    let genesis = build_genesis(creator_addr, dev_addr, val_entries.clone());
    let ledger = ChainLedger::from_genesis(genesis);
    let mut ref_stf = ReferenceSTF::new_genesis(creator_addr, dev_addr);

    let miner_key = Keypair::generate();
    let miner_addr = derive_address_from_pubkey(&miner_key.public_key_bytes());

    let attacker_key = Keypair::generate();
    let attacker_addr = derive_address_from_pubkey(&attacker_key.public_key_bytes());

    let mut mempool = MempoolEngine::new(1024 * 1024, 3600);

    // KASUS ADVERSARIAL A: Akun Tidak Memiliki Saldo Sama Sekali
    {
        let tx = Transaction {
            version: 1,
            chain_id: 1001,
            tx_type: TxType::Transfer,
            flags: 0,
            sender: attacker_addr,
            recipient: creator_addr,
            nonce: 0,
            amount: Quantum::from_aur(10).unwrap(),
            fee: Quantum::from_aur(1).unwrap(),
            valid_until: 1800000000,
            payload: Vec::new(),
            signature: Signature::from_bytes([0u8; 64]),
        };

        let empty_acct = aurion::state::Account::default();
        // Keduanya wajib menolak
        assert!(ref_stf.apply_tx(&miner_addr, &tx).is_err());
        assert!(mempool
            .submit_transaction(
                tx,
                &attacker_key.public_key_bytes(),
                1773532850,
                ledger.get_account(&attacker_addr).unwrap_or(&empty_acct),
            )
            .is_err());
    }

    // KASUS ADVERSARIAL B: Nonce Replay / Loncat Nonce
    {
        // Creator memiliki saldo, tapi kita buat transaksi dengan nonce salah (nonce = 999 bukannya 0)
        let tx = Transaction {
            version: 1,
            chain_id: 1001,
            tx_type: TxType::Transfer,
            flags: 0,
            sender: creator_addr,
            recipient: attacker_addr,
            nonce: 999, // SALAH
            amount: Quantum::from_aur(10).unwrap(),
            fee: Quantum::from_aur(1).unwrap(),
            valid_until: 1800000000,
            payload: Vec::new(),
            signature: Signature::from_bytes([0u8; 64]),
        };

        assert!(ref_stf.apply_tx(&miner_addr, &tx).is_err());
        assert!(mempool
            .submit_transaction(
                tx,
                &creator_key.public_key_bytes(),
                1773532850,
                ledger.get_account(&creator_addr).unwrap(),
            )
            .is_err());
    }

    // KASUS ADVERSARIAL C: Saldo Tidak Mencukupi (Amount + Fee > Balance)
    {
        let available = ledger.get_balance(&creator_addr);
        // Minta transfer sebesar available + 1 Quantum
        let amount = available;
        let fee = Quantum::new(1); // Menyebabkan defisit 1 Quantum

        let tx = Transaction {
            version: 1,
            chain_id: 1001,
            tx_type: TxType::Transfer,
            flags: 0,
            sender: creator_addr,
            recipient: attacker_addr,
            nonce: 0,
            amount,
            fee,
            valid_until: 1800000000,
            payload: Vec::new(),
            signature: Signature::from_bytes([0u8; 64]),
        };

        assert!(ref_stf.apply_tx(&miner_addr, &tx).is_err());
        assert!(mempool
            .submit_transaction(
                tx,
                &creator_key.public_key_bytes(),
                1773532850,
                ledger.get_account(&creator_addr).unwrap(),
            )
            .is_err());
    }

    // Verifikasi state kedua mesin tetap utuh dan tidak terkorupsi sedikit pun
    assert_eq!(
        ledger.get_balance(&creator_addr).as_u128(),
        ref_stf.get_balance(&creator_addr)
    );
    assert_eq!(
        ledger.get_nonce(&creator_addr),
        ref_stf.get_nonce(&creator_addr)
    );
}

// ==============================================================================
// 6. TEST 3: BOUNDARY VALUES & EXACT DEPLETION DIFFERENTIAL TEST
// ==============================================================================

#[test]
fn test_differential_boundary_exact_depletion() {
    let val_keys: Vec<Keypair> = (0..4).map(|_| Keypair::generate()).collect();
    let val_entries: Vec<ValidatorEntry> = val_keys
        .iter()
        .map(|kp| {
            let addr = derive_address_from_pubkey(&kp.public_key_bytes());
            ValidatorEntry {
                validator_id: addr,
                consensus_pubkey: kp.public_key_bytes(),
                voting_weight: 25,
            }
        })
        .collect();

    let creator_key = Keypair::generate();
    let creator_addr = derive_address_from_pubkey(&creator_key.public_key_bytes());
    let dev_key = Keypair::generate();
    let dev_addr = derive_address_from_pubkey(&dev_key.public_key_bytes());

    let genesis = build_genesis(creator_addr, dev_addr, val_entries.clone());
    let validator_set = ValidatorSet::new(val_entries);
    let mut ledger = ChainLedger::from_genesis(genesis);
    let mut ref_stf = ReferenceSTF::new_genesis(creator_addr, dev_addr);

    let bft = BftEngine::new(Some(val_keys[0].clone()), Some(0));

    let alice_key = Keypair::generate();
    let alice_addr = derive_address_from_pubkey(&alice_key.public_key_bytes());

    let bob_key = Keypair::generate();
    let bob_addr = derive_address_from_pubkey(&bob_key.public_key_bytes());

    let miner_key = Keypair::generate();
    let miner_addr = derive_address_from_pubkey(&miner_key.public_key_bytes());

    let mut current_time = 1773532800;

    // Langkah 1: Berikan tepat 100 Quantum ke Alice dari Creator
    {
        current_time += 60;
        let mut mempool = MempoolEngine::new(1024 * 1024, 3600);
        let mut tx = Transaction {
            version: 1,
            chain_id: 1001,
            tx_type: TxType::Transfer,
            flags: 0,
            sender: creator_addr,
            recipient: alice_addr,
            nonce: 0,
            amount: Quantum::new(100),
            fee: Quantum::new(50),
            valid_until: 1800000000,
            payload: Vec::new(),
            signature: Signature::from_bytes([0u8; 64]),
        };
        tx.signature = creator_key.sign(&tx.signing_preimage());

        mempool
            .submit_transaction(
                tx.clone(),
                &creator_key.public_key_bytes(),
                current_time,
                ledger.get_account(&creator_addr).unwrap(),
            )
            .unwrap();

        ref_stf.apply_tx(&miner_addr, &tx).unwrap();
        ref_stf.apply_block_subsidy(1, &miner_addr);

        produce_test_block(
            &bft,
            &mut ledger,
            &mempool,
            &val_keys,
            &validator_set,
            &miner_addr,
            current_time,
        );

        assert_eq!(ledger.get_balance(&alice_addr).as_u128(), 100);
        assert_eq!(ref_stf.get_balance(&alice_addr), 100);
    }

    // Langkah 2: Alice mentransfer seluruh 100 Quantum ke Bob (Amount: 80, Fee: 20 -> Saldo Alice menjadi tepat 0)
    {
        current_time += 60;
        let mut mempool = MempoolEngine::new(1024 * 1024, 3600);
        let mut tx = Transaction {
            version: 1,
            chain_id: 1001,
            tx_type: TxType::Transfer,
            flags: 0,
            sender: alice_addr,
            recipient: bob_addr,
            nonce: 0,
            amount: Quantum::new(80),
            fee: Quantum::new(20),
            valid_until: 1800000000,
            payload: Vec::new(),
            signature: Signature::from_bytes([0u8; 64]),
        };
        tx.signature = alice_key.sign(&tx.signing_preimage());

        mempool
            .submit_transaction(
                tx.clone(),
                &alice_key.public_key_bytes(),
                current_time,
                ledger.get_account(&alice_addr).unwrap(),
            )
            .unwrap();

        ref_stf.apply_tx(&miner_addr, &tx).unwrap();
        ref_stf.apply_block_subsidy(2, &miner_addr);

        produce_test_block(
            &bft,
            &mut ledger,
            &mempool,
            &val_keys,
            &validator_set,
            &miner_addr,
            current_time,
        );

        // Saldo Alice wajib tepat 0 Quantum tanpa underflow
        assert_eq!(ledger.get_balance(&alice_addr), Quantum::ZERO);
        assert_eq!(ref_stf.get_balance(&alice_addr), 0);
        assert_eq!(ledger.get_nonce(&alice_addr), 1);
        assert_eq!(ref_stf.get_nonce(&alice_addr), 1);

        // Saldo Bob menerima 80 Quantum
        assert_eq!(ledger.get_balance(&bob_addr).as_u128(), 80);
        assert_eq!(ref_stf.get_balance(&bob_addr), 80);
    }
}
