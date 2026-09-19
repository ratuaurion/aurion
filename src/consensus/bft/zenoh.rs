#![forbid(unsafe_code)]

use super::block::BlockProposalEnvelope;
use super::transport::{
    BftTransport, ConsensusMessage, TransportError, MAX_PROPOSAL_WIRE_BYTES,
    MAX_TRANSACTION_WIRE_BYTES,
};
use super::vote::{Vote, VOTE_BYTES};
use crate::codec::{CanonicalDecode, CanonicalEncode};
use crate::genesis::builder::GENESIS_CHAIN_ID;
use crate::transaction::types::Transaction;
use std::sync::Arc;
use tokio::sync::mpsc;
use zenoh::config::Config as ZenohConfig;
use zenoh::handlers::FifoChannelHandler;
use zenoh::key_expr::KeyExpr;
use zenoh::sample::Sample;
use zenoh::Session;

const PROPOSAL_TOPIC: &str = "consensus/proposal";
const VOTE_TOPIC: &str = "consensus/vote";
const TRANSACTION_TOPIC: &str = "mempool/tx";
const MESSAGE_PROPOSAL: u8 = 0x01;
const MESSAGE_VOTE: u8 = 0x02;
const MESSAGE_TRANSACTION: u8 = 0x03;
const FRAME_PREFIX_BYTES: usize = 5;

pub struct ZenohBftTransport {
    validator_index: u32,
    chain_id: u32,
    session: Arc<Session>,
    receiver: mpsc::Receiver<Result<ConsensusMessage, TransportError>>,
}

impl ZenohBftTransport {
    pub async fn from_session(
        session: Arc<Session>,
        validator_index: u32,
        chain_id: u32,
    ) -> Result<Self, TransportError> {
        let prefix = format!("aurion/{chain_id}");
        let proposal_key = format!("{prefix}/{PROPOSAL_TOPIC}");
        let vote_key = format!("{prefix}/{VOTE_TOPIC}");
        let transaction_key = format!("{prefix}/{TRANSACTION_TOPIC}");
        let (sender, receiver) = mpsc::channel(256);

        let proposal_subscriber = session
            .declare_subscriber(key_expr(&proposal_key)?)
            .await
            .map_err(|error| TransportError::Zenoh(error.to_string()))?;
        let vote_subscriber = session
            .declare_subscriber(key_expr(&vote_key)?)
            .await
            .map_err(|error| TransportError::Zenoh(error.to_string()))?;
        let transaction_subscriber = session
            .declare_subscriber(key_expr(&transaction_key)?)
            .await
            .map_err(|error| TransportError::Zenoh(error.to_string()))?;

        spawn_subscription(
            proposal_subscriber,
            sender.clone(),
            validator_index,
            MESSAGE_PROPOSAL,
        );
        spawn_subscription(
            vote_subscriber,
            sender.clone(),
            validator_index,
            MESSAGE_VOTE,
        );
        spawn_subscription(
            transaction_subscriber,
            sender,
            validator_index,
            MESSAGE_TRANSACTION,
        );

        Ok(Self {
            validator_index,
            chain_id,
            session,
            receiver,
        })
    }

