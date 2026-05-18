import { proxyPost } from '../../utils/proxy.js';
import {
  type RegexGuardrailFixture,
  setupOpenAiRegexGuardrailFixture,
} from './shared.js';

const ADMIN_KEY = 'test_admin_key_guardrail_responses';
const AUTHORIZED_KEY = 'sk-proxy-guardrail-responses';
const UPSTREAM_API_KEY = 'upstream-key-guardrail-responses';
const UPSTREAM_MODEL = 'test-model';

const parseResponsesSseEvents = (sseBody: string) => {
  const trimmed = sseBody.trim();
  if (!trimmed) {
    return [] as Array<{ event?: string; data: string }>;
  }

  return trimmed.split(/\r?\n\r?\n/).map((block) => {
    const lines = block
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);

    return {
      event: lines.find((line) => line.startsWith('event: '))?.slice(7),
      data: lines
        .filter((line) => line.startsWith('data: '))
        .map((line) => line.slice(6))
        .join('\n'),
    };
  });
};

describe('proxy guardrail /v1/responses', () => {
  let fixture: RegexGuardrailFixture | undefined;

  beforeEach(async () => {
    fixture = await setupOpenAiRegexGuardrailFixture({
      adminKey: ADMIN_KEY,
      authorizedKey: AUTHORIZED_KEY,
      upstreamApiKey: UPSTREAM_API_KEY,
      upstreamModel: UPSTREAM_MODEL,
      modelPrefix: 'mock-responses-guardrail',
    });
  }, 30_000);

  afterEach(async () => {
    await fixture?.close();
  });

  test('input regex guardrail blocks responses request before upstream call', async () => {
    const resp = await proxyPost(
      '/v1/responses',
      {
        model: fixture?.inputGuardedModelName,
        input: 'my secret token is 12345',
      },
      AUTHORIZED_KEY,
    );

    expect(resp.status).toBe(400);
    expect(resp.data.error.code).toBe('gateway_error');
    expect(resp.data.error.type).toBe('invalid_request_error');
    expect(resp.data.error.message).toContain('guardrail regex blocked input');
    expect(resp.data.error.message).toContain(
      'blocked by regex input guardrail',
    );

    const recorded = fixture?.upstream.takeRecordedRequests() ?? [];
    expect(recorded).toHaveLength(0);
  });

  test('input regex guardrail allows safe responses request through to upstream', async () => {
    const resp = await proxyPost(
      '/v1/responses',
      {
        model: fixture?.inputGuardedModelName,
        input: 'safe request through regex guardrail',
      },
      AUTHORIZED_KEY,
    );

    expect(resp.status).toBe(200);
    expect(resp.data.object).toBe('response');
    expect(resp.data.output[0].content[0].text).toBe(
      'hello from mock upstream',
    );

    const recorded = fixture?.upstream.takeRecordedRequests() ?? [];
    expect(recorded).toHaveLength(1);
    expect(
      (
        recorded[0]?.bodyJson as {
          messages: Array<{ content: string }>;
        }
      ).messages[0]?.content,
    ).toBe('safe request through regex guardrail');
  });

  test('output regex guardrail blocks matched responses output', async () => {
    const resp = await proxyPost(
      '/v1/responses',
      {
        model: fixture?.outputGuardedModelName,
        input: 'safe prompt for output guardrail',
      },
      AUTHORIZED_KEY,
    );

    expect(resp.status).toBe(400);
    expect(resp.data.error.code).toBe('gateway_error');
    expect(resp.data.error.type).toBe('invalid_request_error');
    expect(resp.data.error.message).toContain('guardrail regex blocked output');
    expect(resp.data.error.message).toContain(
      'blocked by regex output guardrail',
    );

    const recorded = fixture?.upstream.takeRecordedRequests() ?? [];
    expect(recorded).toHaveLength(1);
    expect(
      (
        recorded[0]?.bodyJson as {
          messages: Array<{ content: string }>;
        }
      ).messages[0]?.content,
    ).toBe('safe prompt for output guardrail');
  });

  test('output regex guardrail blocks matched streamed responses output', async () => {
    const resp = await proxyPost(
      '/v1/responses',
      {
        model: fixture?.outputGuardedModelName,
        input: 'safe prompt for streamed output guardrail',
        stream: true,
      },
      AUTHORIZED_KEY,
      { responseType: 'text' },
    );

    expect(resp.status).toBe(200);
    expect(String(resp.headers['content-type'])).toContain('text/event-stream');

    const events = parseResponsesSseEvents(String(resp.data));
    expect(events).toHaveLength(1);
    expect(events[0]?.event).toBe('error');

    const payload = JSON.parse(events[0]?.data ?? '{}') as {
      type?: string;
      message?: string;
    };
    expect(payload.type).toBe('error');
    expect(payload.message).toContain('guardrail regex blocked output');
    expect(payload.message).toContain('blocked by regex output guardrail');

    const recorded = fixture?.upstream.takeRecordedRequests() ?? [];
    expect(recorded).toHaveLength(1);
  });

  test('approved streamed responses output replays original response events', async () => {
    fixture?.upstream.configure({
      streamEvents: [
        {
          id: 'chatcmpl-e2e-mock',
          object: 'chat.completion.chunk',
          created: 1,
          model: UPSTREAM_MODEL,
          choices: [
            {
              index: 0,
              delta: { role: 'assistant', content: 'safe ' },
              finish_reason: null,
            },
          ],
        },
        {
          id: 'chatcmpl-e2e-mock',
          object: 'chat.completion.chunk',
          created: 1,
          model: UPSTREAM_MODEL,
          choices: [
            {
              index: 0,
              delta: { content: 'streamed response' },
              finish_reason: null,
            },
          ],
        },
        {
          id: 'chatcmpl-e2e-mock',
          object: 'chat.completion.chunk',
          created: 1,
          model: UPSTREAM_MODEL,
          choices: [
            {
              index: 0,
              delta: {},
              finish_reason: 'stop',
            },
          ],
        },
        {
          id: 'chatcmpl-e2e-mock',
          object: 'chat.completion.chunk',
          created: 1,
          model: UPSTREAM_MODEL,
          choices: [],
          usage: {
            prompt_tokens: 10,
            completion_tokens: 8,
            total_tokens: 18,
          },
        },
        '[DONE]',
      ],
    });

    const resp = await proxyPost(
      '/v1/responses',
      {
        model: fixture?.outputGuardedModelName,
        input: 'safe prompt for streamed output replay',
        stream: true,
      },
      AUTHORIZED_KEY,
      { responseType: 'text' },
    );

    expect(resp.status).toBe(200);
    expect(String(resp.headers['content-type'])).toContain('text/event-stream');

    const events = parseResponsesSseEvents(String(resp.data));
    expect(events[0]?.event).toBe('response.created');
    expect(
      events.some((event) => event.event === 'response.output_text.delta'),
    ).toBe(true);
    expect(events.at(-1)?.event).toBe('response.completed');

    const streamedText = events
      .filter((event) => event.event === 'response.output_text.delta')
      .map((event) => JSON.parse(event.data) as { delta?: string })
      .map((event) => event.delta ?? '')
      .join('');
    expect(streamedText).toBe('safe streamed response');

    const recorded = fixture?.upstream.takeRecordedRequests() ?? [];
    expect(recorded).toHaveLength(1);
  });
});
