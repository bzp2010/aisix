use std::collections::HashSet;

use cel::{Context, Value as CelValue};
use serde::Serialize;
use serde_json::Value as JsonValue;

use aisix_core::entities::{
    ApiKey, Model, Policy, Provider,
    policies::{PolicyAction, PolicyStage},
};
use crate::config::entities::ResourceEntry;
use crate::config::entities::ResourceRegistry;

use crate::{
    gateway::{error::GatewayError, types::openai::ChatMessage},
    guardrail::traits::GuardrailStage,
    proxy::{
        guardrails::{ResolvedGuardrail, build_resolved_guardrail_for_stage},
        hooks::{RequestContext, RequestRouteInfo},
    },
};

pub(crate) fn stable_route_format(format_name: &'static str) -> &'static str {
    match format_name {
        "openai_chat" => "chat_completions",
        "anthropic_messages" => "messages",
        "openai_responses" => "responses",
        other => other,
    }
}

pub(crate) struct RequestGuardrailResolution<'a> {
    pub request_ctx: &'a RequestContext,
    pub model: &'a ResourceEntry<Model>,
    pub provider: &'a ResourceEntry<Provider>,
    pub route_format: &'static str,
    pub request_model: &'a str,
    pub request_stream: bool,
    pub request_raw: &'a JsonValue,
    pub input_messages: &'a [ChatMessage],
    pub resources: &'a ResourceRegistry,
}

pub(crate) async fn resolve_request_guardrails(
    request: RequestGuardrailResolution<'_>,
) -> Result<Vec<Box<dyn ResolvedGuardrail>>, GatewayError> {
    let (api_key, route) = {
        let extensions = request.request_ctx.extensions().await;
        let api_key = extensions
            .get::<ResourceEntry<ApiKey>>()
            .cloned()
            .ok_or_else(|| GatewayError::Internal("policy context missing api key".into()))?;
        let route = extensions
            .get::<RequestRouteInfo>()
            .cloned()
            .ok_or_else(|| GatewayError::Internal("policy context missing route info".into()))?;
        (api_key, route)
    };

    let context = PolicyContext {
        auth: PolicyAuthContext {
            api_key: PolicyApiKeyContext { id: &api_key.id },
        },
        model: PolicyModelContext {
            id: &request.model.id,
            name: &request.model.name,
            upstream: &request.model.model,
        },
        provider: PolicyProviderContext {
            id: &request.provider.id,
            name: &request.provider.name,
            provider_type: request.provider.provider_type(),
        },
        route: PolicyRouteContext {
            method: &route.method,
            path: &route.path,
            format: request.route_format,
        },
        request: PolicyRequestContext {
            model: request.request_model,
            stream: request.request_stream,
            raw: request.request_raw,
        },
        input: PolicyInputContext {
            messages: request.input_messages,
        },
    };

    let policies = request.resources.policies.list();
    let mut matched_policies = policies
        .values()
        .filter(|policy| policy.enabled)
        .filter_map(|policy| match policy_matches(policy, &context) {
            Ok(true) => Some(Ok(policy)),
            Ok(false) => None,
            Err(err) => Some(Err(err)),
        })
        .collect::<Result<Vec<_>, _>>()?;

    matched_policies.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut seen_bindings = HashSet::new();
    let mut resolved_guardrails = Vec::new();

    for policy in matched_policies {
        for action in &policy.actions {
            match action {
                PolicyAction::Guardrail(config) => {
                    for stage in &config.stages {
                        let resolved_stage = guardrail_stage_from_policy_stage(stage);
                        let stage_key = guardrail_stage_key(resolved_stage);

                        for guardrail_id in &config.guardrail_ids {
                            let dedupe_key = format!("{guardrail_id}:{stage_key}");
                            if !seen_bindings.insert(dedupe_key) {
                                continue;
                            }

                            let guardrail = request
                                .resources
                                .guardrails
                                .get_by_id(guardrail_id)
                                .ok_or_else(|| {
                                    GatewayError::Internal(format!(
                                        "guardrail {} referenced by policy {} not found",
                                        guardrail_id, policy.id
                                    ))
                                })?;

                            resolved_guardrails.push(build_resolved_guardrail_for_stage(
                                &guardrail.guardrail,
                                resolved_stage,
                            )?);
                        }
                    }
                }
            }
        }
    }

    Ok(resolved_guardrails)
}