    pub async fn open(
        validator_index: u32,
        chain_id: u32,
        listen_endpoint: &str,
        connect_endpoints: &[&str],
    ) -> Result<Self, TransportError> {
        if chain_id != GENESIS_CHAIN_ID {
            return Err(TransportError::InvalidPayload(format!(
                "non-canonical chain ID {chain_id}"
            )));
        }
        let mut config = ZenohConfig::default();
        config
            .insert_json5("mode", r#""peer""#)
            .map_err(|error| TransportError::Zenoh(format!("{error:?}")))?;
        config
            .insert_json5("listen/endpoints", &format!(r#"["{listen_endpoint}"]"#))
            .map_err(|error| TransportError::Zenoh(format!("{error:?}")))?;
        if !connect_endpoints.is_empty() {
            let endpoints = connect_endpoints
                .iter()
                .map(|endpoint| format!(r#""{endpoint}""#))
                .collect::<Vec<_>>()
                .join(",");
            config
                .insert_json5("connect/endpoints", &format!("[{endpoints}]"))
                .map_err(|error| TransportError::Zenoh(format!("{error:?}")))?;
        }
        config
            .insert_json5("scouting/multicast/enabled", "false")
            .map_err(|error| TransportError::Zenoh(format!("{error:?}")))?;
        let session = zenoh::open(config)
            .await
            .map_err(|error| TransportError::Zenoh(error.to_string()))?;
        Self::from_session(Arc::new(session), validator_index, chain_id).await
    }

    pub fn validator_index(&self) -> u32 {
        self.validator_index
    }

    pub fn chain_id(&self) -> u32 {
        self.chain_id
    }

    pub fn session(&self) -> Arc<Session> {
        Arc::clone(&self.session)
    }

    fn topic(&self, suffix: &str) -> Result<KeyExpr<'static>, TransportError> {
        let topic = format!("aurion/{}/{suffix}", self.chain_id);
        key_expr(&topic)
    }

    async fn publish(
        &self,
        topic: &str,
        message_type: u8,
        payload: Vec<u8>,
    ) -> Result<(), TransportError> {
        let mut frame = Vec::with_capacity(FRAME_PREFIX_BYTES + payload.len());
        frame.extend_from_slice(&self.validator_index.to_be_bytes());
        frame.push(message_type);
        frame.extend_from_slice(&payload);
        self.session
            .put(self.topic(topic)?, frame)
            .await
            .map_err(|error| TransportError::Zenoh(error.to_string()))?;
        Ok(())
    }
}

impl BftTransport for ZenohBftTransport {
    async fn broadcast_proposal(
        &self,
        proposal: BlockProposalEnvelope,
    ) -> Result<(), TransportError> {
        let payload = proposal.to_canonical_bytes();
        if payload.len() > MAX_PROPOSAL_WIRE_BYTES {
            return Err(TransportError::ProposalTooLarge {
                actual: payload.len(),
                max: MAX_PROPOSAL_WIRE_BYTES,
            });
        }
        self.publish(PROPOSAL_TOPIC, MESSAGE_PROPOSAL, payload)
            .await
    }

    async fn broadcast_vote(&self, vote: Vote) -> Result<(), TransportError> {
        let payload = vote.to_canonical_bytes();
        if payload.len() != VOTE_BYTES {
            return Err(TransportError::InvalidVoteSize {
                actual: payload.len(),
                expected: VOTE_BYTES,
            });
        }
        self.publish(VOTE_TOPIC, MESSAGE_VOTE, payload).await
    }

    async fn broadcast_transaction(
        &self,
        transaction: Transaction,
        sender_pubkey: [u8; 32],
    ) -> Result<(), TransportError> {
        let payload = transaction.to_canonical_bytes();
        if payload.len().saturating_add(32) > MAX_TRANSACTION_WIRE_BYTES {
            return Err(TransportError::TransactionTooLarge {
                actual: payload.len().saturating_add(32),
                max: MAX_TRANSACTION_WIRE_BYTES,
            });
        }
        let mut framed_payload = Vec::with_capacity(32 + payload.len());
        framed_payload.extend_from_slice(&sender_pubkey);
        framed_payload.extend_from_slice(&payload);
        self.publish(TRANSACTION_TOPIC, MESSAGE_TRANSACTION, framed_payload)
            .await
    }

    async fn recv(&mut self) -> Result<ConsensusMessage, TransportError> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| TransportError::ChannelClosed("Zenoh subscriber fan-in closed".into()))?
    }
}

fn key_expr(topic: &str) -> Result<KeyExpr<'static>, TransportError> {
    topic
        .to_string()
        .try_into()
        .map_err(|error| TransportError::InvalidPayload(format!("invalid topic: {error:?}")))
}

fn spawn_subscription(
    subscriber: zenoh::pubsub::Subscriber<FifoChannelHandler<Sample>>,
    sender: mpsc::Sender<Result<ConsensusMessage, TransportError>>,
    validator_index: u32,
    expected_type: u8,
) {
    tokio::spawn(async move {
        loop {
            let sample = match subscriber.recv_async().await {
                Ok(sample) => sample,
                Err(error) => {
                    let _ = sender
                        .send(Err(TransportError::Zenoh(error.to_string())))
                        .await;
                    break;
                }
            };
            let bytes = sample.payload().to_bytes();
            let result = decode_frame(&bytes, validator_index, expected_type);
            if let Ok(Some(message)) = result {
                if sender.send(Ok(message)).await.is_err() {
                    break;
                }
            } else if let Err(error) = result {
                if sender.send(Err(error)).await.is_err() {
                    break;
                }
            }
        }
    });
}

fn decode_frame(
    bytes: &[u8],
    local_validator_index: u32,
    expected_type: u8,
) -> Result<Option<ConsensusMessage>, TransportError> {
    if bytes.len() < FRAME_PREFIX_BYTES {
        return Err(TransportError::InvalidPayload("truncated frame".into()));
    }
    let sender = u32::from_be_bytes(bytes[..4].try_into().unwrap());
    if sender == local_validator_index {
        return Ok(None);
    }
    if bytes[4] != expected_type {
        return Err(TransportError::InvalidPayload(
            "message type/topic mismatch".into(),
        ));
    }
    let payload = &bytes[FRAME_PREFIX_BYTES..];
    let mut cursor = 0;
    let message = match expected_type {
        MESSAGE_PROPOSAL => {
            if payload.len() > MAX_PROPOSAL_WIRE_BYTES {
                return Err(TransportError::ProposalTooLarge {
                    actual: payload.len(),
                    max: MAX_PROPOSAL_WIRE_BYTES,
                });
            }
            ConsensusMessage::Proposal(
                BlockProposalEnvelope::decode_canonical(payload, &mut cursor)
                    .map_err(|error| TransportError::InvalidPayload(error.to_string()))?,
            )
        }
        MESSAGE_VOTE => {
            if payload.len() != VOTE_BYTES {
                return Err(TransportError::InvalidVoteSize {
                    actual: payload.len(),
                    expected: VOTE_BYTES,
                });
            }
            ConsensusMessage::Vote(
                Vote::decode_canonical(payload, &mut cursor)
                    .map_err(|error| TransportError::InvalidPayload(error.to_string()))?,
            )
        }
        MESSAGE_TRANSACTION => {
            if payload.len() > MAX_TRANSACTION_WIRE_BYTES {
                return Err(TransportError::TransactionTooLarge {
                    actual: payload.len(),
                    max: MAX_TRANSACTION_WIRE_BYTES,
                });
            }
            if payload.len() < 32 {
                return Err(TransportError::InvalidPayload("missing transaction sender public key".into()));
            }
            let sender_pubkey = payload[..32]
                .try_into()
                .map_err(|_| TransportError::InvalidPayload("invalid transaction sender public key".into()))?;
            cursor = 32;
            ConsensusMessage::Transaction {
                sender_pubkey,
                transaction: Transaction::decode_canonical(payload, &mut cursor)
                    .map_err(|error| TransportError::InvalidPayload(error.to_string()))?,
            }
        }
        _ => {
            return Err(TransportError::InvalidPayload(
                "unknown message type".into(),
            ))
        }
    };
    if cursor != payload.len() {
        return Err(TransportError::InvalidPayload(
            "trailing bytes in canonical payload".into(),
        ));
    }
    Ok(Some(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_vote_frame_is_rejected_before_decode() {
        let mut frame = Vec::from(1u32.to_be_bytes());
        frame.push(MESSAGE_VOTE);
        frame.extend_from_slice(&[0u8; VOTE_BYTES - 1]);
        let error = decode_frame(&frame, 0, MESSAGE_VOTE).unwrap_err();
        match error {
            TransportError::InvalidVoteSize { actual, expected } => {
                assert_eq!(actual, VOTE_BYTES - 1);
                assert_eq!(expected, VOTE_BYTES);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn oversized_proposal_frame_is_rejected_before_decode() {
        let mut frame = Vec::from(1u32.to_be_bytes());
        frame.push(MESSAGE_PROPOSAL);
        frame.extend(std::iter::repeat_n(0u8, MAX_PROPOSAL_WIRE_BYTES + 1));
        let error = decode_frame(&frame, 0, MESSAGE_PROPOSAL).unwrap_err();
        match error {
            TransportError::ProposalTooLarge { actual, max } => {
                assert_eq!(actual, MAX_PROPOSAL_WIRE_BYTES + 1);
                assert_eq!(max, MAX_PROPOSAL_WIRE_BYTES);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
