use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use crate::{Faults, MockSutConfig};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrderStatus {
    Pending,
    Paid,
}

#[derive(Clone, Debug)]
pub struct Order {
    pub status: OrderStatus,
}

#[derive(Clone)]
pub struct SharedState {
    inner: Arc<RwLock<Inner>>,
    pub config: MockSutConfig,
}

struct Inner {
    orders: BTreeMap<String, Order>,
    next_seq: u32,
}

impl SharedState {
    pub fn new(config: MockSutConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                orders: BTreeMap::new(),
                next_seq: 1,
            })),
            config,
        }
    }

    pub async fn create_order(&self) -> String {
        let mut guard = self.inner.write().await;
        let id = format!("o-{}", guard.next_seq);
        guard.next_seq += 1;
        guard.orders.insert(
            id.clone(),
            Order {
                status: OrderStatus::Pending,
            },
        );
        id
    }

    pub async fn get_order(&self, id: &str) -> Option<Order> {
        self.inner.read().await.orders.get(id).cloned()
    }

    pub async fn mark_paid_later(&self, id: String) {
        let delay = self.jittered_pay_delay(&id);
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            let mut guard = inner.write().await;
            if let Some(order) = guard.orders.get_mut(&id) {
                order.status = OrderStatus::Paid;
            }
        });
    }

    pub fn faults(&self) -> &Faults {
        &self.config.faults
    }

    pub(crate) fn jittered_pay_delay(&self, order_id: &str) -> Duration {
        let base = self.config.pay_delay;
        let jitter_cap = self.config.jitter_ms;
        if jitter_cap == 0 {
            return base;
        }
        let mut hash = self.config.seed;
        for byte in order_id.as_bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(u64::from(*byte));
        }
        let extra = hash % (jitter_cap + 1);
        base + Duration::from_millis(extra)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jittered_pay_delay_is_seed_and_order_stable() {
        let config = MockSutConfig {
            pay_delay: Duration::from_millis(5),
            jitter_ms: 200,
            seed: 11,
            ..MockSutConfig::default()
        };
        let state = SharedState::new(config);
        let a = state.jittered_pay_delay("o-1");
        let b = state.jittered_pay_delay("o-1");
        assert_eq!(a, b);
        assert!(a >= Duration::from_millis(5));
        assert!(a <= Duration::from_millis(205));
    }
}
