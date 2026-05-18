use std::{convert::Infallible, sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::{
    Json,
    extract::State,
    response::{
        IntoResponse, Response,
        sse::{Event as SseEvent, Sse},
    },
};
use fastrace::prelude::{Event as TraceEvent, Span};
use log::error;
use reqwest::Url;
use serde::Serialize;
use tokio::{
    sync::{oneshot, oneshot::error::TryRecvError},
    time::error::Elapsed,
};

use crate::{
    config::entities::{Model, ResourceEntry},
    gateway::{
        error::GatewayError,
        traits::{ChatFormat, ProviderCapabilities},
        types::{
            common::Usage,
            response::{ChatResponse, ChatResponseStream},
        },
    },
    guardrail::traits::{GuardrailCheckPayload, GuardrailOutcome},
    proxy::{
        AppState,
        guardrails::{
            ConfiguredGuardrailRuntime, resolve_model_guardrails,
            streaming::{
                StreamGuardrailDecision, WholeResponseReplayAction, WholeResponseReplayDriver,
                WholeResponseReplayFinalize,
            },
        },
        hooks::{
            self, RequestContext, authorization::AuthorizationError, rate_limit::RateLimitError,
        },
        provider::create_provider_instance,
        utils::trace::span_attributes::{apply_span_properties, usage_span_properties},
    },
    utils::future::{WithSpan, maybe_timeout},
};

type AdapterFormat<A> = <A as FormatHandlerAdapter>::Format;
type AdapterRequest<A> = <A as FormatHandlerAdapter>::Request;
type AdapterResponse<A> = <A as FormatHandlerAdapter>::Response;
type AdapterCollector<A> = <A as FormatHandlerAdapter>::Collector;

#[async_trait]
pub(crate) trait FormatHandlerAdapter: Send + Sync + 'static {
    type Format: ChatFormat<
            Request = Self::Request,
            Response = Self::Response,
            StreamChunk = Self::StreamChunk,
        >;
    type Request: Sync;
    type Response: Serialize;
    type StreamChunk: Clone + Serialize + Send + 'static;
    type Error: IntoResponse
        + std::fmt::Display
        + From<AuthorizationError>
        + From<RateLimitError>
        + From<GatewayError>
        + From<Elapsed>
        + Send;
    type Collector: Default + Send + 'static;
    type LifecycleState: Default + Send + 'static;

    fn span_name() -> &'static str;

    fn missing_model_error() -> Self::Error;

    fn set_model(request: &mut Self::Request, model: String);

    fn request_span_properties(
        request: &Self::Request,
        provider: &dyn ProviderCapabilities,
        base_url: Option<&Url>,
    ) -> Vec<(String, String)>;

    fn response_span_properties(response: &Self::Response, usage: &Usage) -> Vec<(String, String)>;

    fn apply_chunk_span_properties(span: &Span, chunk: &Self::StreamChunk, is_first_item: bool);

    fn starts_output(chunk: &Self::StreamChunk) -> bool;

    fn record_stream_item(collector: &mut Self::Collector, chunk: &Self::StreamChunk);

    fn output_message_span_properties(collector: &Self::Collector) -> Vec<(String, String)>;

    fn guardrail_input_payload(
        _lifecycle_state: &Self::LifecycleState,
        _request: &Self::Request,
    ) -> Result<Option<GuardrailCheckPayload>, Self::Error> {
        Ok(None)
    }

    fn apply_input_guardrail_rewrite(
        _lifecycle_state: &mut Self::LifecycleState,
        _request: &mut Self::Request,
        _rewrite: GuardrailCheckPayload,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn guardrail_output_payload(
        _lifecycle_state: &Self::LifecycleState,
        _response: &Self::Response,
    ) -> Result<Option<GuardrailCheckPayload>, Self::Error> {
        Ok(None)
    }

    fn apply_output_guardrail_rewrite(
        _lifecycle_state: &mut Self::LifecycleState,
        _response: &mut Self::Response,
        _rewrite: GuardrailCheckPayload,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn guardrail_stream_output_payload(
        _lifecycle_state: &Self::LifecycleState,
        _collector: &Self::Collector,
    ) -> Result<Option<GuardrailCheckPayload>, Self::Error> {
        Ok(None)
    }

    async fn prepare_lifecycle(
        _state: &AppState,
        _request_ctx: &mut RequestContext,
        _request: &mut Self::Request,
    ) -> Result<Self::LifecycleState, Self::Error> {
        Ok(Self::LifecycleState::default())
    }

    async fn handle_complete_response(
        _state: &AppState,
        _request_ctx: &mut RequestContext,
        _lifecycle_state: &mut Self::LifecycleState,
        _response: &mut Self::Response,
        _usage: &Usage,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn handle_stream_item(
        _state: &AppState,
        _request_ctx: &mut RequestContext,
        _lifecycle_state: &mut Self::LifecycleState,
        _chunk: &mut Self::StreamChunk,
    ) {
    }

    async fn handle_stream_success(
        _state: &AppState,
        _request_ctx: &mut RequestContext,
        _lifecycle_state: Self::LifecycleState,
        _usage: Option<&Usage>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn handle_stream_failure(
        _state: &AppState,
        _request_ctx: &mut RequestContext,
        _lifecycle_state: Self::LifecycleState,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_stream_item(chunk: &Self::StreamChunk) -> SseEvent {
        let mut event =
            SseEvent::default().data(<Self::Format as ChatFormat>::serialize_chunk_payload(chunk));

        if let Some(event_type) = <Self::Format as ChatFormat>::sse_event_type(chunk) {
            event = event.event(event_type);
        }

        event
    }

    fn stream_error_event(_error: &GatewayError) -> Option<SseEvent> {
        None
    }

    fn lifecycle_error_event(_error: &Self::Error) -> Option<SseEvent> {
        None
    }

    fn end_of_stream_event(_saw_item: bool) -> Option<SseEvent> {
        None
    }
}

pub(crate) async fn format_handler<A>(
    State(state): State<AppState>,
    mut request_ctx: RequestContext,
    Json(mut request_data): Json<AdapterRequest<A>>,
) -> Result<Response, A::Error>
where
    A: FormatHandlerAdapter,
{
    hooks::observability::record_start_time(&mut request_ctx).await;
    hooks::authorization::check(
        &mut request_ctx,
        <AdapterFormat<A> as ChatFormat>::extract_model(&request_data).to_owned(),
    )
    .await?;
    hooks::rate_limit::pre_check(&mut request_ctx).await?;

    let model = request_ctx
        .extensions()
        .await
        .get::<ResourceEntry<Model>>()
        .cloned()
        .ok_or_else(A::missing_model_error)?;

    A::set_model(&mut request_data, model.model.clone());
    let timeout = model.timeout.map(Duration::from_millis);

    let gateway = state.gateway();
    let resources = state.resources();
    let provider = model.provider(resources.as_ref()).ok_or_else(|| {
        GatewayError::Internal(format!("provider {} not found", model.provider_id))
    })?;
    let configured_guardrails = resolve_model_guardrails(&model, resources.as_ref())?;
    let provider_instance = create_provider_instance(gateway.as_ref(), &provider)?;
    let provider_base_url = provider_instance.effective_base_url().ok();
    let mut lifecycle_state =
        A::prepare_lifecycle(&state, &mut request_ctx, &mut request_data).await?;

    apply_input_guardrails::<A>(
        &configured_guardrails,
        &mut lifecycle_state,
        &mut request_data,
    )
    .await?;

    let span = Span::enter_with_local_parent(A::span_name());
    apply_span_properties(
        &span,
        A::request_span_properties(
            &request_data,
            provider_instance.def.as_ref(),
            provider_base_url.as_ref(),
        ),
    );

    let (response, span) = (WithSpan {
        inner: maybe_timeout(timeout, async {
            Ok(state
                .gateway()
                .chat::<AdapterFormat<A>>(&request_data, &provider_instance)
                .await?)
        }),
        span: Some(span),
    })
    .await;

    match response {
        Ok(Ok(ChatResponse::Complete {
            mut response,
            usage,
        })) => {
            let output_guardrail_result = apply_output_guardrails::<A>(
                &configured_guardrails,
                &mut lifecycle_state,
                &mut response,
            )
            .await;
            apply_span_properties(
                &span,
                output_guardrail_failure_span_properties(&output_guardrail_result, || {
                    A::response_span_properties(&response, &usage)
                }),
            );
            output_guardrail_result?;
            A::handle_complete_response(
                &state,
                &mut request_ctx,
                &mut lifecycle_state,
                &mut response,
                &usage,
            )
            .await?;
            span.add_properties(|| A::response_span_properties(&response, &usage));
            handle_regular_response::<A>(response, usage, &mut request_ctx).await
        }
        Ok(Ok(ChatResponse::Stream { stream, usage_rx })) => {
            handle_stream_response::<A>(
                state,
                configured_guardrails,
                stream,
                usage_rx,
                &mut request_ctx,
                span,
                lifecycle_state,
            )
            .await
        }
        Ok(Err(err)) => {
            span.add_property(|| ("error.type", "gateway_error"));
            Err(err)
        }
        Err(err) => {
            span.add_property(|| ("error.type", "timeout"));
            Err(err.into())
        }
    }
}

async fn apply_input_guardrails<A>(
    guardrails: &[Box<dyn ConfiguredGuardrailRuntime>],
    lifecycle_state: &mut A::LifecycleState,
    request: &mut AdapterRequest<A>,
) -> Result<(), A::Error>
where
    A: FormatHandlerAdapter,
{
    for guardrail in guardrails {
        let Some(payload) = A::guardrail_input_payload(lifecycle_state, request)? else {
            continue;
        };
        let Some(outcome) = guardrail.check(&payload).await? else {
            continue;
        };

        match outcome {
            GuardrailOutcome::Allow => {}
            GuardrailOutcome::Rewrite(rewrite) => {
                A::apply_input_guardrail_rewrite(lifecycle_state, request, rewrite)?;
            }
            GuardrailOutcome::Block { reason } => {
                return Err(GatewayError::Validation(format!(
                    "guardrail {} blocked input: {}",
                    guardrail.name(),
                    reason
                ))
                .into());
            }
        }
    }

    Ok(())
}

async fn apply_output_guardrails<A>(
    guardrails: &[Box<dyn ConfiguredGuardrailRuntime>],
    lifecycle_state: &mut A::LifecycleState,
    response: &mut AdapterResponse<A>,
) -> Result<(), A::Error>
where
    A: FormatHandlerAdapter,
{
    for guardrail in guardrails {
        let Some(payload) = A::guardrail_output_payload(lifecycle_state, response)? else {
            continue;
        };
        let Some(outcome) = guardrail.check(&payload).await? else {
            continue;
        };

        match outcome {
            GuardrailOutcome::Allow => {}
            GuardrailOutcome::Rewrite(rewrite) => {
                A::apply_output_guardrail_rewrite(lifecycle_state, response, rewrite)?;
            }
            GuardrailOutcome::Block { reason } => {
                return Err(GatewayError::Validation(format!(
                    "guardrail {} blocked output: {}",
                    guardrail.name(),
                    reason
                ))
                .into());
            }
        }
    }

    Ok(())
}

async fn apply_stream_output_guardrails<A>(
    guardrails: &[Box<dyn ConfiguredGuardrailRuntime>],
    payload: &GuardrailCheckPayload,
) -> Result<(), A::Error>
where
    A: FormatHandlerAdapter,
{
    for guardrail in guardrails {
        let Some(outcome) = guardrail.check(payload).await? else {
            continue;
        };

        match outcome {
            GuardrailOutcome::Allow => {}
            GuardrailOutcome::Rewrite(_) => {
                return Err(GatewayError::Validation(format!(
                    "guardrail {} requested streaming output rewrite, which is not supported yet",
                    guardrail.name()
                ))
                .into());
            }
            GuardrailOutcome::Block { reason } => {
                return Err(GatewayError::Validation(format!(
                    "guardrail {} blocked output: {}",
                    guardrail.name(),
                    reason
                ))
                .into());
            }
        }
    }

    Ok(())
}

fn require_stream_output_guardrail_payload(
    payload: Option<GuardrailCheckPayload>,
) -> Result<GuardrailCheckPayload, GatewayError> {
    payload.ok_or_else(|| {
        GatewayError::Internal(
            "stream output guardrails were enabled, but the adapter did not provide a stream output payload"
                .into(),
        )
    })
}

fn has_output_guardrails(guardrails: &[Box<dyn ConfiguredGuardrailRuntime>]) -> bool {
    guardrails
        .iter()
        .any(|guardrail| guardrail.supports_stage(crate::guardrail::traits::GuardrailStage::Output))
}

fn output_guardrail_failure_span_properties<E, F>(
    result: &Result<(), E>,
    properties: F,
) -> Vec<(String, String)>
where
    F: FnOnce() -> Vec<(String, String)>,
{
    if result.is_err() {
        return properties();
    }

    Vec::new()
}

async fn handle_regular_response<A>(
    response: AdapterResponse<A>,
    usage: Usage,
    request_ctx: &mut RequestContext,
) -> Result<Response, A::Error>
where
    A: FormatHandlerAdapter,
{
    if let Err(err) = hooks::rate_limit::post_check(request_ctx, &usage).await {
        error!("Rate limit post_check error: {}", err);
    }

    let mut response = Json(response).into_response();
    hooks::rate_limit::inject_response_headers(request_ctx, response.headers_mut()).await;
    hooks::observability::record_usage(request_ctx, &usage).await;

    Ok(response)
}

fn spawn_stream_usage_observer(request_ctx: RequestContext, usage_rx: oneshot::Receiver<Usage>) {
    tokio::spawn(async move {
        let mut request_ctx = request_ctx;

        match usage_rx.await {
            Ok(usage) => {
                if let Err(err) =
                    hooks::rate_limit::post_check_streaming(&mut request_ctx, &usage).await
                {
                    error!("Rate limit post_check_streaming error: {}", err);
                }
                hooks::observability::record_streaming_usage(&mut request_ctx, &usage).await;
            }
            Err(err) => {
                error!("Failed to receive streaming usage from gateway: {}", err);
            }
        }
    });
}

async fn finalize_stream_usage_observation(
    request_ctx: &mut RequestContext,
    usage_rx: &mut Option<oneshot::Receiver<Usage>>,
    span: &Span,
) {
    if let Some(mut usage_rx) = usage_rx.take() {
        match usage_rx.try_recv() {
            Ok(usage) => {
                if let Err(err) = hooks::rate_limit::post_check_streaming(request_ctx, &usage).await
                {
                    error!("Rate limit post_check_streaming error: {}", err);
                }
                hooks::observability::record_streaming_usage(request_ctx, &usage).await;
                span.add_properties(|| usage_span_properties(&usage));
            }
            Err(TryRecvError::Empty) => {
                spawn_stream_usage_observer(request_ctx.clone(), usage_rx);
            }
            Err(TryRecvError::Closed) => {
                error!("Failed to receive streaming usage from gateway: channel closed");
            }
        }
    }
}

async fn finalize_stream_success<A>(
    state: &AppState,
    request_ctx: &mut RequestContext,
    usage_rx: &mut Option<oneshot::Receiver<Usage>>,
    span: &Span,
    output_message_properties: Vec<(String, String)>,
    lifecycle_state: &mut Option<A::LifecycleState>,
) -> Result<(), A::Error>
where
    A: FormatHandlerAdapter,
{
    span.add_properties(|| output_message_properties);

    let Some(lifecycle_state) = lifecycle_state.take() else {
        finalize_stream_usage_observation(request_ctx, usage_rx, span).await;
        return Ok(());
    };

    if let Some(mut usage_rx) = usage_rx.take() {
        match usage_rx.try_recv() {
            Ok(usage) => {
                if let Err(err) = hooks::rate_limit::post_check_streaming(request_ctx, &usage).await
                {
                    error!("Rate limit post_check_streaming error: {}", err);
                }
                hooks::observability::record_streaming_usage(request_ctx, &usage).await;
                span.add_properties(|| usage_span_properties(&usage));

                A::handle_stream_success(state, request_ctx, lifecycle_state, Some(&usage)).await?;
            }
            Err(TryRecvError::Empty) => match usage_rx.await {
                Ok(usage) => {
                    if let Err(err) =
                        hooks::rate_limit::post_check_streaming(request_ctx, &usage).await
                    {
                        error!("Rate limit post_check_streaming error: {}", err);
                    }
                    hooks::observability::record_streaming_usage(request_ctx, &usage).await;
                    span.add_properties(|| usage_span_properties(&usage));

                    A::handle_stream_success(state, request_ctx, lifecycle_state, Some(&usage))
                        .await?;
                }
                Err(err) => {
                    error!("Failed to receive streaming usage from gateway: {}", err);
                    A::handle_stream_success(state, request_ctx, lifecycle_state, None).await?;
                }
            },
            Err(TryRecvError::Closed) => {
                error!("Failed to receive streaming usage from gateway: channel closed");
                A::handle_stream_success(state, request_ctx, lifecycle_state, None).await?;
            }
        }
        return Ok(());
    }

    A::handle_stream_success(state, request_ctx, lifecycle_state, None).await
}

async fn record_first_stream_output_emit<A>(
    request_ctx: &mut RequestContext,
    span: &Span,
    first_output_arrived: &mut bool,
    starts_output: bool,
) where
    A: FormatHandlerAdapter,
{
    if *first_output_arrived || !starts_output {
        return;
    }

    *first_output_arrived = true;
    hooks::observability::record_first_token_latency(request_ctx).await;
    span.add_event(
        TraceEvent::new("first token arrived").with_property(|| ("kind", "first_token_arrived")),
    );
}

async fn handle_stream_response<A>(
    state: AppState,
    configured_guardrails: Vec<Box<dyn ConfiguredGuardrailRuntime>>,
    stream: ChatResponseStream<AdapterFormat<A>>,
    usage_rx: oneshot::Receiver<Usage>,
    request_ctx: &mut RequestContext,
    span: Span,
    lifecycle_state: A::LifecycleState,
) -> Result<Response, A::Error>
where
    A: FormatHandlerAdapter,
{
    use futures::stream::StreamExt;

    let stream_request_ctx = request_ctx.clone();
    let stream_state = state.clone();
    let replay_driver =
        WholeResponseReplayDriver::new(has_output_guardrails(&configured_guardrails));
    let configured_guardrails = Arc::new(configured_guardrails);
    let sse_stream = futures::stream::unfold(
        (
            stream_state,
            configured_guardrails,
            stream,
            span,
            stream_request_ctx,
            false,
            false,
            Some(usage_rx),
            AdapterCollector::<A>::default(),
            AdapterCollector::<A>::default(),
            false,
            Some(lifecycle_state),
            replay_driver,
        ),
        |(
            state,
            configured_guardrails,
            mut stream,
            span,
            mut request_ctx,
            should_terminate,
            saw_item,
            mut usage_rx,
            mut guardrail_output_collector,
            mut output_collector,
            mut first_output_arrived,
            mut lifecycle_state,
            mut replay_driver,
        )| async move {
            if should_terminate {
                drop(span);
                return None;
            }

            loop {
                if let Some(mut chunk) = replay_driver.take_replay_chunk() {
                    if let Some(lifecycle_state) = lifecycle_state.as_mut() {
                        A::handle_stream_item(
                            &state,
                            &mut request_ctx,
                            lifecycle_state,
                            &mut chunk,
                        );
                    }

                    record_first_stream_output_emit::<A>(
                        &mut request_ctx,
                        &span,
                        &mut first_output_arrived,
                        A::starts_output(&chunk),
                    )
                    .await;

                    A::record_stream_item(&mut output_collector, &chunk);
                    A::apply_chunk_span_properties(&span, &chunk, !saw_item);

                    break Some((
                        Ok::<SseEvent, Infallible>(A::serialize_stream_item(&chunk)),
                        (
                            state,
                            configured_guardrails,
                            stream,
                            span,
                            request_ctx,
                            false,
                            true,
                            usage_rx,
                            guardrail_output_collector,
                            output_collector,
                            first_output_arrived,
                            lifecycle_state,
                            replay_driver,
                        ),
                    ));
                }

                if replay_driver.is_upstream_finished() {
                    match finalize_stream_success::<A>(
                        &state,
                        &mut request_ctx,
                        &mut usage_rx,
                        &span,
                        A::output_message_span_properties(&output_collector),
                        &mut lifecycle_state,
                    )
                    .await
                    {
                        Ok(()) => {}
                        Err(err) => {
                            error!("Stream success lifecycle error: {}", err);
                            span.add_property(|| ("error.type", "stream_success_lifecycle_error"));

                            if let Some(event) = A::lifecycle_error_event(&err) {
                                break Some((
                                    Ok(event),
                                    (
                                        state,
                                        configured_guardrails,
                                        stream,
                                        span,
                                        request_ctx,
                                        true,
                                        saw_item,
                                        usage_rx,
                                        guardrail_output_collector,
                                        output_collector,
                                        first_output_arrived,
                                        lifecycle_state,
                                        replay_driver,
                                    ),
                                ));
                            }

                            drop(span);
                            break None;
                        }
                    }

                    if let Some(event) = A::end_of_stream_event(saw_item) {
                        break Some((
                            Ok(event),
                            (
                                state,
                                configured_guardrails,
                                stream,
                                span,
                                request_ctx,
                                true,
                                saw_item,
                                usage_rx,
                                guardrail_output_collector,
                                output_collector,
                                first_output_arrived,
                                lifecycle_state,
                                replay_driver,
                            ),
                        ));
                    }

                    drop(span);
                    break None;
                }

                match stream.next().await {
                    Some(Ok(chunk)) => match replay_driver.push_upstream_chunk(chunk) {
                        WholeResponseReplayAction::Buffered(chunk) => {
                            A::record_stream_item(&mut guardrail_output_collector, &chunk);
                            continue;
                        }
                        WholeResponseReplayAction::Emit(mut chunk) => {
                            if let Some(lifecycle_state) = lifecycle_state.as_mut() {
                                A::handle_stream_item(
                                    &state,
                                    &mut request_ctx,
                                    lifecycle_state,
                                    &mut chunk,
                                );
                            }

                            record_first_stream_output_emit::<A>(
                                &mut request_ctx,
                                &span,
                                &mut first_output_arrived,
                                A::starts_output(&chunk),
                            )
                            .await;

                            A::record_stream_item(&mut output_collector, &chunk);
                            A::apply_chunk_span_properties(&span, &chunk, !saw_item);

                            break Some((
                                Ok::<SseEvent, Infallible>(A::serialize_stream_item(&chunk)),
                                (
                                    state,
                                    configured_guardrails,
                                    stream,
                                    span,
                                    request_ctx,
                                    false,
                                    true,
                                    usage_rx,
                                    guardrail_output_collector,
                                    output_collector,
                                    first_output_arrived,
                                    lifecycle_state,
                                    replay_driver,
                                ),
                            ));
                        }
                    },
                    Some(Err(err)) => {
                        error!("Gateway stream error: {}", err);
                        span.add_property(|| ("error.type", "stream_error"));
                        if replay_driver.is_buffering() {
                            span.add_properties(|| {
                                A::output_message_span_properties(&guardrail_output_collector)
                            });
                        } else {
                            span.add_properties(|| {
                                A::output_message_span_properties(&output_collector)
                            });
                        }

                        if let Some(lifecycle_state) = lifecycle_state.take() {
                            if let Err(lifecycle_err) =
                                A::handle_stream_failure(&state, &mut request_ctx, lifecycle_state)
                                    .await
                            {
                                error!("Stream failure lifecycle error: {}", lifecycle_err);
                            }
                        }

                        finalize_stream_usage_observation(&mut request_ctx, &mut usage_rx, &span)
                            .await;

                        if let Some(event) = A::stream_error_event(&err) {
                            break Some((
                                Ok(event),
                                (
                                    state,
                                    configured_guardrails,
                                    stream,
                                    span,
                                    request_ctx,
                                    true,
                                    saw_item,
                                    usage_rx,
                                    guardrail_output_collector,
                                    output_collector,
                                    first_output_arrived,
                                    lifecycle_state,
                                    replay_driver,
                                ),
                            ));
                        }

                        drop(span);
                        break None;
                    }
                    None => match replay_driver.finish_upstream() {
                        WholeResponseReplayFinalize::NeedsGuardrailCheck => {
                            let output_guardrail_result =
                                if let Some(lifecycle_state) = lifecycle_state.as_ref() {
                                    match A::guardrail_stream_output_payload(
                                        lifecycle_state,
                                        &guardrail_output_collector,
                                    ) {
                                        Ok(payload) => {
                                            match require_stream_output_guardrail_payload(payload) {
                                                Ok(payload) => {
                                                    apply_stream_output_guardrails::<A>(
                                                        configured_guardrails.as_ref(),
                                                        &payload,
                                                    )
                                                    .await
                                                }
                                                Err(err) => Err(err.into()),
                                            }
                                        }
                                        Err(err) => Err(err),
                                    }
                                } else {
                                    Ok(())
                                };

                            match output_guardrail_result {
                                Ok(()) => {
                                    let decision = replay_driver.approve_buffered();
                                    debug_assert!(matches!(
                                        decision,
                                        StreamGuardrailDecision::Allow { .. }
                                    ));
                                    continue;
                                }
                                Err(err) => {
                                    error!("Stream output guardrail error: {}", err);
                                    span.add_property(|| {
                                        ("error.type", "stream_output_guardrail_error")
                                    });
                                    span.add_properties(|| {
                                        A::output_message_span_properties(
                                            &guardrail_output_collector,
                                        )
                                    });

                                    if let Some(lifecycle_state) = lifecycle_state.take() {
                                        if let Err(lifecycle_err) = A::handle_stream_failure(
                                            &state,
                                            &mut request_ctx,
                                            lifecycle_state,
                                        )
                                        .await
                                        {
                                            error!(
                                                "Stream failure lifecycle error: {}",
                                                lifecycle_err
                                            );
                                        }
                                    }

                                    finalize_stream_usage_observation(
                                        &mut request_ctx,
                                        &mut usage_rx,
                                        &span,
                                    )
                                    .await;

                                    if let Some(event) = A::lifecycle_error_event(&err) {
                                        break Some((
                                            Ok(event),
                                            (
                                                state,
                                                configured_guardrails,
                                                stream,
                                                span,
                                                request_ctx,
                                                true,
                                                saw_item,
                                                usage_rx,
                                                guardrail_output_collector,
                                                output_collector,
                                                first_output_arrived,
                                                lifecycle_state,
                                                replay_driver,
                                            ),
                                        ));
                                    }

                                    drop(span);
                                    break None;
                                }
                            }
                        }
                        WholeResponseReplayFinalize::Finished => {}
                    },
                }
            }
        },
    );

    let mut response = Sse::new(sse_stream).into_response();
    hooks::rate_limit::inject_response_headers(request_ctx, response.headers_mut()).await;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use pretty_assertions::assert_eq;

    use super::{
        output_guardrail_failure_span_properties, require_stream_output_guardrail_payload,
    };
    use crate::{
        gateway::error::GatewayError,
        guardrail::traits::{GuardrailCheckPayload, OutputGuardrailPayload},
    };

    #[test]
    fn output_guardrail_failure_span_properties_skips_computation_on_success() {
        let computed = Cell::new(false);

        let properties =
            output_guardrail_failure_span_properties(&Ok::<(), &'static str>(()), || {
                computed.set(true);
                vec![(
                    "llm.output_messages.0.message.content".into(),
                    "hidden".into(),
                )]
            });

        assert!(properties.is_empty());
        assert!(!computed.get());
    }

    #[test]
    fn output_guardrail_failure_span_properties_returns_properties_on_error() {
        let computed = Cell::new(false);

        let properties =
            output_guardrail_failure_span_properties(&Err::<(), &'static str>("blocked"), || {
                computed.set(true);
                vec![(
                    "llm.output_messages.0.message.content".into(),
                    "raw upstream output".into(),
                )]
            });

        assert_eq!(
            properties,
            vec![(
                "llm.output_messages.0.message.content".into(),
                "raw upstream output".into(),
            )]
        );
        assert!(computed.get());
    }

    #[test]
    fn require_stream_output_guardrail_payload_rejects_missing_payload() {
        let err = require_stream_output_guardrail_payload(None).unwrap_err();

        assert!(matches!(err, GatewayError::Internal(_)));
        assert_eq!(
            err.to_string(),
            "internal: stream output guardrails were enabled, but the adapter did not provide a stream output payload",
        );
    }

    #[test]
    fn require_stream_output_guardrail_payload_passes_through_present_payload() {
        let payload = GuardrailCheckPayload::Output(OutputGuardrailPayload::default());

        assert_eq!(
            require_stream_output_guardrail_payload(Some(payload.clone())).unwrap(),
            payload,
        );
    }
}
