use std::{collections::HashMap, sync::Arc};

use aisix_core::entities::{Guardrail, guardrails};

use super::{EntityStore, ResourceEntry};
use crate::ConfigProvider;

#[derive(Clone)]
pub struct GuardrailsStore {
    store: EntityStore<Guardrail>,
}

impl GuardrailsStore {
    pub async fn new(provider: Arc<dyn ConfigProvider + Send + Sync>) -> Self {
        Self {
            store: EntityStore::new(
                provider,
                "/guardrails/",
                "guardrails",
                Some(guardrails::validate_guardrail_definition),
                &[],
            )
            .await,
        }
    }

    pub fn list(&self) -> Arc<HashMap<String, ResourceEntry<Guardrail>>> {
        self.store.list()
    }

    pub fn get_by_id(&self, id: &str) -> Option<ResourceEntry<Guardrail>> {
        self.store.get(id)
    }
}
