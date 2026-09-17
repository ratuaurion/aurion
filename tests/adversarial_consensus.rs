#![forbid(unsafe_code)]

//! Suite Pengujian Adversarial Konsensus BFT Aurion (VER-005).
//!
//! Menguji ketahanan, keamanan, dan keandalan konsensus Aurion-BFT Single-Slot Finality
//! terhadap aneka skenario serangan Byzantine dan kegagalan jaringan terdistribusi:
//!
//! 1. Toleransi Node Offline / Silent Byzantine (1 dari 4 validator offline, f < n/3, kuorum tercapai).
//! 2. Partisi Jaringan Simetris (2 vs 2, 50% vs 50%): penghentian progres aman tanpa pembelahan rantai (Zero Forks).
//! 3. Pemulihan Partisi Jaringan: rekoneksi cluster, transisi putaran deterministik, dan pemulihan finalitas.
//! 4. Deteksi Ekuivokasi & Double Voting: penolakan duplikasi suara dan penolakan suara dengan hash bertentangan.
//! 5. Penolakan Pemalsuan Tanda Tangan Suara (Forged Vote Signatures & Out-of-Bounds Validator Indices).
//! 6. Penolakan Manipulasi Fase Voting (Phase Confusion Attack & Illegal Phase Bytes).
//! 7. Pemilihan Proposer Deterministik & Rotasi Putaran Anti-Sensor.
//! 8. Simulator Jaringan Adversarial Terpadu: injeksi message drop, corrupt signatures, dan latency delay.

use std::collections::HashSet;

use aurion::consensus::bft::block::Block;
use aurion::consensus::bft::certificate::{
    CertificateError, CommitCertificate, ValidatorEntry, ValidatorSet,
};
use aurion::consensus::bft::engine::{BftEngine, BftEngineError};
use aurion::consensus::bft::vote::{
    Vote, VoteError, PHASE_PRECOMMIT, PHASE_PREVOTE,
};
use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::crypto::{derive_address_from_pubkey, Keypair};
use aurion::genesis::builder::build_genesis;
use aurion::mempool::MempoolEngine;
use aurion::state::chain::ChainLedger;
use aurion::transaction::types::{Transaction, TxType};

/// Helper untuk membuat cluster pengujian 4-validator kanonikal Aurion.
/// Setiap validator memiliki bobot suara 25 (Total: 100, Kuorum Q = 67).
struct ClusterFixture {
    val_keys: Vec<Keypair>,
    val_addrs: Vec<Address>,
    val_entries: Vec<ValidatorEntry>,
    validator_set: ValidatorSet,
    creator_key: Keypair,
    creator_addr: Address,
    dev_addr: Address,
    ledgers: Vec<ChainLedger>,
    mempools: Vec<MempoolEngine>,
    engines: Vec<BftEngine>,
}

impl ClusterFixture {
    fn new() -> Self {
        let val_keys: Vec<Keypair> = (0..4).map(|_| Keypair::generate()).collect();
        let val_addrs: Vec<Address> = val_keys
            .iter()
            .map(|kp| derive_address_from_pubkey(&kp.public_key_bytes()))
            .collect();

        let val_entries: Vec<ValidatorEntry> = val_keys
            .iter()
            .enumerate()
            .map(|(idx, kp)| ValidatorEntry {
                validator_id: val_addrs[idx],
                consensus_pubkey: kp.public_key_bytes(),
                voting_weight: 25,
            })
            .collect();

        let validator_set = ValidatorSet::new(val_entries.clone());
        assert_eq!(validator_set.total_voting_power(), 100);
        assert_eq!(validator_set.quorum_threshold(), 67);

        let creator_key = Keypair::generate();
        let creator_addr = derive_address_from_pubkey(&creator_key.public_key_bytes());

        let dev_key = Keypair::generate();
        let dev_addr = derive_address_from_pubkey(&dev_key.public_key_bytes());

        let genesis = build_genesis(creator_addr, dev_addr, val_entries.clone());

        let mut ledgers = Vec::with_capacity(4);
        let mut mempools = Vec::with_capacity(4);
        let mut engines = Vec::with_capacity(4);

        for (idx, key) in val_keys.iter().enumerate().take(4) {
            ledgers.push(ChainLedger::from_genesis(genesis.clone()));
            mempools.push(MempoolEngine::new(1024 * 1024, 3600));
            engines.push(BftEngine::new(
                Some(key.clone()),
                Some(idx as u32),
            ));
        }

        Self {
            val_keys,
            val_addrs,
            val_entries,
            validator_set,
            creator_key,
            creator_addr,
            dev_addr,
            ledgers,
            mempools,
            engines,
        }
    }

