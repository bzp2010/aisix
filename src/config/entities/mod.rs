pub mod apikeys;
pub mod guardrails;
pub mod models;
pub mod policies;
pub mod providers;

use std::{collections::HashMap, sync::Arc};

pub use apikeys::ApiKeysStore;
pub use guardrails::GuardrailsStore;
pub use models::ModelsStore;
pub use policies::PoliciesStore;
pub use providers::ProvidersStore;

use std::ops::Deref;

use arc_swap::ArcSwap;
use log::{info, warn};
use serde::de::DeserializeOwned;
use tokio::sync::mpsc::Receiver;

use crate::config::{ConfigEvent, ConfigProvider, GetEntry};

#[derive(Clone, Debug)]
pub struct ResourceEntry<T> {
    pub id: String,
    value: T,
    #[allow(dead_code)]
    revision: i64,
}

impl<T> ResourceEntry<T> {
    pub fn new(id: String, value: T, revision: i64) -> Self {
        Self { id, value, revision }
    }

    #[allow(dead_code)]
    pub fn revision(&self) -> i64 {
        self.revision
    }
}

impl<T> Deref for ResourceEntry<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[derive(Clone)]
pub struct ResourceRegistry {
    pub models: ModelsStore,
    pub apikeys: ApiKeysStore,
    pub guardrails: GuardrailsStore,
    pub policies: PoliciesStore,
    pub providers: ProvidersStore,
}

impl ResourceRegistry {
    pub async fn new(provider: Arc<dyn ConfigProvider + Send + Sync>) -> Self {
        let providers = ProvidersStore::new(provider.clone()).await;
        let guardrails = GuardrailsStore::new(provider.clone()).await;
        let policies = PoliciesStore::new(provider.clone()).await;
        let models = ModelsStore::new(provider.clone()).await;
        let apikeys = ApiKeysStore::new(provider).await;

        Self {
            models,
            apikeys,
            guardrails,
            policies,
            providers,
        }
    }
}

pub type IndexFn<T> = (&'static str, fn(&T) -> Option<String>);
pub type EntityValidator<T> = fn(&str, &T) -> Result<(), String>;

struct StoreData<T> {
    primary: Arc<HashMap<String, ResourceEntry<T>>>,
    secondary: HashMap<&'static str, HashMap<String, String>>,
    mod_revision: i64,
}

impl<T: Clone> StoreData<T> {
    fn empty(index_fns: &'static [IndexFn<T>]) -> Self {
        let secondary = index_fns
            .iter()
            .map(|(name, _)| (*name, HashMap::new()))
            .collect();
        Self {
            primary: Arc::new(HashMap::new()),
            secondary,
            mod_revision: 0,
        }
    }
}

#[derive(Clone)]
struct ResourceStore<T: 'static> {
    data: Arc<ArcSwap<StoreData<T>>>,
    index_fns: &'static [IndexFn<T>],
}

