//! Pembangkit Golden Vectors Kanonikal Lintas Bahasa (JSON Export).
//! Digunakan oleh SDK eksternal (Go, Python, TypeScript) untuk memvalidasi kepatuhan byte-demi-byte.

pub fn generate_golden_vectors_json() -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"protocol\": \"Aurion\",\n");
    out.push_str("  \"version\": \"1.0.0\",\n");
    out.push_str("  \"constants\": {\n");
    out.push_str("    \"max_supply_aur\": 66000000,\n");
    out.push_str("    \"quanta_per_aur\": 100000000,\n");
    out.push_str("    \"max_supply_quanta\": \"6600000000000000\",\n");
    out.push_str("    \"genesis_allocation_quanta\": \"2310000000000000\",\n");
    out.push_str("    \"creator_allocation_quanta\": \"1980000000000000\",\n");
    out.push_str("    \"developer_allocation_quanta\": \"330000000000000\",\n");
    out.push_str("    \"fee_burn_percent\": 20,\n");
    out.push_str("    \"fee_miner_percent\": 80,\n");
    out.push_str("    \"wire_magic\": \"0x41555230\",\n");
    out.push_str("    \"wire_magic_str\": \"AUR0\",\n");
    out.push_str("    \"wire_header_bytes\": 52,\n");
    out.push_str("    \"block_header_bytes\": 124,\n");
    out.push_str("    \"vote_bytes\": 117,\n");
    out.push_str("    \"validator_entry_bytes\": 72,\n");
    out.push_str("    \"transaction_base_bytes\": 184\n");
    out.push_str("  },\n");
    out.push_str("  \"cryptography\": {\n");
    out.push_str("    \"blake3_empty_string\": \"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262\",\n");
    out.push_str("    \"blake3_aurion_string\": \"c81453c888cdd581753d4b72f466a585e98ffda6f68dbd2fabc5f43d5fee86c8\",\n");
    out.push_str("    \"blake3_kdf_context\": \"AURION-TEST-V1\",\n");
    out.push_str("    \"blake3_kdf_key_material\": \"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f\",\n");
    out.push_str("    \"blake3_kdf_digest\": \"d08662334d813cc3e872910df43129dd5dbf7ba49215352184def9f82ec5e195\",\n");
    out.push_str("    \"ed25519_seed\": \"9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60\",\n");
    out.push_str("    \"ed25519_public_key\": \"d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a\",\n");
    out.push_str("    \"ed25519_test_message\": \"AURION-CONSENSUS-TEST-MESSAGE\",\n");
    out.push_str("    \"ed25519_signature\": \"cb8dba50a61f5269904f7ea575769cfb4d6916af4ef4c984edeca8fbe36194f500b8373535e9806ad93d04921365f9820d8561ef050f2ff6d8e35f3da3452209\",\n");
    out.push_str("    \"raw_address\": \"7acb7a9e77ef27c92b8049ec96ea40901febbf3861e97a97f01e7a02f0170253\",\n");
    out.push_str("    \"bech32m_mainnet\": \"aur10t9h48nhaunuj2uqf8kfd6jqjq07h0ecv85h49lsreaq9uqhqffshf0p6h\",\n");
    out.push_str("    \"bech32m_testnet\": \"aurt10t9h48nhaunuj2uqf8kfd6jqjq07h0ecv85h49lsreaq9uqhqffsjazk8c\"\n");
    out.push_str("  },\n");
    out.push_str("  \"domain_separation_tags\": {\n");
    out.push_str("    \"tx\": \"AURION-TX-V1\",\n");
    out.push_str("    \"address\": \"AURION-ADDRESS-V1\",\n");
    out.push_str("    \"tx_id\": \"AURION-TX-ID-V1\",\n");
    out.push_str("    \"block_id\": \"AURION-BLOCK-ID-V1\",\n");
    out.push_str("    \"bft_proposal\": \"AURION-BFT-PROPOSAL-V1\",\n");
    out.push_str("    \"bft_prevote\": \"AURION-BFT-PREVOTE-V1\",\n");
    out.push_str("    \"bft_precommit\": \"AURION-BFT-PRECOMMIT-V1\",\n");
    out.push_str("    \"smt_branch\": \"AURION-SMT-BRANCH-V1\",\n");
    out.push_str("    \"smt_leaf\": \"AURION-SMT-LEAF-V1\"\n");
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}