    /// Membuat transaksi transfer sah dari creator untuk menguji eksekusi transaksi.
    fn create_tx(&self, recipient: Address, amount_aur: u64, fee_aur: u64, nonce: u64) -> Transaction {
        let amount = Quantum::from_aur(amount_aur).expect("Valid amount");
        let fee = Quantum::from_aur(fee_aur).expect("Valid fee");

        let mut tx = Transaction {
            version: 1,
            chain_id: 1,
            tx_type: TxType::Transfer,
            flags: 0,
            sender: self.creator_addr,
            recipient,
            nonce,
            amount,
            fee,
            valid_until: 1800000000,
            payload: Vec::new(),
            signature: Signature::from_bytes([0u8; 64]),
        };

        let preimage = tx.signing_preimage();
        tx.signature = self.creator_key.sign(&preimage);
        tx
    }
}

// ============================================================================
// SKENARIO 1: TOLERANSI NODE OFFLINE (1/4 BYZANTINE OFFLINE, f < n/3)
// ============================================================================
#[test]
fn test_adversarial_offline_validator_tolerance_liveness() {
    let mut cluster = ClusterFixture::new();

    // Validator 3 disimulasikan offline total (crash / disconnected).
    // Validator online: 0, 1, 2 (Total Bobot Suara: 3 * 25 = 75 >= 67 Kuorum).
    let online_indices = [0usize, 1usize, 2usize];

    // Buat transaksi dan submit ke mempool validator online
    let tx = cluster.create_tx(cluster.dev_addr, 100, 1, 0);
    for &idx in &online_indices {
        let acct = cluster.ledgers[idx]
            .get_account(&cluster.creator_addr)
            .unwrap()
            .clone();
        cluster.mempools[idx]
            .submit_transaction(
                tx.clone(),
                &cluster.creator_key.public_key_bytes(),
                1773532850,
                &acct,
            )
            .expect("Mempool submission succeeded");
    }

    // Tentukan proposer deterministik untuk H=1, R=0
    let prev_hash = cluster.ledgers[0].latest_block().hash();
    let proposer_idx = BftEngine::select_proposer(&cluster.validator_set, 1, 0, &prev_hash) as usize;

    // Pastikan proposer termasuk validator online (jika idx 3 terpilih, fallback ke round 1)
    let round = if proposer_idx == 3 { 1 } else { 0 };
    let actual_proposer = BftEngine::select_proposer(&cluster.validator_set, 1, round, &prev_hash) as usize;
    assert_ne!(actual_proposer, 3, "Proposer must be an online validator");

    let miner_addr = cluster.val_addrs[actual_proposer];

    // Proposer merakit blok proposal
    let candidate = cluster.engines[actual_proposer].assemble_block_proposal(
        &cluster.ledgers[actual_proposer],
        &cluster.mempools[actual_proposer],
        round,
        1773532900,
        &miner_addr,
        1024 * 1024,
    );
    assert_eq!(candidate.height(), 1);
    assert_eq!(candidate.transactions.len(), 1);

    let block_hash = candidate.hash();

    // Fase 1: Prevote dari 3 validator online (0, 1, 2)
    let mut prevote_weight = 0u64;
    for &idx in &online_indices {
        let prevote = cluster.engines[idx]
            .produce_prevote(block_hash, 1, round)
            .expect("Prevote produce succeeded");
        prevote.verify(&cluster.validator_set).expect("Prevote valid");
        prevote_weight += cluster.val_entries[idx].voting_weight;
    }
    assert!(prevote_weight >= cluster.validator_set.quorum_threshold());

    // Fase 2: Precommit dari 3 validator online (0, 1, 2)
    let mut precommits = Vec::new();
    for &idx in &online_indices {
        let precommit = cluster.engines[idx]
            .produce_precommit(block_hash, 1, round)
            .expect("Precommit produce succeeded");
        precommit.verify(&cluster.validator_set).expect("Precommit valid");
        precommits.push(precommit);
    }

    // Bentuk CommitCertificate
    let cert = cluster.engines[actual_proposer]
        .create_commit_certificate(&cluster.validator_set, block_hash, 1, round, precommits)
        .expect("Quorum reached with 75 voting weight");

    // Komit blok ke seluruh node online
    let block = Block::new(candidate.header, candidate.transactions, Some(cert));
    for &idx in &online_indices {
        cluster.ledgers[idx]
            .apply_block(block.clone(), &miner_addr)
            .expect("Block apply succeeded on online node");
        assert_eq!(cluster.ledgers[idx].latest_height(), 1);
    }

    // Node 3 tetap pada H=0 karena offline
    assert_eq!(cluster.ledgers[3].latest_height(), 0);

    // Verifikasi identitas konsensus: seluruh node online memiliki state root yang 100% identik
    let root_0 = cluster.ledgers[0].latest_block().header.state_root;
    let root_1 = cluster.ledgers[1].latest_block().header.state_root;
    let root_2 = cluster.ledgers[2].latest_block().header.state_root;
    assert_eq!(root_0, root_1);
    assert_eq!(root_1, root_2);
}

