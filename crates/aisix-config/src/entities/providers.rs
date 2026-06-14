use std::{collections::HashMap, sync::Arc};

use aisix_core::entities::{Provider, providers};

use super::{EntityStore, ResourceEntry};
use crate::ConfigProvider;

#[derive(Clone)]
pub struct ProvidersStore {
    store: EntityStore<Provider>,
}

impl ProvidersStore {
    pub async fn new(provider: Arc<dyn ConfigProvider + Send + Sync>) -> Self {
        Self {
            store: EntityStore::new(
                provider,
                "/providers/",
                "providers",
                Some(providers::validate),
                &[],
            )
            .await,
        }
    }

    pub fn list(&self) -> Arc<HashMap<String, ResourceEntry<Provider>>> {
        self.store.list()
    }

    pub fn get_by_id(&self, id: &str) -> Option<ResourceEntry<Provider>> {
        self.store.get(id)
    }
}
