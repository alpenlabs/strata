use std::{collections::HashMap, sync::Arc};

use alpen_express_primitives::bridge::OperatorIdx;
use tokio::sync::RwLock;

#[derive(Default)]
pub(crate) struct OperatorBandwidth {
    bandwidth: Arc<RwLock<HashMap<OperatorIdx, u32>>>,
}

impl OperatorBandwidth {
    pub(crate) fn new() -> Self {
        Self {
            bandwidth: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(crate) async fn increment(&self, operator: OperatorIdx) {
        let mut bandwidth = self.bandwidth.write().await;
        *bandwidth.entry(operator).or_insert(0) += 1;
    }

    #[cfg(test)]
    pub(crate) async fn reset(&self, operator: OperatorIdx) {
        self.bandwidth.write().await.insert(operator, 0);
    }

    pub(crate) async fn get(&self, operator: OperatorIdx) -> u32 {
        *self.bandwidth.read().await.get(&operator).unwrap_or(&0)
    }

    pub(crate) async fn clear(&self) {
        self.bandwidth.write().await.clear();
    }
}