// ============================================================================
// SKENARIO 2: PARTISI JARINGAN SIMETRIS (2 VS 2): PENGHENTIAN AMAN & ZERO FORKS
// ============================================================================
#[test]
fn test_adversarial_network_partition_zero_forks_safety() {
    let cluster = ClusterFixture::new();

    // Jaringan terbelah menjadi dua partisi yang tidak dapat saling berkomunikasi:
    // Partisi A: Validator 0 & 1 (Bobot: 50, Kuorum Butuh: 67)
    // Partisi B: Validator 2 & 3 (Bobot: 50, Kuorum Butuh: 67)

    // Partisi A mencoba merakit blok 1A
    let miner_a = cluster.val_addrs[0];
    let candidate_a = cluster.engines[0].assemble_block_proposal(
        &cluster.ledgers[0],
        &cluster.mempools[0],
        0,
        1773532910,
        &miner_a,
        1024 * 1024,
    );
    let hash_a = candidate_a.hash();

    // Partisi A hanya mengumpulkan precommit dari Val 0 dan Val 1
    let precommit_0 = cluster.engines[0].produce_precommit(hash_a, 1, 0).unwrap();
    let precommit_1 = cluster.engines[1].produce_precommit(hash_a, 1, 0).unwrap();

    let cert_a_res = cluster.engines[0].create_commit_certificate(
        &cluster.validator_set,
        hash_a,
        1,
        0,
        vec![precommit_0, precommit_1],
    );

    // Verifikasi kegagalan kuorum di Partisi A
    assert_eq!(
        cert_a_res.unwrap_err(),
        BftEngineError::Certificate(CertificateError::QuorumNotReached {
            accumulated: 50,
            required: 67,
        })
    );

    // Partisi B mencoba merakit blok 1B (dengan miner berbeda)
    let miner_b = cluster.val_addrs[2];
    let candidate_b = cluster.engines[2].assemble_block_proposal(
        &cluster.ledgers[2],
        &cluster.mempools[2],
        0,
        1773532920,
        &miner_b,
        1024 * 1024,
    );
    let hash_b = candidate_b.hash();

    // Partisi B hanya mengumpulkan precommit dari Val 2 dan Val 3
    let precommit_2 = cluster.engines[2].produce_precommit(hash_b, 1, 0).unwrap();
    let precommit_3 = cluster.engines[3].produce_precommit(hash_b, 1, 0).unwrap();

    let cert_b_res = cluster.engines[2].create_commit_certificate(
        &cluster.validator_set,
        hash_b,
        1,
        0,
        vec![precommit_2, precommit_3],
    );

    // Verifikasi kegagalan kuorum di Partisi B
    assert_eq!(
        cert_b_res.unwrap_err(),
        BftEngineError::Certificate(CertificateError::QuorumNotReached {
            accumulated: 50,
            required: 67,
        })
    );

    // ZERO FORKS: Kedua sisi tidak mampu memfinalisasi blok.
    // Seluruh simpul tetap berada di H=0 secara aman tanpa percabangan state.
    for ledger in &cluster.ledgers {
        assert_eq!(ledger.latest_height(), 0);
    }
}

