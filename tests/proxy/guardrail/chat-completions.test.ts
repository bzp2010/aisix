import { parseSseDataEvents, proxyPost } from '../../utils/proxy.js';
import {
  expectStreamMatchesAssistantText,
  expectStreamStopsBeforeDone,
} from '../../utils/stream-assert.js';
import {
  type RegexGuardrailFixture,
  setupOpenAiRegexGuardrailFixture,
} from './shared.js';

const ADMIN_KEY = 'test_admin_key_guardrail_chat_completions';
const AUTHORIZED_KEY = 'sk-proxy-guardrail-chat-completions';
const UPSTREAM_API_KEY = 'upstream-key-guardrail-chat-completions';
const UPSTREAM_MODEL = 'test-model';

describe('proxy guardrail /v1/chat/completions', () => {
  let fixture: RegexGuardrailFixture | undefined;

  beforeEach(async () => {
    fixture = await setupOpenAiRegexGuardrailFixture({
      adminKey: ADMIN_KEY,
      authorizedKey: AUTHORIZED_KEY,
      upstreamApiKey: UPSTREAM_API_KEY,
      upstreamModel: UPSTREAM_MODEL,
      modelPrefix: 'mock-chat-guardrail',
    });
  }, 30_000);

  afterEach(async () => {
    await fixture?.close();
  });

  test('input regex guardrail blocks request before upstream call', async () => {
    const resp = await proxyPost(
      '/v1/chat/completions',
      {
        model: fixture?.inputGuardedModelName,
        messages: [{ role: 'user', content: 'my secret token is 12345' }],
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

  test('input regex guardrail allows safe request through to upstream', async () => {
    const resp = await proxyPost(
      '/v1/chat/completions',
      {
        model: fixture?.inputGuardedModelName,
        messages: [
          { role: 'user', content: 'safe request through regex guardrail' },
        ],
      },
      AUTHORIZED_KEY,
    );

    expect(resp.status).toBe(200);
    expect(resp.data.choices[0].message.content).toBe(
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

  test('output regex guardrail blocks matched upstream response', async () => {
    const resp = await proxyPost(
      '/v1/chat/completions',
      {
        model: fixture?.outputGuardedModelName,
        messages: [
          { role: 'user', content: 'safe prompt for output guardrail' },
        ],
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

  test('output regex guardrail blocks matched upstream streamed response', async () => {
    const resp = await proxyPost(
      '/v1/chat/completions',
      {
        model: fixture?.outputGuardedModelName,
        stream: true,
        messages: [
          {
            role: 'user',
            content: 'safe prompt for streamed output guardrail',
          },
        ],
      },
      AUTHORIZED_KEY,
      { responseType: 'text' },
    );

    expect(resp.status).toBe(200);
    expect(String(resp.headers['content-type'])).toContain('text/event-stream');

    const events = expectStreamStopsBeforeDone(String(resp.data));
    const payloads = parseSseDataEvents(String(resp.data)).map(
      (item) =>
        JSON.parse(item) as {
          error?: {
            message?: string;
            type?: string;
            code?: string;
          };
        },
    );

    expect(events).toHaveLength(1);
    expect(payloads[0]?.error?.type).toBe('invalid_request_error');
    expect(payloads[0]?.error?.code).toBe('gateway_error');
    expect(payloads[0]?.error?.message).toContain(
      'guardrail regex blocked output',
    );
    expect(payloads[0]?.error?.message).toContain(
      'blocked by regex output guardrail',
    );

    const recorded = fixture?.upstream.takeRecordedRequests() ?? [];
    expect(recorded).toHaveLength(1);
  });

  test('approved streamed output replays original chunks after guardrail check', async () => {
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
      '/v1/chat/completions',
      {
        model: fixture?.outputGuardedModelName,
        stream: true,
        messages: [
          { role: 'user', content: 'safe prompt for streamed output replay' },
        ],
      },
      AUTHORIZED_KEY,
      { responseType: 'text' },
    );

    expect(resp.status).toBe(200);
    expect(String(resp.headers['content-type'])).toContain('text/event-stream');
    expectStreamMatchesAssistantText(
      String(resp.data),
      'safe streamed response',
    );

    const recorded = fixture?.upstream.takeRecordedRequests() ?? [];
    expect(recorded).toHaveLength(1);
  });
});