impl<T: Clone + 'static> ResourceStore<T> {
    fn new(index_fns: &'static [IndexFn<T>]) -> Self {
        Self {
            data: Arc::new(ArcSwap::from_pointee(StoreData::empty(index_fns))),
            index_fns,
        }
    }

    fn upsert(&self, key: String, value: T, revision: i64) {
        let current = self.data.load();

        let mut new_primary = (*current.primary).clone();
        let mut new_secondary = current.secondary.clone();

        for (name, key_fn) in self.index_fns {
            let index = new_secondary.entry(name).or_default();
            if let Some(old_entry) = new_primary.get(&key)
                && let Some(old_sec_key) = key_fn(old_entry)
            {
                index.remove(&old_sec_key);
            }
            if let Some(new_sec_key) = key_fn(&value) {
                index.insert(new_sec_key, key.clone());
            }
        }

        new_primary.insert(key.clone(), ResourceEntry::new(key, value, revision));

        self.data.store(Arc::new(StoreData {
            primary: Arc::new(new_primary),
            secondary: new_secondary,
            mod_revision: revision.max(current.mod_revision),
        }));
    }

    fn delete(&self, key: &str, mod_revision: i64) -> bool {
        let current = self.data.load();

        let mut new_primary = (*current.primary).clone();
        let mut new_secondary = current.secondary.clone();

        if let Some(old_entry) = new_primary.get(key) {
            for (name, key_fn) in self.index_fns {
                if let Some(old_sec_key) = key_fn(old_entry)
                    && let Some(index) = new_secondary.get_mut(name)
                {
                    index.remove(&old_sec_key);
                }
            }
        }

        let deleted = new_primary.remove(key).is_some();

        self.data.store(Arc::new(StoreData {
            primary: Arc::new(new_primary),
            secondary: new_secondary,
            mod_revision: mod_revision.max(current.mod_revision),
        }));

        deleted
    }

    fn get(&self, key: &str) -> Option<ResourceEntry<T>> {
        self.data.load().primary.get(key).cloned()
    }

    fn get_by_secondary(&self, index: &str, sec_key: &str) -> Option<ResourceEntry<T>> {
        let snapshot = self.data.load();
        let primary_key = snapshot.secondary.get(index)?.get(sec_key)?;
        snapshot.primary.get(primary_key).cloned()
    }

    fn primary_snapshot(&self) -> Arc<HashMap<String, ResourceEntry<T>>> {
        Arc::clone(&self.data.load().primary)
    }

    #[cfg(test)]
    fn latest_mod_revision(&self) -> i64 {
        self.data.load().mod_revision
    }

    fn replace_from_entries(&self, entries: Vec<(String, T, i64)>) {
        let mut new_primary: HashMap<String, ResourceEntry<T>> =
            HashMap::with_capacity(entries.len());
        let mut new_secondary: HashMap<&'static str, HashMap<String, String>> = self
            .index_fns
            .iter()
            .map(|(name, _)| (*name, HashMap::new()))
            .collect();
        let mut max_rev = 0i64;

        for (key, value, revision) in entries {
            for (name, key_fn) in self.index_fns {
                if let Some(sec_key) = key_fn(&value) {
                    new_secondary
                        .entry(name)
                        .or_default()
                        .insert(sec_key, key.clone());
                }
            }
            new_primary.insert(key.clone(), ResourceEntry::new(key, value, revision));
            max_rev = max_rev.max(revision);
        }

        self.data.store(Arc::new(StoreData {
            primary: Arc::new(new_primary),
            secondary: new_secondary,
            mod_revision: max_rev,
        }));
    }
}

#[derive(Clone)]
pub struct EntityStore<T: 'static> {
    store: ResourceStore<T>,
}