// ============================================================================
// SKENARIO 3: PEMULIHAN PARTISI & REKONEKSI JARINGAN
// ============================================================================
#[test]
fn test_adversarial_partition_healing_and_recovery() {
    let mut cluster = ClusterFixture::new();

    // Setelah kegagalan partisi di Round 0, partisi sembuh dan jaringan beralih ke Round 1
    let prev_hash = cluster.ledgers[0].latest_block().hash();
    let round = 1;
    let proposer_idx = BftEngine::select_proposer(&cluster.validator_set, 1, round, &prev_hash) as usize;
    let miner_addr = cluster.val_addrs[proposer_idx];

    // Proposer terpilih merakit blok di Round 1
    let candidate = cluster.engines[proposer_idx].assemble_block_proposal(
        &cluster.ledgers[proposer_idx],
        &cluster.mempools[proposer_idx],
        round,
        1773532950,
        &miner_addr,
        1024 * 1024,
    );
    let block_hash = candidate.hash();

    // Seluruh 4 validator telah terhubung kembali dan memberikan suara di Round 1
    let mut precommits = Vec::new();
    for idx in 0..4 {
        let prevote = cluster.engines[idx].produce_prevote(block_hash, 1, round).unwrap();
        prevote.verify(&cluster.validator_set).unwrap();

        let precommit = cluster.engines[idx].produce_precommit(block_hash, 1, round).unwrap();
        precommit.verify(&cluster.validator_set).unwrap();
        precommits.push(precommit);
    }

    // Sertifikat berhasil dibentuk dengan bobot 100 >= 67
    let cert = cluster.engines[proposer_idx]
        .create_commit_certificate(&cluster.validator_set, block_hash, 1, round, precommits)
        .expect("Reconnected cluster reaches 100% quorum");

    let block = Block::new(candidate.header, candidate.transactions, Some(cert));

    // Seluruh 4 node berhasil menerapkan blok hasil pemulihan
    for ledger in &mut cluster.ledgers {
        ledger.apply_block(block.clone(), &miner_addr).expect("Block applied after partition healed");
        assert_eq!(ledger.latest_height(), 1);
        assert_eq!(ledger.latest_block().hash(), block.hash());
    }
}