fn policy_matches(
    policy: &ResourceEntry<Policy>,
    context: &PolicyContext<'_>,
) -> Result<bool, GatewayError> {
    let mut cel_context = Context::default();
    cel_context
        .add_variable("auth", &context.auth)
        .map_err(|err| {
            GatewayError::Internal(format!("policy {} context error: {err}", policy.id))
        })?;
    cel_context
        .add_variable("model", &context.model)
        .map_err(|err| {
            GatewayError::Internal(format!("policy {} context error: {err}", policy.id))
        })?;
    cel_context
        .add_variable("provider", &context.provider)
        .map_err(|err| {
            GatewayError::Internal(format!("policy {} context error: {err}", policy.id))
        })?;
    cel_context
        .add_variable("route", &context.route)
        .map_err(|err| {
            GatewayError::Internal(format!("policy {} context error: {err}", policy.id))
        })?;
    cel_context
        .add_variable("request", &context.request)
        .map_err(|err| {
            GatewayError::Internal(format!("policy {} context error: {err}", policy.id))
        })?;
    cel_context
        .add_variable("input", &context.input)
        .map_err(|err| {
            GatewayError::Internal(format!("policy {} context error: {err}", policy.id))
        })?;

    let program = policy.compiled_when().map_err(|err| {
        GatewayError::Internal(format!(
            "policy {} failed to compile cached CEL: {err}",
            policy.id
        ))
    })?;
    let result = program.execute(&cel_context).map_err(|err| {
        GatewayError::Internal(format!("policy {} evaluation failed: {err}", policy.id))
    })?;

    match result {
        CelValue::Bool(value) => Ok(value),
        other => Err(GatewayError::Internal(format!(
            "policy {} when must evaluate to bool, got {:?}",
            policy.id, other
        ))),
    }
}

fn guardrail_stage_from_policy_stage(stage: &PolicyStage) -> GuardrailStage {
    match stage {
        PolicyStage::Input => GuardrailStage::Input,
        PolicyStage::Output => GuardrailStage::Output,
    }
}

fn guardrail_stage_key(stage: GuardrailStage) -> &'static str {
    match stage {
        GuardrailStage::Input => "input",
        GuardrailStage::Output => "output",
    }
}

#[derive(Serialize)]
struct PolicyContext<'a> {
    auth: PolicyAuthContext<'a>,
    model: PolicyModelContext<'a>,
    provider: PolicyProviderContext<'a>,
    route: PolicyRouteContext<'a>,
    request: PolicyRequestContext<'a>,
    input: PolicyInputContext<'a>,
}

#[derive(Serialize)]
struct PolicyAuthContext<'a> {
    api_key: PolicyApiKeyContext<'a>,
}

#[derive(Serialize)]
struct PolicyApiKeyContext<'a> {
    id: &'a str,
}

#[derive(Serialize)]
struct PolicyModelContext<'a> {
    id: &'a str,
    name: &'a str,
    upstream: &'a str,
}

#[derive(Serialize)]
struct PolicyProviderContext<'a> {
    id: &'a str,
    name: &'a str,
    #[serde(rename = "type")]
    provider_type: &'a str,
}

#[derive(Serialize)]
struct PolicyRouteContext<'a> {
    method: &'a str,
    path: &'a str,
    format: &'a str,
}

#[derive(Serialize)]
struct PolicyRequestContext<'a> {
    model: &'a str,
    stream: bool,
    raw: &'a JsonValue,
}

#[derive(Serialize)]
struct PolicyInputContext<'a> {
    messages: &'a [ChatMessage],
}
