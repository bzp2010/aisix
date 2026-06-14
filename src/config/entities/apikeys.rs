use std::{collections::HashMap, sync::Arc};

use aisix_core::entities::{ApiKey, apikeys};
use aisix_core::entities::types::{HasRateLimit, RateLimit, RateLimitMetric};

use super::{EntityStore, IndexFn, ResourceEntry};
use crate::config::ConfigProvider;

pub static INDEX_FNS: &[IndexFn<ApiKey>] = &[("by_key", |k: &ApiKey| Some(k.key.clone()))];

impl HasRateLimit for ResourceEntry<ApiKey> {
    fn rate_limit(&self) -> Option<RateLimit> {
        self.rate_limit.clone()
    }

    fn rate_limit_key(&self, metric: RateLimitMetric) -> String {
        format!("apikey:{}:{}", self.id, metric)
    }
}

#[derive(Clone)]
pub struct ApiKeysStore {
    store: EntityStore<ApiKey>,
}

impl ApiKeysStore {
    pub async fn new(provider: Arc<dyn ConfigProvider + Send + Sync>) -> Self {
        Self {
            store: EntityStore::new(
                provider,
                "/apikeys/",
                "apikeys",
                Some(apikeys::validate),
                INDEX_FNS,
            )
            .await,
        }
    }

    pub fn list(&self) -> Arc<HashMap<String, ResourceEntry<ApiKey>>> {
        self.store.list()
    }

    pub fn get_by_key(&self, api_key: &str) -> Option<ResourceEntry<ApiKey>> {
        self.store.get_by_secondary("by_key", api_key)
    }
}
