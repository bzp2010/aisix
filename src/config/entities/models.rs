use std::{collections::HashMap, sync::Arc};

use aisix_core::entities::{Model, models};
use aisix_core::entities::types::{HasRateLimit, RateLimit, RateLimitMetric};

use super::{EntityStore, IndexFn, ResourceEntry};
use crate::config::ConfigProvider;

pub static INDEX_FNS: &[IndexFn<Model>] = &[("by_name", |m: &Model| Some(m.name.clone()))];

impl HasRateLimit for ResourceEntry<Model> {
    fn rate_limit(&self) -> Option<RateLimit> {
        self.rate_limit.clone()
    }

    fn rate_limit_key(&self, metric: RateLimitMetric) -> String {
        format!("model:{}:{}", self.id, metric)
    }
}

#[derive(Clone)]
pub struct ModelsStore {
    store: EntityStore<Model>,
}

impl ModelsStore {
    pub async fn new(provider: Arc<dyn ConfigProvider + Send + Sync>) -> Self {
        Self {
            store: EntityStore::new(
                provider,
                "/models/",
                "models",
                Some(models::validate),
                INDEX_FNS,
            )
            .await,
        }
    }

    pub fn list(&self) -> Arc<HashMap<String, ResourceEntry<Model>>> {
        self.store.list()
    }

    pub fn get_by_name(&self, name: &str) -> Option<ResourceEntry<Model>> {
        self.store.get_by_secondary("by_name", name)
    }
}
