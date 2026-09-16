//! # Aurion L3 Specialized Domain: Microsecond Order-Book DEX
//!
//! High-throughput in-memory deterministic order matching engine (Price-Time Priority / FIFO).
//! Designed for specialized microsecond DeFi execution domains with periodic batch settlement.
//!
//! Conforms strictly to:
//! - AUR-ARCH-011: Zero unsafe code (`#![forbid(unsafe_code)]`)
//! - AUR-ARCH-012: Zero floating-point arithmetic (All values in `Quantum(u128)`)
//! - AUR-L3-ARCH-002: Specialized domain execution & batch settlement

use std::cmp::Reverse;
use std::collections::{BTreeMap, VecDeque};
use blake3::Hasher;

use crate::primitives::core::Quantum;
use crate::specialized::types::DomainId;

/// Direction of the order in the order book.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    /// Bid to buy base asset with quote asset.
    Buy,
    /// Ask to sell base asset for quote asset.
    Sell,
}

/// Order type specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    /// Limit order with exact maximum buy or minimum sell price.
    Limit,
    /// Market order executing immediately at best available prices.
    Market,
}

/// An order submitted to the specialized DEX order book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    /// Unique incremental identifier.
    pub id: u64,
    /// Trader public key or account identifier.
    pub trader: [u8; 32],
    /// Buy or Sell side.
    pub side: OrderSide,
    /// Order type.
    pub order_type: OrderType,
    /// Limit price per unit in Quantum (0 for market orders).
    pub price: Quantum,
    /// Total base quantity ordered in Quantum.
    pub quantity: Quantum,
    /// Base quantity filled so far in Quantum.
    pub filled: Quantum,
    /// Monotonic timestamp or sequence number.
    pub sequence: u64,
}

impl Order {
    /// Remaining unfilled base quantity.
    pub fn remaining(&self) -> Quantum {
        self.quantity
            .checked_sub(self.filled)
            .unwrap_or(Quantum(0))
    }

    /// Whether the order is completely fulfilled.
    pub fn is_filled(&self) -> bool {
        self.filled >= self.quantity
    }
}

/// Record of an executed trade match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trade {
    /// ID of the maker resting order.
    pub maker_order_id: u64,
    /// ID of the taker aggressive order.
    pub taker_order_id: u64,
    /// Trader public key of the maker.
    pub maker: [u8; 32],
    /// Trader public key of the taker.
    pub taker: [u8; 32],
    /// Execution price in Quantum.
    pub price: Quantum,
    /// Executed base quantity in Quantum.
    pub quantity: Quantum,
    /// Side of the maker.
    pub maker_side: OrderSide,
    /// Trade execution sequence.
    pub trade_id: u64,
}

/// Specialized DEX operational errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DexError {
    /// Order quantity must be greater than zero.
    ZeroQuantity,
    /// Limit orders must specify a price greater than zero.
    ZeroLimitPrice,
    /// Order not found in book for cancellation.
    OrderNotFound(u64),
    /// Market order could not find any liquidity.
    InsufficientLiquidity,
    /// Integer arithmetic overflow during quantity or price calculation.
    ArithmeticOverflow,
}

/// In-memory deterministic Price-Time Priority order book matching engine.
#[derive(Debug)]
pub struct OrderBook {
    /// Parent specialized domain ID.
    pub domain_id: DomainId,
    /// Trading pair symbol identifier (e.g., [b'A', b'U', b'R', b'/', b'U', b'S', b'D', 0]).
    pub pair: [u8; 8],
    /// Bids: descending price order -> FIFO queue of resting limit orders.
    bids: BTreeMap<Reverse<u128>, VecDeque<Order>>,
    /// Asks: ascending price order -> FIFO queue of resting limit orders.
    asks: BTreeMap<u128, VecDeque<Order>>,
    /// Monotonic counter for next order ID.
    next_order_id: u64,
    /// Monotonic counter for executed trades.
    next_trade_id: u64,
    /// Buffer of executed trades pending batch checkpointing.
    executed_trades: Vec<Trade>,
}

impl OrderBook {
    /// Instantiates a new empty order book for a given trading pair within a domain.
    pub fn new(domain_id: DomainId, pair: [u8; 8]) -> Self {
        Self {
            domain_id,
            pair,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            next_order_id: 1,
            next_trade_id: 1,
            executed_trades: Vec::new(),
        }
    }

