mod guardrails;
mod handlers;
mod hooks;
pub mod message_history;
mod middlewares;
mod policies;
mod provider;
mod utils;

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware::{from_fn, from_fn_with_state},
    routing::{get, post},
};
use std::str::FromStr;

use http::{HeaderValue, Method, header::HeaderName};
use tower_http::cors::{AllowOrigin, CorsLayer};

use aisix_config::entities::ResourceRegistry;
use aisix_llm::Gateway;

use self::message_history::MessageHistoryStorage;

#[derive(Clone, Debug, Default)]
pub struct ServerCommonCors {
    pub enabled: bool,
    pub allowed_origins: Option<Vec<String>>,
    pub allowed_methods: Option<Vec<String>>,
    pub allowed_headers: Option<Vec<String>>,
    pub exposed_headers: Option<Vec<String>>,
    pub allow_credentials: Option<bool>,
}

impl ServerCommonCors {
    pub fn to_cors_layer(&self) -> Result<CorsLayer> {
        let mut cors = CorsLayer::new().allow_credentials(self.allow_credentials.unwrap_or(false));

        if let Some(origins) = self.allowed_origins.as_deref() {
            cors = cors.allow_origin(if origins.iter().any(|o| o == "*") {
                AllowOrigin::any()
            } else {
                AllowOrigin::list(Self::parse_cors_values(
                    "allowed_origin",
                    origins,
                    HeaderValue::from_str,
                )?)
            });
        }

        if let Some(methods) = self.allowed_methods.as_deref() {
            cors = cors.allow_methods(Self::parse_cors_values(
                "allowed_method",
                methods,
                Method::from_str,
            )?);
        }

        if let Some(headers) = self.allowed_headers.as_deref() {
            cors = cors.allow_headers(Self::parse_cors_values(
                "allowed_header",
                headers,
                HeaderName::from_str,
            )?);
        }

        if let Some(headers) = self.exposed_headers.as_deref() {
            cors = cors.expose_headers(Self::parse_cors_values(
                "exposed_header",
                headers,
                HeaderName::from_str,
            )?);
        }

        Ok(cors)
    }

    fn parse_cors_values<T, E, F>(field: &str, values: &[String], mut parse: F) -> Result<Vec<T>>
    where
        F: FnMut(&str) -> std::result::Result<T, E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        values
            .iter()
            .map(|value| {
                parse(value).with_context(|| format!("Invalid CORS {}: {}", field, value))
            })
            .collect()
    }
}

#[derive(Clone)]
pub struct AppState {
    cors: ServerCommonCors,
    resources: Arc<ResourceRegistry>,
    gateway: Arc<Gateway>,
    message_history_storage: Arc<dyn MessageHistoryStorage>,
}

impl AppState {
    pub fn new(
        cors: ServerCommonCors,
        resources: Arc<ResourceRegistry>,
        gateway: Arc<Gateway>,
        message_history_storage: Arc<dyn MessageHistoryStorage>,
    ) -> Self {
        Self {
            cors,
            resources,
            gateway,
            message_history_storage,
        }
    }

    pub fn resources(&self) -> Arc<ResourceRegistry> {
        self.resources.clone()
    }

    pub fn gateway(&self) -> Arc<Gateway> {
        self.gateway.clone()
    }

    pub fn message_history_storage(&self) -> Arc<dyn MessageHistoryStorage> {
        self.message_history_storage.clone()
    }
}

pub fn create_router(state: AppState) -> Result<Router> {
    let mut router = Router::new()
        .merge(Router::new().route("/v1/models", get(handlers::models::list_models)))
        .route(
            "/v1/chat/completions",
            post(handlers::format_handler::<handlers::chat_completions::ChatCompletionsAdapter>),
        )
        .route(
            "/v1/messages",
            post(handlers::format_handler::<handlers::messages::MessagesAdapter>)
                .layer(DefaultBodyLimit::max(32 * 1024 * 1024)),
        )
        .route(
            "/v1/responses",
            post(handlers::format_handler::<handlers::responses::ResponsesAdapter>),
        )
        .route("/v1/embeddings", post(handlers::embeddings::embeddings))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .layer(from_fn_with_state(state.clone(), middlewares::auth))
        .layer(from_fn(middlewares::trace));

    let cors = &state.cors;
    if cors.enabled {
        router = router.layer(cors.to_cors_layer()?)
    };

    Ok(router.with_state(state))
}
