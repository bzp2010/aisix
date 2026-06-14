use std::{collections::HashMap, sync::Arc};

use aisix_core::entities::{Policy, policies};

use super::{EntityStore, IndexFn, ResourceEntry};
use crate::ConfigProvider;

pub static INDEX_FNS: &[IndexFn<Policy>] =
    &[("by_name", |policy: &Policy| Some(policy.name.clone()))];

#[derive(Clone)]
pub struct PoliciesStore {
    store: EntityStore<Policy>,
}

impl PoliciesStore {
    pub async fn new(provider: Arc<dyn ConfigProvider + Send + Sync>) -> Self {
        Self {
            store: EntityStore::new(
                provider,
                "/policies/",
                "policies",
                Some(policies::validate_policy_definition),
                INDEX_FNS,
            )
            .await,
        }
    }

    pub fn list(&self) -> Arc<HashMap<String, ResourceEntry<Policy>>> {
        self.store.list()
    }

    pub fn get_by_id(&self, id: &str) -> Option<ResourceEntry<Policy>> {
        self.store.get(id)
    }

    pub fn get_by_name(&self, name: &str) -> Option<ResourceEntry<Policy>> {
        self.store.get_by_secondary("by_name", name)
    }
}
