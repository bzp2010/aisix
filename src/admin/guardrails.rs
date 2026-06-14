use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use http::StatusCode;
use uuid::Uuid;

use crate::{
    admin::{
        AppState,
        types::{APIError, DeleteResponse, ItemResponse, ListResponse},
    },
    config::PutEntry,
};
use aisix_core::entities::{
    Guardrail, Policy,
    guardrails::{SCHEMA_VALIDATOR, validate_guardrail_definition},
};
use aisix_utils::jsonschema::format_evaluation_error;

pub const OPENAPI_TAG: &str = "Guardrails";

#[utoipa::path(
    get,
    context_path = crate::admin::PATH_PREFIX,
    path = "/guardrails",
    tag = OPENAPI_TAG,
    responses(
        (status = StatusCode::OK, description = "Get guardrail list success", body = ListResponse<ItemResponse<Guardrail>>),
        (status = StatusCode::INTERNAL_SERVER_ERROR, description = "Internal server error", body = APIError)
    )
)]
pub async fn list(State(state): State<AppState>) -> Response {
    let data = match state
        .config_provider
        .get_all::<serde_json::Value>("/guardrails")
        .await
    {
        Ok(data) => data,
        Err(err) => return APIError::InternalError(err).into_response(),
    };

    ListResponse {
        total: data.len(),
        list: data
            .into_iter()
            .map(|item| ItemResponse {
                key: item.key,
                value: item.value,
                created_index: Some(item.create_revision),
                modified_index: Some(item.mod_revision),
            })
            .collect(),
    }
    .into_response()
}

#[utoipa::path(
    get,
    context_path = crate::admin::PATH_PREFIX,
    path = "/guardrails/{id}",
    tag = OPENAPI_TAG,
    params(
        ("id" = String, Path, description = "The ID of the guardrail"),
    ),
    responses(
        (status = StatusCode::OK, description = "Get guardrail success", body = ItemResponse<Guardrail>),
        (status = StatusCode::NOT_FOUND, description = "Guardrail not found", body = APIError),
        (status = StatusCode::INTERNAL_SERVER_ERROR, description = "Internal server error", body = APIError)
    )
)]
pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let key = format!("/guardrails/{id}");
    let data = match state.config_provider.get::<serde_json::Value>(&key).await {
        Ok(Some(data)) => data,
        Ok(None) => {
            return APIError::NotFound(format!("Guardrail with ID {id} not found")).into_response();
        }
        Err(err) => return APIError::InternalError(err).into_response(),
    };

    ItemResponse {
        key,
        value: data.value,
        created_index: Some(data.create_revision),
        modified_index: Some(data.mod_revision),
    }
    .into_response()
}

#[utoipa::path(
    post,
    context_path = crate::admin::PATH_PREFIX,
    path = "/guardrails",
    tag = OPENAPI_TAG,
    request_body(content_type = "application/json", content = Guardrail),
    responses(
        (status = StatusCode::CREATED, description = "Guardrail created successfully", body = ItemResponse<Guardrail>),
        (status = StatusCode::BAD_REQUEST, description = "Bad request", body = APIError),
        (status = StatusCode::INTERNAL_SERVER_ERROR, description = "Internal server error", body = APIError)
    )
)]
pub async fn post(State(state): State<AppState>, body: Bytes) -> Response {
    update(state, &Uuid::new_v4().to_string(), body).await
}

#[utoipa::path(
    put,
    context_path = crate::admin::PATH_PREFIX,
    path = "/guardrails/{id}",
    tag = OPENAPI_TAG,
    params(
        ("id" = String, Path, description = "The ID of the guardrail"),
    ),
    request_body(content_type = "application/json", content = Guardrail),
    responses(
        (status = StatusCode::OK, description = "Guardrail updated successfully", body = ItemResponse<Guardrail>),
        (status = StatusCode::CREATED, description = "Guardrail created successfully", body = ItemResponse<Guardrail>),
        (status = StatusCode::BAD_REQUEST, description = "Bad request", body = APIError),
        (status = StatusCode::INTERNAL_SERVER_ERROR, description = "Internal server error", body = APIError)
    )
)]
pub async fn put(State(state): State<AppState>, Path(id): Path<String>, body: Bytes) -> Response {
    update(state, &id, body).await
}

#[utoipa::path(
    delete,
    context_path = crate::admin::PATH_PREFIX,
    path = "/guardrails/{id}",
    tag = OPENAPI_TAG,
    params(
        ("id" = String, Path, description = "The ID of the guardrail"),
    ),
    responses(
        (status = StatusCode::BAD_REQUEST, description = "Guardrail is still referenced by policies", body = APIError),
        (status = StatusCode::OK, description = "Guardrail deleted successfully", body = DeleteResponse),
        (status = StatusCode::NOT_FOUND, description = "Guardrail not found", body = APIError),
        (status = StatusCode::INTERNAL_SERVER_ERROR, description = "Internal server error", body = APIError)
    )
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let key = format!("/guardrails/{id}");

    match state.config_provider.get::<serde_json::Value>(&key).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return APIError::NotFound(format!("Guardrail with ID {id} not found")).into_response();
        }
        Err(err) => return APIError::InternalError(err).into_response(),
    }

    match state.config_provider.get_all::<Policy>("/policies").await {
        Ok(policies) => {
            if policies.iter().any(|item| {
                item.value
                    .referenced_guardrail_ids()
                    .any(|guardrail_id| guardrail_id == id)
            }) {
                return APIError::BadRequest(
                    "guardrail is still referenced by policies".to_string(),
                )
                .into_response();
            }
        }
        Err(err) => return APIError::InternalError(err).into_response(),
    }

    match state.config_provider.delete(&key).await {
        Ok(deleted) if deleted > 0 => DeleteResponse { deleted, key }.into_response(),
        Ok(_) => APIError::NotFound(format!("Guardrail with ID {id} not found")).into_response(),
        Err(err) => APIError::InternalError(err).into_response(),
    }
}

async fn update(state: AppState, id: &str, body: Bytes) -> Response {
    let key = format!("/guardrails/{id}");

    let guardrail = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(value) => value,
        Err(err) => return APIError::BadRequest(format!("Invalid JSON: {err}")).into_response(),
    };

    let evaluation = SCHEMA_VALIDATOR.evaluate(&guardrail);
    if !evaluation.flag().valid {
        return APIError::BadRequest(format!(
            "JSON schema validation error: {}",
            format_evaluation_error(&evaluation)
        ))
        .into_response();
    }

    let guardrail = match serde_json::from_value::<Guardrail>(guardrail) {
        Ok(value) => value,
        Err(err) => {
            return APIError::BadRequest(format!("Invalid guardrail data: {err}")).into_response();
        }
    };

    if let Err(err) = validate_guardrail_definition(id, &guardrail) {
        return APIError::BadRequest(err).into_response();
    }

    match state.config_provider.put(&key, &guardrail).await {
        Ok(res) => match res {
            PutEntry::Created => (
                StatusCode::CREATED,
                ItemResponse {
                    key: key.to_string(),
                    value: guardrail,
                    created_index: None,
                    modified_index: None,
                },
            )
                .into_response(),
            PutEntry::Updated(_prev) => (
                StatusCode::OK,
                ItemResponse {
                    key: key.to_string(),
                    value: guardrail,
                    created_index: None,
                    modified_index: None,
                },
            )
                .into_response(),
        },
        Err(err) => APIError::InternalError(err).into_response(),
    }
}