impl<T: DeserializeOwned + Clone + Send + Sync + 'static> EntityStore<T> {
    pub async fn new(
        provider: Arc<dyn ConfigProvider>,
        prefix: &str,
        entity_name: &str,
        validator: Option<EntityValidator<T>>,
        index_fns: &'static [IndexFn<T>],
    ) -> Self {
        let store = ResourceStore::new(index_fns);

        info!("{} starting full load, prefix={}", entity_name, prefix);
        match provider.get_all::<T>(prefix).await {
            Ok(kvs) => {
                Self::apply_full_load(&store, kvs, prefix, entity_name, validator);
                info!("{} full load completed", entity_name);
            }
            Err(err) => {
                warn!(
                    "{} full load failed: {}, will only rely on watch events",
                    entity_name, err
                );
            }
        }

        match provider.watch(Some(prefix)).await {
            Ok(mut rx) => {
                let store_clone = store.clone();
                let entity_name = entity_name.to_string();
                let prefix = prefix.to_string();
                let provider_clone = provider.clone();

                tokio::spawn(async move {
                    Self::consume_events(
                        store_clone,
                        &mut rx,
                        &entity_name,
                        &prefix,
                        validator,
                        provider_clone,
                    )
                    .await;
                });
            }
            Err(_) => {
                warn!("Duplicate registration of {entity_name} prefix watch ignored: {prefix}");
            }
        }

        Self { store }
    }

    pub fn list(&self) -> Arc<HashMap<String, ResourceEntry<T>>> {
        self.store.primary_snapshot()
    }

    pub fn get(&self, key: &str) -> Option<ResourceEntry<T>> {
        self.store.get(key)
    }

    pub fn get_by_secondary(&self, index: &str, sec_key: &str) -> Option<ResourceEntry<T>> {
        self.store.get_by_secondary(index, sec_key)
    }

    #[cfg(test)]
    pub fn latest_mod_revision(&self) -> i64 {
        self.store.latest_mod_revision()
    }

    fn relative_key(key: &str, prefix: &str) -> String {
        let base_prefix = if let Some(idx) = key.find(prefix) {
            &key[..idx + prefix.len()]
        } else {
            key
        };
        key.strip_prefix(base_prefix)
            .unwrap_or(key)
            .trim_start_matches('/')
            .to_string()
    }

    fn apply_full_load(
        store: &ResourceStore<T>,
        kvs: Vec<GetEntry<T>>,
        prefix: &str,
        entity_name: &str,
        validator: Option<EntityValidator<T>>,
    ) -> i64 {
        let mut entries: Vec<(String, T, i64)> = Vec::with_capacity(kvs.len());
        for GetEntry {
            key,
            value,
            create_revision: _,
            mod_revision,
        } in kvs
        {
            let relative_key = Self::relative_key(&key, prefix);
            let mut skip = false;
            if let Some(ref v) = validator
                && let Err(err) = v(&relative_key, &value)
            {
                warn!(
                    "{} validation failed, key={}: {}",
                    entity_name, relative_key, err
                );
                skip = true;
            }
            if !skip {
                entries.push((relative_key, value, mod_revision));
            }
        }
        let max_rev = entries.iter().map(|(_, _, r)| *r).max().unwrap_or(0);
        store.replace_from_entries(entries);
        max_rev
    }

    async fn consume_events(
        store: ResourceStore<T>,
        rx: &mut Receiver<ConfigEvent>,
        entity_name: &str,
        prefix: &str,
        validator: Option<EntityValidator<T>>,
        provider: Arc<dyn ConfigProvider>,
    ) {
        info!("{} watch started, prefix={}", entity_name, prefix);

        while let Some(event) = rx.recv().await {
            match event {
                ConfigEvent::Put((key, value, mod_revision)) => {
                    let relative_key = Self::relative_key(&key, prefix);

                    match serde_json::from_slice::<T>(&value) {
                        Ok(parsed) => {
                            if let Some(ref v) = validator {
                                match v(&relative_key, &parsed) {
                                    Ok(_) => {
                                        store.upsert(relative_key.clone(), parsed, mod_revision);
                                    }
                                    Err(err) => {
                                        warn!(
                                            "{} validation failed, key={}: {}",
                                            entity_name, relative_key, err
                                        );
                                    }
                                }
                            } else {
                                store.upsert(relative_key.clone(), parsed, mod_revision);
                            }
                        }
                        Err(err) => {
                            warn!(
                                "{} JSON parsing failed, key={}: {}",
                                entity_name, relative_key, err
                            );
                        }
                    }
                }
                ConfigEvent::Delete((key, mod_revision)) => {
                    let relative_key = Self::relative_key(&key, prefix);

                    if !store.delete(relative_key.as_str(), mod_revision) {
                        info!(
                            "{} Delete event missed cache, key={}",
                            entity_name, relative_key
                        );
                    }
                }
                ConfigEvent::Resync => {
                    warn!(
                        "{} Resync requested, performing full reload, prefix={}",
                        entity_name, prefix
                    );
                    match provider.get_all::<T>(prefix).await {
                        Ok(kvs) => {
                            let max_rev =
                                Self::apply_full_load(&store, kvs, prefix, entity_name, validator);
                            info!(
                                "{} full resync completed (max_rev={})",
                                entity_name, max_rev
                            );
                        }
                        Err(err) => {
                            warn!(
                                "{} full resync failed: {}, retaining stale snapshot",
                                entity_name, err
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Mutex, time::Duration};

    use anyhow::Result;
    use async_trait::async_trait;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc;

    use super::*;
    use crate::config::{ConfigEvent, ConfigEventReceiver, GetEntry, PutEntry};

    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Item {
        name: String,
        tag: String,
    }

    fn item(name: &str, tag: &str) -> Item {
        Item {
            name: name.into(),
            tag: tag.into(),
        }
    }

    static ITEM_INDEXES: &[IndexFn<Item>] = &[
        ("by_name", |i: &Item| Some(i.name.clone())),
        ("by_tag", |i: &Item| Some(i.tag.clone())),
    ];

    struct MockProvider {
        data: Vec<(String, Vec<u8>)>,
        watch_rx: Mutex<Option<ConfigEventReceiver>>,
    }

    impl MockProvider {
        fn new(data: Vec<(&str, Vec<u8>)>, rx: ConfigEventReceiver) -> Arc<Self> {
            Arc::new(Self {
                data: data.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
                watch_rx: Mutex::new(Some(rx)),
            })
        }
    }

    #[async_trait]
    impl crate::config::ConfigProvider for MockProvider {
        async fn get_all_raw(
            &self,
            _prefix: Option<&str>,
        ) -> Result<Vec<GetEntry<Vec<u8>>>, String> {
            Ok(self
                .data
                .iter()
                .enumerate()
                .map(|(i, (k, v))| GetEntry {
                    key: k.clone(),
                    value: v.clone(),
                    create_revision: i as i64 + 1,
                    mod_revision: i as i64 + 1,
                })
                .collect())
        }

        async fn get_raw(&self, _key: &str) -> Result<Option<GetEntry<Vec<u8>>>, String> {
            Ok(None)
        }

        async fn put_raw(&self, _key: &str, _value: Vec<u8>) -> Result<PutEntry<Vec<u8>>, String> {
            Ok(PutEntry::Created)
        }

        async fn delete(&self, _key: &str) -> Result<i64, String> {
            Ok(0)
        }

        async fn watch(&self, _prefix: Option<&str>) -> Result<ConfigEventReceiver> {
            Ok(self
                .watch_rx
                .lock()
                .unwrap()
                .take()
                .expect("MockProvider::watch called more than once"))
        }

        async fn shutdown(&self) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn raw(v: &impl serde::Serialize) -> Vec<u8> {
        serde_json::to_vec(v).unwrap()
    }

    fn put_event(key: &str, v: &impl serde::Serialize, rev: i64) -> ConfigEvent {
        ConfigEvent::Put((key.into(), serde_json::to_vec(v).unwrap(), rev))
    }

    fn delete_event(key: &str, rev: i64) -> ConfigEvent {
        ConfigEvent::Delete((key.into(), rev))
    }

    #[tokio::test]
    async fn entity_store_full_load() {
        let (tx, rx) = mpsc::channel(8);
        let provider = MockProvider::new(
            vec![
                ("/items/a", raw(&item("alice", "admin"))),
                ("/items/b", raw(&item("bob", "user"))),
            ],
            rx,
        );

        let store = EntityStore::new(provider, "/items/", "test", None, ITEM_INDEXES).await;

        assert!(store.get("a").is_some());
        assert!(store.get("b").is_some());
        assert!(store.get("c").is_none());
        assert_eq!(store.list().len(), 2);

        drop(tx);
    }

    #[tokio::test]
    async fn entity_store_watch_put_adds_entry() {
        let (tx, rx) = mpsc::channel(8);
        let provider = MockProvider::new(vec![], rx);

        let store = EntityStore::new(provider, "/items/", "test", None, ITEM_INDEXES).await;

        tx.send(put_event("/items/c", &item("carol", "mod"), 10))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        let entry = store
            .get("c")
            .expect("entry c should exist after Put event");
        assert_eq!(entry.name, "carol");
        assert_eq!(store.latest_mod_revision(), 10);
    }

    #[tokio::test]
    async fn entity_store_watch_delete_removes_entry() {
        let (tx, rx) = mpsc::channel(8);
        let provider = MockProvider::new(vec![("/items/a", raw(&item("alice", "admin")))], rx);

        let store = EntityStore::new(provider, "/items/", "test", None, ITEM_INDEXES).await;
        assert!(store.get("a").is_some());

        tx.send(delete_event("/items/a", 5)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(store.get("a").is_none());
        assert_eq!(store.latest_mod_revision(), 5);
    }

    #[tokio::test]
    async fn entity_store_validator_skips_invalid_on_full_load() {
        let (tx, rx) = mpsc::channel(8);
        let provider = MockProvider::new(
            vec![
                ("/items/good", raw(&item("alice", "admin"))),
                ("/items/bad", raw(&item("", "admin"))),
            ],
            rx,
        );

        fn reject_empty_name(_key: &str, v: &Item) -> Result<(), String> {
            if v.name.is_empty() {
                Err("name must not be empty".into())
            } else {
                Ok(())
            }
        }

        let store = EntityStore::new(
            provider,
            "/items/",
            "test",
            Some(reject_empty_name),
            ITEM_INDEXES,
        )
        .await;

        assert!(store.get("good").is_some());
        assert!(store.get("bad").is_none());

        drop(tx);
    }

    #[tokio::test]
    async fn entity_store_validator_skips_invalid_on_watch_put() {
        let (tx, rx) = mpsc::channel(8);
        let provider = MockProvider::new(vec![], rx);

        fn reject_empty_name(_key: &str, v: &Item) -> Result<(), String> {
            if v.name.is_empty() {
                Err("name must not be empty".into())
            } else {
                Ok(())
            }
        }

        let store = EntityStore::new(
            provider,
            "/items/",
            "test",
            Some(reject_empty_name),
            ITEM_INDEXES,
        )
        .await;

        tx.send(put_event("/items/bad", &item("", "x"), 1))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(store.get("bad").is_none());
    }

    #[tokio::test]
    async fn entity_store_secondary_index_available_after_full_load() {
        let (tx, rx) = mpsc::channel(8);
        let provider = MockProvider::new(vec![("/items/a", raw(&item("alice", "admin")))], rx);

        let store = EntityStore::new(provider, "/items/", "test", None, ITEM_INDEXES).await;

        assert!(store.get_by_secondary("by_name", "alice").is_some());
        assert!(store.get_by_secondary("by_tag", "admin").is_some());

        drop(tx);
    }

    #[tokio::test]
    async fn entity_store_secondary_index_updated_via_watch() {
        let (tx, rx) = mpsc::channel(8);
        let provider = MockProvider::new(vec![], rx);

        let store = EntityStore::new(provider, "/items/", "test", None, ITEM_INDEXES).await;

        tx.send(put_event("/items/a", &item("alice", "admin"), 1))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(store.get_by_secondary("by_name", "alice").is_some());

        tx.send(put_event("/items/a", &item("bob", "admin"), 2))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(store.get_by_secondary("by_name", "alice").is_none());
        assert!(store.get_by_secondary("by_name", "bob").is_some());
    }

    #[tokio::test]
    async fn entity_store_secondary_index_cleared_on_delete() {
        let (tx, rx) = mpsc::channel(8);
        let provider = MockProvider::new(vec![("/items/a", raw(&item("alice", "admin")))], rx);

        let store = EntityStore::new(provider, "/items/", "test", None, ITEM_INDEXES).await;

        assert!(store.get_by_secondary("by_name", "alice").is_some());

        tx.send(delete_event("/items/a", 3)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(store.get_by_secondary("by_name", "alice").is_none());
    }
}
