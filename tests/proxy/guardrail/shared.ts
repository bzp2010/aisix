import { randomUUID } from 'node:crypto';

import {
  MODELS_URL,
  POLICIES_URL,
  PROVIDERS_URL,
  adminPost,
  adminPut,
  bearerAuthHeader,
  startIsolatedAdminApp,
} from '../../utils/admin.js';
import { etcdPutJson } from '../../utils/etcd.js';
import {
  type OpenAiMockUpstream,
  buildOpenAiProviderConfig,
  startOpenAiMockUpstream,
} from '../../utils/mock-upstream.js';
import { App } from '../../utils/setup.js';

const waitConfigPropagation = async () => {
  await new Promise((resolve) => setTimeout(resolve, 1000));
};

const ensureStatus = (
  response: { status: number; data?: unknown },
  expectedStatus: number,
  context: string,
) => {
  if (response.status !== expectedStatus) {
    throw new Error(
      `${context} failed: expected ${expectedStatus}, got ${response.status}: ${JSON.stringify(response.data)}`,
    );
  }
};

export interface RegexGuardrailFixture {
  server: App;
  upstream: OpenAiMockUpstream;
  inputGuardedModelName: string;
  outputGuardedModelName: string;
  close: () => Promise<void>;
}

interface SetupOpenAiRegexGuardrailFixtureOptions {
  adminKey: string;
  authorizedKey: string;
  upstreamApiKey: string;
  upstreamModel: string;
  modelPrefix: string;
  buildModel?: (model: string) => string;
}

export const setupOpenAiRegexGuardrailFixture = async ({
  adminKey,
  authorizedKey,
  upstreamApiKey,
  upstreamModel,
  modelPrefix,
  buildModel = (model) => model,
}: SetupOpenAiRegexGuardrailFixtureOptions): Promise<RegexGuardrailFixture> => {
  const etcdPrefix = `/ai-admin-${randomUUID()}`;
  let server: App | undefined;
  let upstream: OpenAiMockUpstream | undefined;

  try {
    server = await startIsolatedAdminApp(adminKey, etcdPrefix);
    upstream = await startOpenAiMockUpstream();
    const auth = bearerAuthHeader(adminKey);

    const inputGuardedModelName = `${modelPrefix}-input-${randomUUID()}`;
    const outputGuardedModelName = `${modelPrefix}-output-${randomUUID()}`;
    const providerId = `${modelPrefix}-provider-${randomUUID()}`;
    const inputGuardrailId = `${modelPrefix}-regex-input-${randomUUID()}`;
    const outputGuardrailId = `${modelPrefix}-regex-output-${randomUUID()}`;

    ensureStatus(
      await adminPut(
        `${PROVIDERS_URL}/${providerId}`,
        {
          name: providerId,
          type: 'openai',
          config: buildOpenAiProviderConfig(upstream.apiBase, upstreamApiKey),
        },
        auth,
      ),
      201,
      'create provider',
    );

    await etcdPutJson(etcdPrefix, `/guardrails/${inputGuardrailId}`, {
      name: `${modelPrefix}-regex-input`,
      type: 'regex',
      config: {
        pattern: 'secret token',
        block_reason: 'blocked by regex input guardrail',
      },
    });

    await etcdPutJson(etcdPrefix, `/guardrails/${outputGuardrailId}`, {
      name: `${modelPrefix}-regex-output`,
      type: 'regex',
      config: {
        pattern: 'hello from mock upstream',
        block_reason: 'blocked by regex output guardrail',
      },
    });

    ensureStatus(
      await adminPost(
        MODELS_URL,
        {
          name: inputGuardedModelName,
          model: buildModel(upstreamModel),
          provider_id: providerId,
        },
        auth,
      ),
      201,
      'create input-guarded model',
    );

    ensureStatus(
      await adminPost(
        MODELS_URL,
        {
          name: outputGuardedModelName,
          model: buildModel(upstreamModel),
          provider_id: providerId,
        },
        auth,
      ),
      201,
      'create output-guarded model',
    );

    ensureStatus(
      await adminPost(
        POLICIES_URL,
        {
          name: `${modelPrefix}-input-policy-${randomUUID()}`,
          when: `model.name == '${inputGuardedModelName}'`,
          actions: [
            {
              type: 'guardrail',
              config: {
                stages: ['input'],
                guardrail_ids: [inputGuardrailId],
              },
            },
          ],
        },
        auth,
      ),
      201,
      'create input guardrail policy',
    );

    ensureStatus(
      await adminPost(
        POLICIES_URL,
        {
          name: `${modelPrefix}-output-policy-${randomUUID()}`,
          when: `model.name == '${outputGuardedModelName}'`,
          actions: [
            {
              type: 'guardrail',
              config: {
                stages: ['output'],
                guardrail_ids: [outputGuardrailId],
              },
            },
          ],
        },
        auth,
      ),
      201,
      'create output guardrail policy',
    );

    ensureStatus(
      await adminPost(
        '/apikeys',
        {
          key: authorizedKey,
          allowed_models: [inputGuardedModelName, outputGuardedModelName],
        },
        auth,
      ),
      201,
      'create apikey',
    );

    await waitConfigPropagation();

    return {
      server,
      upstream,
      inputGuardedModelName,
      outputGuardedModelName,
      close: async () => {
        await upstream?.close();
        await server?.exit();
      },
    };
  } catch (error) {
    await upstream?.close();
    await server?.exit();
    throw error;
  }
};