    /// Places and matches an order against resting liquidity.
    /// Returns the list of resulting trades executed by this order.
    pub fn place_order(
        &mut self,
        trader: [u8; 32],
        side: OrderSide,
        order_type: OrderType,
        price: Quantum,
        quantity: Quantum,
    ) -> Result<Vec<Trade>, DexError> {
        if quantity == Quantum(0) {
            return Err(DexError::ZeroQuantity);
        }
        if order_type == OrderType::Limit && price == Quantum(0) {
            return Err(DexError::ZeroLimitPrice);
        }

        let order_id = self.next_order_id;
        self.next_order_id = self.next_order_id.saturating_add(1);

        let mut incoming = Order {
            id: order_id,
            trader,
            side,
            order_type,
            price,
            quantity,
            filled: Quantum(0),
            sequence: order_id,
        };

        let mut trades = Vec::new();

        match side {
            OrderSide::Buy => {
                self.match_buy_order(&mut incoming, &mut trades)?;
                if !incoming.is_filled() && incoming.order_type == OrderType::Limit {
                    let level = self.bids.entry(Reverse(incoming.price.as_u128())).or_default();
                    level.push_back(incoming);
                }
            }
            OrderSide::Sell => {
                self.match_sell_order(&mut incoming, &mut trades)?;
                if !incoming.is_filled() && incoming.order_type == OrderType::Limit {
                    let level = self.asks.entry(incoming.price.as_u128()).or_default();
                    level.push_back(incoming);
                }
            }
        }

        self.executed_trades.extend_from_slice(&trades);
        Ok(trades)
    }

    /// Cancels a resting limit order by ID.
    pub fn cancel_order(&mut self, order_id: u64) -> Result<Order, DexError> {
        let mut found = None;
        let mut empty_bid_key = None;
        for (&key, level) in self.bids.iter_mut() {
            if let Some(pos) = level.iter().position(|o| o.id == order_id) {
                let order = level.remove(pos).expect("position checked");
                if level.is_empty() {
                    empty_bid_key = Some(key);
                }
                found = Some(order);
                break;
            }
        }
        if let Some(key) = empty_bid_key {
            self.bids.remove(&key);
        }
        if let Some(order) = found {
            return Ok(order);
        }

        let mut empty_ask_key = None;
        for (&key, level) in self.asks.iter_mut() {
            if let Some(pos) = level.iter().position(|o| o.id == order_id) {
                let order = level.remove(pos).expect("position checked");
                if level.is_empty() {
                    empty_ask_key = Some(key);
                }
                found = Some(order);
                break;
            }
        }
        if let Some(key) = empty_ask_key {
            self.asks.remove(&key);
        }
        if let Some(order) = found {
            return Ok(order);
        }

        Err(DexError::OrderNotFound(order_id))
    }

    /// Matches an incoming buy order against resting asks (lowest price first).
    fn match_buy_order(
        &mut self,
        incoming: &mut Order,
        trades: &mut Vec<Trade>,
    ) -> Result<(), DexError> {
        let mut prices_to_remove = Vec::new();

        for (&ask_price_val, queue) in self.asks.iter_mut() {
            let ask_price = Quantum(ask_price_val);
            if incoming.order_type == OrderType::Limit && ask_price > incoming.price {
                break; // No more matching asks within limit price
            }

            while let Some(maker) = queue.front_mut() {
                let trade_qty = std::cmp::min(incoming.remaining(), maker.remaining());
                if trade_qty == Quantum(0) {
                    break;
                }

                let trade_id = self.next_trade_id;
                self.next_trade_id = self.next_trade_id.saturating_add(1);

                let trade = Trade {
                    maker_order_id: maker.id,
                    taker_order_id: incoming.id,
                    maker: maker.trader,
                    taker: incoming.trader,
                    price: ask_price, // Maker price priority
                    quantity: trade_qty,
                    maker_side: OrderSide::Sell,
                    trade_id,
                };
                trades.push(trade);

                incoming.filled = incoming
                    .filled
                    .checked_add(trade_qty)
                    .map_err(|_| DexError::ArithmeticOverflow)?;
                maker.filled = maker
                    .filled
                    .checked_add(trade_qty)
                    .map_err(|_| DexError::ArithmeticOverflow)?;

                if maker.is_filled() {
                    queue.pop_front();
                }

                if incoming.is_filled() {
                    break;
                }
            }

            if queue.is_empty() {
                prices_to_remove.push(ask_price_val);
            }

            if incoming.is_filled() {
                break;
            }
        }

        for p in prices_to_remove {
            self.asks.remove(&p);
        }

        Ok(())
    }

    /// Matches an incoming sell order against resting bids (highest price first).
    fn match_sell_order(
        &mut self,
        incoming: &mut Order,
        trades: &mut Vec<Trade>,
    ) -> Result<(), DexError> {
        let mut prices_to_remove = Vec::new();

        for (Reverse(bid_price_val), queue) in self.bids.iter_mut() {
            let bid_price = Quantum(*bid_price_val);
            if incoming.order_type == OrderType::Limit && bid_price < incoming.price {
                break; // No more matching bids within limit price
            }

            while let Some(maker) = queue.front_mut() {
                let trade_qty = std::cmp::min(incoming.remaining(), maker.remaining());
                if trade_qty == Quantum(0) {
                    break;
                }

                let trade_id = self.next_trade_id;
                self.next_trade_id = self.next_trade_id.saturating_add(1);

                let trade = Trade {
                    maker_order_id: maker.id,
                    taker_order_id: incoming.id,
                    maker: maker.trader,
                    taker: incoming.trader,
                    price: bid_price, // Maker price priority
                    quantity: trade_qty,
                    maker_side: OrderSide::Buy,
                    trade_id,
                };
                trades.push(trade);

                incoming.filled = incoming
                    .filled
                    .checked_add(trade_qty)
                    .map_err(|_| DexError::ArithmeticOverflow)?;
                maker.filled = maker
                    .filled
                    .checked_add(trade_qty)
                    .map_err(|_| DexError::ArithmeticOverflow)?;

                if maker.is_filled() {
                    queue.pop_front();
                }

                if incoming.is_filled() {
                    break;
                }
            }

            if queue.is_empty() {
                prices_to_remove.push(Reverse(*bid_price_val));
            }

            if incoming.is_filled() {
                break;
            }
        }

        for p in prices_to_remove {
            self.bids.remove(&p);
        }

        Ok(())
    }

