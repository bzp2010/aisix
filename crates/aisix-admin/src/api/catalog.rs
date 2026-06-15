use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
};
use http::StatusCode;
use serde::Serialize;
use utoipa::ToSchema;

use serde_json::json;

use crate::{
    api::{AppState, types::APIError},
    catalog::{ModelEntry, ProviderSummary},
};

pub const OPENAPI_TAG: &str = "models.dev Catalog";

#[derive(Serialize, ToSchema)]
pub struct RefreshResponse {
    pub message: String,
}

impl IntoResponse for RefreshResponse {
    fn into_response(self) -> Response {
        (StatusCode::OK, axum::Json(self)).into_response()
    }
}

#[utoipa::path(
    get,
    context_path = "/aisix/models-dev",
    path = "/providers",
    tag = OPENAPI_TAG,
    responses(
        (status = StatusCode::OK, description = "List providers from models.dev catalog", body = Vec<ProviderSummary>)
    )
)]
pub async fn list_providers(State(state): State<AppState>) -> Response {
    let providers = state.catalog_cache.list_providers();
    (StatusCode::OK, axum::Json(providers)).into_response()
}

#[utoipa::path(
    get,
    context_path = "/aisix/models-dev",
    path = "/providers/{id}/models",
    tag = OPENAPI_TAG,
    params(
        ("id" = String, Path, description = "The models.dev provider ID"),
    ),
    responses(
        (status = StatusCode::OK, description = "List models for a provider", body = Vec<ModelEntry>),
        (status = StatusCode::NOT_FOUND, description = "Provider not found in catalog", body = APIError)
    )
)]
pub async fn get_provider_models(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    match state.catalog_cache.get_provider_models(&id) {
        Some(models) => (StatusCode::OK, axum::Json(models)).into_response(),
        None => APIError::NotFound(format!("Provider '{}' not found in catalog", id))
            .into_response(),
    }
}

#[utoipa::path(
    post,
    context_path = "/aisix/models-dev",
    path = "/refresh",
    tag = OPENAPI_TAG,
    responses(
        (status = StatusCode::OK, description = "Catalog refreshed", body = RefreshResponse),
        (status = StatusCode::GATEWAY_TIMEOUT, description = "Refresh timed out", body = APIError)
    )
)]
pub async fn refresh(State(state): State<AppState>) -> Response {
    match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        state.catalog_cache.refresh(),
    )
    .await
    {
        Ok(()) => RefreshResponse {
            message: "Catalog refreshed".to_string(),
        }
        .into_response(),
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            axum::Json(json!({"error_msg": "Catalog refresh timed out"})),
        )
            .into_response(),
    }
}