// ============================================================================
// SKENARIO 4: DETEKSI EKUIVOKASI & DOUBLE VOTING (BYZANTINE ATTACK)
// ============================================================================
#[test]
fn test_adversarial_double_voting_equivocation_detected_and_rejected() {
    let cluster = ClusterFixture::new();

    let miner = cluster.val_addrs[0];
    let block_a = cluster.engines[0].assemble_block_proposal(
        &cluster.ledgers[0],
        &cluster.mempools[0],
        0,
        1773532960,
        &miner,
        1024 * 1024,
    );
    let hash_a = block_a.hash();

    // Buat blok kedua yang bertentangan di height dan round yang sama
    let mut header_b = block_a.header.clone();
    header_b.timestamp += 1;
    let block_b = Block::new(header_b, Vec::new(), None);
    let hash_b = block_b.hash();
    assert_ne!(hash_a, hash_b);

    // Kasus 4a: Validator 1 melakukan ekuivokasi (menandatangani dua suara berbeda untuk round yang sama)
    let precommit_1_for_a = cluster.engines[1].produce_precommit(hash_a, 1, 0).unwrap();
    let precommit_1_for_b = cluster.engines[1].produce_precommit(hash_b, 1, 0).unwrap();

    // Upaya 1: Penyerang menduplikasi suara Validator 1 pada sertifikat yang sama untuk menggelembungkan bobot
    let precommit_0 = cluster.engines[0].produce_precommit(hash_a, 1, 0).unwrap();
    let duplicate_votes = vec![
        precommit_0.clone(),
        precommit_1_for_a.clone(),
        precommit_1_for_a.clone(), // Duplikat suara Val 1
    ];

    let dup_cert_res = cluster.engines[0].create_commit_certificate(
        &cluster.validator_set,
        hash_a,
        1,
        0,
        duplicate_votes,
    );

    assert_eq!(
        dup_cert_res.unwrap_err(),
        BftEngineError::Certificate(CertificateError::DuplicateVote(1))
    );

    // Upaya 2: Penyerang menyelipkan suara untuk hash_b ke dalam sertifikat hash_a
    let conflicting_votes = vec![
        precommit_0,
        precommit_1_for_b, // Berisi hash_b, bukan hash_a
    ];

    let conflict_cert_res = cluster.engines[0].create_commit_certificate(
        &cluster.validator_set,
        hash_a,
        1,
        0,
        conflicting_votes,
    );

    assert_eq!(
        conflict_cert_res.unwrap_err(),
        BftEngineError::Certificate(CertificateError::MismatchedBlockHash)
    );
}

// ============================================================================
// SKENARIO 5: PENOLAKAN PEMALSUAN TANDA TANGAN SUARA (FORGED SIGNATURE & SYBIL)
// ============================================================================
#[test]
fn test_adversarial_forged_vote_signature_and_sybil_rejected() {
    let cluster = ClusterFixture::new();

    let dummy_hash = Hash256::from_bytes([0xAA; 32]);
    let attacker_key = Keypair::generate();

    // Kasus 5a: Penyerang mengaku sebagai Validator 0 tetapi menandatangani dengan kuncinya sendiri
    let forged_vote = Vote::new_signed(&attacker_key, PHASE_PRECOMMIT, 1, 0, dummy_hash, 0)
        .expect("Vote signed syntactically");

    // Verifikasi individual harus menolak tanda tangan palsu
    let verify_res = forged_vote.verify(&cluster.validator_set);
    assert_eq!(verify_res.unwrap_err(), VoteError::InvalidSignature);

    // Verifikasi dalam CommitCertificate juga harus menolak
    let cert = CommitCertificate {
        block_hash: dummy_hash,
        height: 1,
        round: 0,
        precommits: vec![forged_vote],
    };
    assert_eq!(
        cert.verify(&cluster.validator_set).unwrap_err(),
        CertificateError::InvalidSignature(0)
    );

    // Kasus 5b: Penyerang menggunakan validator_index di luar batas (Sybil Index out-of-bounds)
    let out_of_bounds_idx = 99u32;
    let oob_vote = Vote::new_signed(&attacker_key, PHASE_PRECOMMIT, 1, 0, dummy_hash, out_of_bounds_idx)
        .expect("Vote signed syntactically");

    assert_eq!(
        oob_vote.verify(&cluster.validator_set).unwrap_err(),
        VoteError::ValidatorIndexOutOfBounds {
            index: 99,
            set_size: 4,
        }
    );

    let oob_cert = CommitCertificate {
        block_hash: dummy_hash,
        height: 1,
        round: 0,
        precommits: vec![oob_vote],
    };
    assert_eq!(
        oob_cert.verify(&cluster.validator_set).unwrap_err(),
        CertificateError::UnknownValidator(99)
    );

    // Kasus 5c: Tampering byte payload suara sah setelah ditandatangani
    let mut legitimate_vote = cluster.engines[0]
        .produce_precommit(dummy_hash, 1, 0)
        .expect("Valid vote produced");
    assert!(legitimate_vote.verify(&cluster.validator_set).is_ok());

    // Manipulasi hash blok pada suara
    legitimate_vote.block_hash = Hash256::from_bytes([0xFF; 32]);
    assert_eq!(
        legitimate_vote.verify(&cluster.validator_set).unwrap_err(),
        VoteError::InvalidSignature
    );
}

