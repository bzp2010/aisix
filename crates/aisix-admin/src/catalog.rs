use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const MODELS_DEV_URL: &str = "https://models.dev/api.json";
const REFRESH_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Deserialize)]
struct RawProviderEntry {
    id: String,
    name: String,
    #[serde(default)]
    api: Option<String>,
    #[serde(default)]
    doc: Option<String>,
    #[serde(default)]
    models: HashMap<String, RawModelEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawModelEntry {
    id: String,
    name: String,
}

struct CatalogData {
    providers: HashMap<String, RawProviderEntry>,
}

#[derive(Clone, Serialize, ToSchema)]
pub struct ProviderSummary {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

#[derive(Clone, Serialize, ToSchema)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
}

pub struct CatalogCache {
    data: Arc<ArcSwap<Option<CatalogData>>>,
    http_client: reqwest::Client,
}

impl CatalogCache {
    pub fn new() -> Arc<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");
        let cache = Arc::new(Self {
            data: Arc::new(ArcSwap::new(Arc::new(None))),
            http_client,
        });

        let cache_clone = cache.clone();
        tokio::spawn(async move {
            cache_clone.fetch_and_store().await;
            loop {
                tokio::time::sleep(REFRESH_INTERVAL).await;
                cache_clone.fetch_and_store().await;
            }
        });

        cache
    }

    async fn fetch_and_store(&self) {
        match self.http_client.get(MODELS_DEV_URL).send().await {
            Ok(resp) => {
                match resp.json::<HashMap<String, RawProviderEntry>>().await {
                    Ok(providers) => {
                        let count = providers.len();
                        self.data.store(Arc::new(Some(CatalogData { providers })));
                        info!("models.dev catalog refreshed: {} providers", count);
                    }
                    Err(e) => warn!("Failed to parse models.dev catalog: {}", e),
                }
            }
            Err(e) => warn!("Failed to fetch models.dev catalog: {}", e),
        }
    }

    pub fn list_providers(&self) -> Vec<ProviderSummary> {
        let data = self.data.load();
        let Some(d) = data.as_ref().as_ref() else {
            return vec![];
        };
        let mut providers: Vec<_> = d
            .providers
            .values()
            .map(|p| ProviderSummary {
                id: p.id.clone(),
                name: p.name.clone(),
                api: p.api.clone(),
                doc: p.doc.clone(),
            })
            .collect();
        providers.sort_by(|a, b| a.id.cmp(&b.id));
        providers
    }

    pub fn get_provider_models(&self, id: &str) -> Option<Vec<ModelEntry>> {
        let data = self.data.load();
        let d = data.as_ref().as_ref()?;
        let provider = d.providers.get(id)?;
        let mut models: Vec<_> = provider
            .models
            .values()
            .map(|m| ModelEntry {
                id: m.id.clone(),
                name: m.name.clone(),
            })
            .collect();
        models.sort_by(|a, b| a.id.cmp(&b.id));
        Some(models)
    }

    pub async fn refresh(&self) {
        self.fetch_and_store().await;
    }
}
