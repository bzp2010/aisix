mod apikeys;
mod guardrails;
mod models;
mod playground;
mod policies;
mod providers;
pub mod types;

use std::sync::Arc;
use std::str::FromStr;

use anyhow::{Context, Result};
use axum::{
    Router,
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use http::{HeaderValue, Method, header::HeaderName};
use tower_http::cors::{AllowOrigin, CorsLayer};
use utoipa::{
    Modify, OpenApi,
    openapi::security::{
        ApiKey as OASApiKey, ApiKeyValue as OASApiKeyValue, HttpAuthScheme,
        HttpBuilder as OASHttpBuilder, SecurityScheme,
    },
};
use utoipa_scalar::{Scalar, Servable as ScalarServable};

use aisix_config::{ConfigProvider, entities::ResourceRegistry};

use self::types::AuthError;

pub const PATH_PREFIX: &str = "/aisix/admin";

#[derive(OpenApi)]
#[openapi(
    info(description = "AI Gateway Admin API"),
    modifiers(&SecurityAddon),
    tags(
        (name = models::OPENAPI_TAG, description = "Admin API for managing AI models"),
        (name = apikeys::OPENAPI_TAG, description = "Admin API for managing API keys"),
        (name = guardrails::OPENAPI_TAG, description = "Admin API for managing guardrails"),
        (name = policies::OPENAPI_TAG, description = "Admin API for managing guardrail policies"),
        (name = providers::OPENAPI_TAG, description = "Admin API for managing AI providers")
    ),
    security(
        ("bearer" = []),
        ("api_key" = [])
    ),
    paths(
        models::list,
        models::get,
        models::post,
        models::put,
        models::delete,
        providers::list,
        providers::get,
        providers::post,
        providers::put,
        providers::delete,
        apikeys::list,
        apikeys::get,
        apikeys::post,
        apikeys::put,
        apikeys::delete,
        guardrails::list,
        guardrails::get,
        guardrails::post,
        guardrails::put,
        guardrails::delete,
        policies::list,
        policies::get,
        policies::post,
        policies::put,
        policies::delete,
    )
)]
struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer",
                SecurityScheme::Http(OASHttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
            );
            components.add_security_scheme(
                "api_key",
                SecurityScheme::ApiKey(OASApiKey::Header(OASApiKeyValue::new("x-api-key"))),
            );
        }
    }
}

/// CORS configuration for the admin server.
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
        let mut cors =
            CorsLayer::new().allow_credentials(self.allow_credentials.unwrap_or(false));

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
                parse(value)
                    .with_context(|| format!("Invalid CORS {}: {}", field, value))
            })
            .collect()
    }
}

/// An admin API key entry.
#[derive(Clone, Debug)]
pub struct AdminKey {
    pub key: String,
}

#[derive(Clone)]
pub struct AppState {
    cors_config: ServerCommonCors,
    admin_keys: Vec<AdminKey>,
    config_provider: Arc<dyn ConfigProvider>,
    resources: Arc<ResourceRegistry>,
    proxy_router: Option<Router>,
}

impl AppState {
    pub fn new(
        cors_config: ServerCommonCors,
        admin_keys: Vec<AdminKey>,
        config_provider: Arc<dyn ConfigProvider>,
        resources: Arc<ResourceRegistry>,
        proxy_router: Option<Router>,
    ) -> Self {
        Self {
            cors_config,
            admin_keys,
            config_provider,
            resources,
            proxy_router,
        }
    }

    pub fn proxy_router(&self) -> Option<Router> {
        self.proxy_router.clone()
    }
}

pub fn create_router(state: AppState) -> Result<Router> {
    let mut router = Router::new()
        .nest(
            PATH_PREFIX,
            Router::new()
                .merge(
                    Router::new()
                        .route("/models", get(models::list).post(models::post))
                        .route(
                            "/models/{id}",
                            get(models::get).put(models::put).delete(models::delete),
                        ),
                )
                .merge(
                    Router::new()
                        .route("/providers", get(providers::list).post(providers::post))
                        .route(
                            "/providers/{id}",
                            get(providers::get)
                                .put(providers::put)
                                .delete(providers::delete),
                        ),
                )
                .merge(
                    Router::new()
                        .route("/apikeys", get(apikeys::list).post(apikeys::post))
                        .route(
                            "/apikeys/{id}",
                            get(apikeys::get).put(apikeys::put).delete(apikeys::delete),
                        ),
                )
                .merge(
                    Router::new()
                        .route("/guardrails", get(guardrails::list).post(guardrails::post))
                        .route(
                            "/guardrails/{id}",
                            get(guardrails::get)
                                .put(guardrails::put)
                                .delete(guardrails::delete),
                        ),
                )
                .merge(
                    Router::new()
                        .route("/policies", get(policies::list).post(policies::post))
                        .route(
                            "/policies/{id}",
                            get(policies::get)
                                .put(policies::put)
                                .delete(policies::delete),
                        ),
                )
                .layer(axum::middleware::from_fn_with_state(state.clone(), auth)),
        )
        // These routes use API key authentication instead of Admin key authentication.
        .nest(
            "/playground",
            Router::new().route("/chat/completions", post(playground::chat_completions)),
        )
        .route("/ui", get(|| async { Redirect::to("/ui/") }))
        .route("/ui/", get(crate::ui::handler))
        .route("/ui/{*path}", get(crate::ui::handler))
        .merge(Scalar::with_url("/openapi", ApiDoc::openapi()));

    let cors = &state.cors_config;
    if cors.enabled {
        router = router.layer(cors.to_cors_layer()?)
    };

    Ok(router.with_state(state))
}

async fn auth(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let api_key = match req.headers().get(http::header::AUTHORIZATION) {
        Some(value) => {
            let header = value.to_str().unwrap_or("");
            let (prefix, rest) = header.split_at(7.min(header.len()));
            if prefix.eq_ignore_ascii_case("bearer ") {
                rest
            } else {
                header
            }
        }
        None => match req.headers().get("x-api-key") {
            Some(value) => value.to_str().unwrap_or(""),
            None => return Err(AuthError::MissingKey.into_response()),
        },
    };

    if state.admin_keys.is_empty() {
        return Err(AuthError::MissingKey.into_response());
    }

    if !state.admin_keys.iter().any(|item| item.key == api_key) {
        return Err(AuthError::InvalidKey.into_response());
    }

    Ok(next.run(req).await)
}