// ============================================================================
// SKENARIO 6: PENOLAKAN MANIPULASI FASE VOTING (PHASE CONFUSION ATTACK)
// ============================================================================
#[test]
fn test_adversarial_phase_confusion_and_illegal_phase_rejected() {
    let cluster = ClusterFixture::new();
    let dummy_hash = Hash256::from_bytes([0xBB; 32]);

    // Kasus 6a: Menggunakan suara Prevote (0x01) di dalam CommitCertificate (harus Precommit 0x02)
    let prevote = cluster.engines[0]
        .produce_prevote(dummy_hash, 1, 0)
        .expect("Prevote produce succeeded");
    assert_eq!(prevote.phase, PHASE_PREVOTE);

    let cert_with_prevote = CommitCertificate {
        block_hash: dummy_hash,
        height: 1,
        round: 0,
        precommits: vec![prevote],
    };

    assert_eq!(
        cert_with_prevote.verify(&cluster.validator_set).unwrap_err(),
        CertificateError::InvalidPhase(PHASE_PREVOTE)
    );

    // Kasus 6b: Pembuatan suara dengan nilai fase ilegal (bukan 0x01 dan bukan 0x02)
    let illegal_phase = 0x03u8;
    let illegal_vote_res = Vote::new_signed(
        &cluster.val_keys[0],
        illegal_phase,
        1,
        0,
        dummy_hash,
        0,
    );

    assert_eq!(
        illegal_vote_res.unwrap_err(),
        VoteError::InvalidPhase(illegal_phase)
    );
}

// ============================================================================
// SKENARIO 7: DETERMINISME & ROTASI PROPOSER ANTI-SENSOR
// ============================================================================
#[test]
fn test_adversarial_proposer_election_determinism_and_fair_rotation() {
    let cluster = ClusterFixture::new();
    let prev_hash = Hash256::from_bytes([0x42; 32]);

    // 1. Deterministik 100%: 1000 iterasi dengan parameter identik menghasilkan proposer yang identik
    let expected_p = BftEngine::select_proposer(&cluster.validator_set, 10, 0, &prev_hash);
    for _ in 0..1000 {
        let p = BftEngine::select_proposer(&cluster.validator_set, 10, 0, &prev_hash);
        assert_eq!(p, expected_p);
    }

    // 2. Batas index selalu valid (< jumlah validator)
    for h in 1..=50 {
        for r in 0..=5 {
            let p = BftEngine::select_proposer(&cluster.validator_set, h, r, &prev_hash);
            assert!((p as usize) < cluster.validator_set.validators.len());
        }
    }

    // 3. Rotasi putaran anti-sensor: Ketika putaran dinaikkan karena timeout,
    // proposer berganti secara deterministik untuk mencegah sensor permanen oleh 1 node jahat.
    let mut proposers_seen = HashSet::new();
    for r in 0..10 {
        let p = BftEngine::select_proposer(&cluster.validator_set, 1, r, &prev_hash);
        proposers_seen.insert(p);
    }
    // Dari 10 putaran, setidaknya lebih dari 1 validator unik terpilih
    assert!(proposers_seen.len() > 1, "Proposer must rotate across rounds");

    // 4. Kasus himpunan validator kosong
    let empty_set = ValidatorSet::new(Vec::new());
    assert_eq!(BftEngine::select_proposer(&empty_set, 1, 0, &prev_hash), 0);
}

