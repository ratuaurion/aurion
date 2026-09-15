//! Mesin Langganan Aliran Data WebSocket (WebSocket Pub/Sub Engine) `aur_subscribe`.
//! Mematuhi Dokumen 02 (02-RPC-API-RULES.md Bagian 6).

use crate::consensus::header::BlockHeader;
use crate::core::{Address, Hash256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubscriptionTopic {
    NewHeads,
    FinalizedHeads,
    NewPendingTransactions,
    AddressEvents(Address),
}

impl SubscriptionTopic {
    pub fn parse(topic_str: &str, target_address: Option<Address>) -> Option<Self> {
        match topic_str {
            "newHeads" => Some(Self::NewHeads),
            "finalizedHeads" => Some(Self::FinalizedHeads),
            "newPendingTransactions" => Some(Self::NewPendingTransactions),
            "addressEvents" => target_address.map(Self::AddressEvents),
            _ => None,
        }
    }
}

pub struct Subscriber {
    pub id: u64,
    pub topic: SubscriptionTopic,
    pub sender: mpsc::UnboundedSender<String>,
}

/// Pengelola langganan real-time WebSocket simpul Aurion.
pub struct SubscriptionManager {
    next_sub_id: AtomicU64,
    subscribers: Mutex<HashMap<u64, Subscriber>>,
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionManager {
    pub fn new() -> Self {
        Self {
            next_sub_id: AtomicU64::new(1),
            subscribers: Mutex::new(HashMap::new()),
        }
    }

    /// Mendaftarkan klien baru ke topik langganan tertentu.
    pub fn subscribe(
        &self,
        topic: SubscriptionTopic,
        sender: mpsc::UnboundedSender<String>,
    ) -> u64 {
        let sub_id = self.next_sub_id.fetch_add(1, Ordering::SeqCst);
        let mut subs = self.subscribers.lock().unwrap();
        subs.insert(
            sub_id,
            Subscriber {
                id: sub_id,
                topic,
                sender,
            },
        );
        sub_id
    }

    /// Membatalkan langganan klien berdasarkan subscription ID.
    pub fn unsubscribe(&self, sub_id: u64) -> bool {
        let mut subs = self.subscribers.lock().unwrap();
        subs.remove(&sub_id).is_some()
    }

    /// Menyiarkan blok baru ke seluruh pelanggan `newHeads`.
    pub fn notify_new_head(&self, header: &BlockHeader) {
        let payload = format!(
            r#"{{"jsonrpc":"2.0","method":"aur_subscription","params":{{"subscription":"{}","result":{{"height":{},"round":{},"timestamp":{},"prev_hash":"{}"}}}}}}"#,
            "{}",
            header.height,
            header.round,
            header.timestamp,
            hex::encode(header.prev_block_hash.as_bytes())
        );
        self.broadcast_topic(&SubscriptionTopic::NewHeads, &payload);
    }

    /// Menyiarkan blok final ke seluruh pelanggan `finalizedHeads`.
    pub fn notify_finalized_head(&self, header: &BlockHeader) {
        let payload = format!(
            r#"{{"jsonrpc":"2.0","method":"aur_subscription","params":{{"subscription":"{}","result":{{"height":{},"round":{},"timestamp":{},"prev_hash":"{}","status":"FINALIZED"}}}}}}"#,
            "{}",
            header.height,
            header.round,
            header.timestamp,
            hex::encode(header.prev_block_hash.as_bytes())
        );
        self.broadcast_topic(&SubscriptionTopic::FinalizedHeads, &payload);
    }

    /// Menyiarkan TxID baru yang diterima di mempool ke `newPendingTransactions`.
    pub fn notify_pending_tx(&self, tx_id: &Hash256) {
        let tx_hex = hex::encode(tx_id.as_bytes());
        let payload = format!(
            r#"{{"jsonrpc":"2.0","method":"aur_subscription","params":{{"subscription":"{}","result":"{}"}}}}"#,
            "{}", tx_hex
        );
        self.broadcast_topic(&SubscriptionTopic::NewPendingTransactions, &payload);
    }

    fn broadcast_topic(&self, target_topic: &SubscriptionTopic, payload_template: &str) {
        let mut dead_subscribers = Vec::new();
        let subs = self.subscribers.lock().unwrap();

        for (id, sub) in subs.iter() {
            if sub.topic == *target_topic {
                let formatted = payload_template.replacen("{}", &format!("{id}"), 1);
                if sub.sender.send(formatted).is_err() {
                    dead_subscribers.push(*id);
                }
            }
        }
        drop(subs);

        // Hapus koneksi klien yang terputus
        if !dead_subscribers.is_empty() {
            let mut subs = self.subscribers.lock().unwrap();
            for id in dead_subscribers {
                subs.remove(&id);
            }
        }
    }

    pub fn active_subscribers_count(&self) -> usize {
        self.subscribers.lock().unwrap().len()
    }
}
