#![forbid(unsafe_code)]

//! Jaringan Relay Tepi (Edge Mesh) Zenoh & Perisai Proteksi Anti-DDoS L5 (REQ-L5-10).
//! Invariant: AUR-L5-ARCH-001 (Edge Decoupling from Consensus), AUR-L5-SEC-001 (Byzantine Resiliency).

use std::collections::BTreeMap;
use super::types::InfrastructureNodeId;

/// Metadata Peer Relay Tepi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayPeer {
    pub peer_id: InfrastructureNodeId,
    pub endpoint: String,
    pub capacity_pps: u32, // Packets per second
    pub is_active: bool,
}

/// Jaringan Relay Tepi Terdistribusi (Edge Relay Mesh).
#[derive(Debug, Default)]
pub struct EdgeRelayMesh {
    peers: BTreeMap<InfrastructureNodeId, RelayPeer>,
}

impl EdgeRelayMesh {
    pub fn new() -> Self {
        Self {
            peers: BTreeMap::new(),
        }
    }

    pub fn register_peer(&mut self, peer: RelayPeer) {
        self.peers.insert(peer.peer_id, peer);
    }

    pub fn active_peer_count(&self) -> usize {
        self.peers.values().filter(|p| p.is_active).count()
    }

    pub fn route_packet(&self, packet_bytes: &[u8]) -> Result<InfrastructureNodeId, &'static str> {
        let active_peers: Vec<&RelayPeer> = self.peers.values().filter(|p| p.is_active).collect();
        if active_peers.is_empty() {
            return Err("No active relay peers available in mesh");
        }

        // Deterministik routing hash
        let hash = blake3::hash(packet_bytes);
        let idx = (hash.as_bytes()[0] as usize) % active_peers.len();
        Ok(active_peers[idx].peer_id)
    }
}

/// Status Hasil Filter Perisai Anti-DDoS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DdosFilterDecision {
    Allow,
    RateLimited,
    Blacklisted,
}

/// Catatan Token Bucket Per Klien / Alamat Asal.
#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: u32,
    last_refill_slot: u64,
    violations: u32,
}

/// Perisai Proteksi Anti-DDoS Tepi (Edge Anti-DDoS Shield).
#[derive(Debug)]
pub struct AntiDdosShield {
    bucket_capacity: u32,
    refill_rate_per_slot: u32,
    max_violations_before_ban: u32,
    client_buckets: BTreeMap<[u8; 32], TokenBucket>,
    blacklist: BTreeMap<[u8; 32], u64>, // Client ID -> Ban until slot
}

impl AntiDdosShield {
    pub fn new(
        bucket_capacity: u32,
        refill_rate_per_slot: u32,
        max_violations_before_ban: u32,
    ) -> Self {
        Self {
            bucket_capacity,
            refill_rate_per_slot,
            max_violations_before_ban,
            client_buckets: BTreeMap::new(),
            blacklist: BTreeMap::new(),
        }
    }

    /// Mengevaluasi paket masuk dan memutuskan apakah diizinkan, dibatasi, atau diblokir.
    pub fn inspect_traffic(
        &mut self,
        source_id: &[u8; 32],
        current_slot: u64,
    ) -> DdosFilterDecision {
        // 1. Periksa blacklist
        if let Some(ban_until) = self.blacklist.get(source_id) {
            if current_slot <= *ban_until {
                return DdosFilterDecision::Blacklisted;
            } else {
                self.blacklist.remove(source_id);
            }
        }

        // 2. Dapatkan atau inisialisasi token bucket
        let capacity = self.bucket_capacity;
        let refill_rate = self.refill_rate_per_slot;
        let bucket = self.client_buckets.entry(*source_id).or_insert(TokenBucket {
            tokens: capacity,
            last_refill_slot: current_slot,
            violations: 0,
        });

        // 3. Refill token berdasarkan slot yang telah berlalu
        let slots_elapsed = current_slot.saturating_sub(bucket.last_refill_slot);
        if slots_elapsed > 0 {
            let tokens_to_add = slots_elapsed.saturating_mul(refill_rate as u64) as u32;
            bucket.tokens = bucket.tokens.saturating_add(tokens_to_add).min(capacity);
            bucket.last_refill_slot = current_slot;
        }

        // 4. Periksa ketersediaan token
        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            DdosFilterDecision::Allow
        } else {
            bucket.violations = bucket.violations.saturating_add(1);
            if bucket.violations >= self.max_violations_before_ban {
                let ban_duration = 100; // Ban selama 100 slot
                self.blacklist.insert(*source_id, current_slot + ban_duration);
                DdosFilterDecision::Blacklisted
            } else {
                DdosFilterDecision::RateLimited
            }
        }
    }

    pub fn is_blacklisted(&self, source_id: &[u8; 32], current_slot: u64) -> bool {
        self.blacklist
            .get(source_id)
            .map(|ban_until| current_slot <= *ban_until)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_relay_mesh_routing() {
        let mut mesh = EdgeRelayMesh::new();
        let p1 = RelayPeer {
            peer_id: [0x11; 32],
            endpoint: "tcp://10.0.0.1:7447".to_string(),
            capacity_pps: 50_000,
            is_active: true,
        };
        let p2 = RelayPeer {
            peer_id: [0x22; 32],
            endpoint: "tcp://10.0.0.2:7447".to_string(),
            capacity_pps: 50_000,
            is_active: true,
        };

        mesh.register_peer(p1);
        mesh.register_peer(p2);
        assert_eq!(mesh.active_peer_count(), 2);

        let packet = b"hello aurion l5 zenoh mesh";
        let target = mesh.route_packet(packet).expect("Routing should succeed");
        assert!(target == [0x11; 32] || target == [0x22; 32]);
    }

    #[test]
    fn test_anti_ddos_shield_rate_limiting_and_ban() {
        // Kapasitas 3 token, isi 1 per slot, ban setelah 2 pelanggaran
        let mut shield = AntiDdosShield::new(3, 1, 2);
        let client = [0x99; 32];

        // 3 request pertama di slot 10: Allow
        assert_eq!(shield.inspect_traffic(&client, 10), DdosFilterDecision::Allow);
        assert_eq!(shield.inspect_traffic(&client, 10), DdosFilterDecision::Allow);
        assert_eq!(shield.inspect_traffic(&client, 10), DdosFilterDecision::Allow);

        // Request ke-4: RateLimited (Pelanggaran 1)
        assert_eq!(shield.inspect_traffic(&client, 10), DdosFilterDecision::RateLimited);

        // Request ke-5: Blacklisted (Pelanggaran 2 -> Ban)
        assert_eq!(shield.inspect_traffic(&client, 10), DdosFilterDecision::Blacklisted);
        assert!(shield.is_blacklisted(&client, 10));
    }
}