// ============================================================================
// SKENARIO 8: SIMULATOR JARINGAN ADVERSARIAL LENGKAP (MULTI-ROUND BYZANTINE)
// ============================================================================

/// Model pesan jaringan dalam simulasi P2P BFT.
#[derive(Clone, Debug)]
enum ConsensusMessage {
    BlockProposal(Block),
    Prevote(Vote),
    Precommit(Vote),
}

/// Simulator Jaringan Adversarial dengan kemampuan injeksi kesalahan terdistribusi.
struct AdversarialNetworkSimulator {
    dropped_links: HashSet<(u32, u32)>, // (sender, receiver)
    corrupted_nodes: HashSet<u32>,      // Node Byzantine yang merusak payload
}

impl AdversarialNetworkSimulator {
    fn new() -> Self {
        Self {
            dropped_links: HashSet::new(),
            corrupted_nodes: HashSet::new(),
        }
    }

    fn drop_link(&mut self, from: u32, to: u32) {
        self.dropped_links.insert((from, to));
    }

    fn restore_link(&mut self, from: u32, to: u32) {
        self.dropped_links.remove(&(from, to));
    }

    fn corrupt_node(&mut self, node: u32) {
        self.corrupted_nodes.insert(node);
    }

    fn deliver(&self, from: u32, to: u32, msg: &ConsensusMessage) -> Option<ConsensusMessage> {
        if self.dropped_links.contains(&(from, to)) {
            return None; // Message dropped
        }

        if self.corrupted_nodes.contains(&from) {
            // Simulasi Byzantine: memalsukan signature suara
            match msg {
                ConsensusMessage::Precommit(v) => {
                    let mut bad_vote = v.clone();
                    bad_vote.signature = Signature::from_bytes([0xEE; 64]);
                    return Some(ConsensusMessage::Precommit(bad_vote));
                }
                ConsensusMessage::Prevote(v) => {
                    let mut bad_vote = v.clone();
                    bad_vote.signature = Signature::from_bytes([0xEE; 64]);
                    return Some(ConsensusMessage::Prevote(bad_vote));
                }
                _ => return Some(msg.clone()),
            }
        }

        Some(msg.clone())
    }
}