    /// Best current bid price, if any.
    pub fn best_bid(&self) -> Option<Quantum> {
        self.bids
            .iter()
            .find(|(_, q)| !q.is_empty())
            .map(|(Reverse(p), _)| Quantum(*p))
    }

    /// Best current ask price, if any.
    pub fn best_ask(&self) -> Option<Quantum> {
        self.asks
            .iter()
            .find(|(_, q)| !q.is_empty())
            .map(|(&p, _)| Quantum(p))
    }

    /// Drains executed trades buffer for batching and checkpointing.
    pub fn drain_executed_batch(&mut self) -> Vec<Trade> {
        std::mem::take(&mut self.executed_trades)
    }

    /// Computes a deterministic Blake3 commitment root of a trade batch for checkpointing.
    pub fn compute_trade_batch_root(trades: &[Trade]) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION_L3_DEX_TRADE_BATCH_V1");
        hasher.update(&(trades.len() as u64).to_be_bytes());

        for t in trades {
            hasher.update(&t.maker_order_id.to_be_bytes());
            hasher.update(&t.taker_order_id.to_be_bytes());
            hasher.update(&t.maker);
            hasher.update(&t.taker);
            hasher.update(&t.price.as_u128().to_be_bytes());
            hasher.update(&t.quantity.as_u128().to_be_bytes());
            hasher.update(&[t.maker_side as u8]);
            hasher.update(&t.trade_id.to_be_bytes());
        }

        *hasher.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dex_order_matching_fifo_and_price_priority() {
        let domain_id = DomainId::named("dex-ultra-fast");
        let pair = *b"AUR/USDT";
        let mut book = OrderBook::new(domain_id, pair);

        let maker1 = [1u8; 32];
        let maker2 = [2u8; 32];
        let taker = [3u8; 32];

        // Resting Ask 1: Sell 10 @ 100
        let trades1 = book
            .place_order(maker1, OrderSide::Sell, OrderType::Limit, Quantum(100), Quantum(10))
            .expect("place ask 1");
        assert!(trades1.is_empty());
        assert_eq!(book.best_ask(), Some(Quantum(100)));

        // Resting Ask 2: Sell 20 @ 105
        let trades2 = book
            .place_order(maker2, OrderSide::Sell, OrderType::Limit, Quantum(105), Quantum(20))
            .expect("place ask 2");
        assert!(trades2.is_empty());

        // Aggressive Bid: Buy 15 @ 105 (Crosses ask 1 completely, and fills 5 of ask 2)
        let matched = book
            .place_order(taker, OrderSide::Buy, OrderType::Limit, Quantum(105), Quantum(15))
            .expect("place aggressive bid");

        assert_eq!(matched.len(), 2);
        // Trade 1: 10 @ 100 from maker1
        assert_eq!(matched[0].maker, maker1);
        assert_eq!(matched[0].price, Quantum(100));
        assert_eq!(matched[0].quantity, Quantum(10));

        // Trade 2: 5 @ 105 from maker2
        assert_eq!(matched[1].maker, maker2);
        assert_eq!(matched[1].price, Quantum(105));
        assert_eq!(matched[1].quantity, Quantum(5));

        // Best ask should now still be 105 with remaining 15
        assert_eq!(book.best_ask(), Some(Quantum(105)));

        // Verify batch root generation
        let batch = book.drain_executed_batch();
        assert_eq!(batch.len(), 2);
        let batch_root = OrderBook::compute_trade_batch_root(&batch);
        assert_ne!(batch_root, [0u8; 32]);
    }

    #[test]
    fn test_dex_order_cancellation() {
        let domain_id = DomainId::named("dex-ultra-fast");
        let pair = *b"AUR/USDT";
        let mut book = OrderBook::new(domain_id, pair);

        let trader = [4u8; 32];
        book.place_order(trader, OrderSide::Buy, OrderType::Limit, Quantum(50), Quantum(100))
            .expect("place bid");
        assert_eq!(book.best_bid(), Some(Quantum(50)));

        let cancelled = book.cancel_order(1).expect("cancel bid");
        assert_eq!(cancelled.id, 1);
        assert_eq!(book.best_bid(), None);

        let err = book.cancel_order(999);
        assert_eq!(err, Err(DexError::OrderNotFound(999)));
    }
}