#[test]
fn test_adversarial_byzantine_network_simulator_multi_round() {
    let mut cluster = ClusterFixture::new();
    let mut net = AdversarialNetworkSimulator::new();

    let mut history: Vec<(Block, Address)> = Vec::new();

    // Jalankan 5 blok berturut-turut di bawah aneka gangguan jaringan adversarial
    for height in 1..=5 {
        let prev_hash = cluster.ledgers[0].latest_block().hash();
        let mut round = 0;

        // Skenario gangguan per tinggi blok:
        if height == 2 {
            // Blok 2: Node 3 mengalami pemutusan jaringan total (dropped links dari/ke Node 3)
            for i in 0..4 {
                net.drop_link(3, i);
                net.drop_link(i, 3);
            }
        } else if height == 3 {
            // Blok 3: Jaringan pulih, namun Node 1 berperilaku Byzantine (mengirim tanda tangan rusak)
            for i in 0..4 {
                net.restore_link(3, i);
                net.restore_link(i, 3);
            }
            net.corrupt_node(1);
        } else if height == 4 {
            // Blok 4: Node 1 dinetralkan kembali
            net.corrupted_nodes.remove(&1);
        }

        // Jika proposer terpilih sedang terputus (offline), pacemaker BFT memicu timeout
        // dan menaikkan putaran ke round berikutnya secara deterministik hingga proposer online
        let mut proposer = BftEngine::select_proposer(&cluster.validator_set, height, round, &prev_hash);
        while net.dropped_links.contains(&(proposer, (proposer + 1) % 4)) {
            round += 1;
            proposer = BftEngine::select_proposer(&cluster.validator_set, height, round, &prev_hash);
        }
        let miner = cluster.val_addrs[proposer as usize];

        // 1. Proposer merakit blok
        let candidate = cluster.engines[proposer as usize].assemble_block_proposal(
            &cluster.ledgers[proposer as usize],
            &cluster.mempools[proposer as usize],
            round,
            1773533000 + height * 10,
            &miner,
            1024 * 1024,
        );
        let block_hash = candidate.hash();
        let proposal_msg = ConsensusMessage::BlockProposal(candidate.clone());

        // 2. Broadcast proposal dan kumpulkan Prevote
        let mut collected_prevotes = Vec::new();
        for sender in 0..4 {
            // Evaluasi apakah sender menerima proposal
            if let Some(ConsensusMessage::BlockProposal(ref received_block)) =
                net.deliver(proposer, sender, &proposal_msg)
            {
                assert_eq!(received_block.hash(), block_hash);
                let prevote = cluster.engines[sender as usize]
                    .produce_prevote(block_hash, height, round)
                    .unwrap();

                // Broadcast prevote ke proposer
                if let Some(ConsensusMessage::Prevote(pv)) =
                    net.deliver(sender, proposer, &ConsensusMessage::Prevote(prevote))
                {
                    if pv.verify(&cluster.validator_set).is_ok() {
                        collected_prevotes.push(pv);
                    }
                }
            }
        }

        // Pastikan prevote mencapai kuorum (> 2/3 bobot)
        let prevote_weight: u64 = collected_prevotes
            .iter()
            .map(|v| cluster.val_entries[v.validator_index as usize].voting_weight)
            .sum();
        assert!(
            prevote_weight >= cluster.validator_set.quorum_threshold(),
            "Height {} Prevote failed to reach quorum: accumulated {}",
            height,
            prevote_weight
        );

        // 3. Broadcast Precommit
        let mut collected_precommits = Vec::new();
        for sender in 0..4 {
            if let Some(ConsensusMessage::BlockProposal(ref received_block)) =
                net.deliver(proposer, sender, &proposal_msg)
            {
                assert_eq!(received_block.hash(), block_hash);
                let precommit = cluster.engines[sender as usize]
                    .produce_precommit(block_hash, height, round)
                    .unwrap();

                // Broadcast precommit ke proposer
                if let Some(ConsensusMessage::Precommit(pc)) =
                    net.deliver(sender, proposer, &ConsensusMessage::Precommit(precommit))
                {
                    if pc.verify(&cluster.validator_set).is_ok() {
                        collected_precommits.push(pc);
                    }
                }
            }
        }

        // 4. Bentuk CommitCertificate
        let cert = cluster.engines[proposer as usize]
            .create_commit_certificate(
                &cluster.validator_set,
                block_hash,
                height,
                round,
                collected_precommits,
            )
            .expect("Commit certificate successfully generated under BFT tolerance");

        let finalized_block = Block::new(candidate.header, candidate.transactions, Some(cert));
        history.push((finalized_block.clone(), miner));

        // 5. Terapkan ke semua simpul yang aktif (sinkronkan blok tertinggal bila ada)
        for node_idx in 0..4 {
            if !net.dropped_links.contains(&(node_idx, proposer)) {
                let cur_h = cluster.ledgers[node_idx as usize].latest_height();
                for (past_b, past_m) in &history {
                    if past_b.height() > cur_h {
                        cluster.ledgers[node_idx as usize]
                            .apply_block(past_b.clone(), past_m)
                            .expect("Block apply / catchup succeeded");
                    }
                }
            }
        }
    }

    // Verifikasi akhir: Seluruh 4 simpul (termasuk Node 3 yang sempat offline dan catchup)
    // berhasil memfinalisasi hingga Blok 5 dengan state root 100% identik
    let expected_root = cluster.ledgers[0].latest_block().header.state_root;
    for idx in 0..4 {
        assert_eq!(cluster.ledgers[idx].latest_height(), 5);
        assert_eq!(
            cluster.ledgers[idx].latest_block().header.state_root,
            expected_root
        );
    }
}
