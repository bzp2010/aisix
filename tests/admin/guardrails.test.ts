import { randomUUID } from 'node:crypto';

import {
  GUARDRAILS_URL,
  POLICIES_URL,
  adminDelete,
  adminGet,
  adminPost,
  adminPut,
  bearerAuthHeader,
  extractIdFromStorageKey,
  startIsolatedAdminApp,
} from '../utils/admin.js';
import { App } from '../utils/setup.js';

const ADMIN_KEY = 'test_admin_key';

const buildRegexGuardrailBody = (
  name: string,
  pattern = 'blocked phrase',
  blockReason = 'blocked by guardrail admin test',
) => ({
  name,
  type: 'regex',
  config: {
    pattern,
    block_reason: blockReason,
  },
});

const buildPolicyBody = (name: string, guardrailId: string) => ({
  name,
  when: 'true',
  actions: [
    {
      type: 'guardrail',
      config: {
        guardrail_ids: [guardrailId],
      },
    },
  ],
});

describe('admin guardrails', () => {
  let server: App | undefined;
  let etcdPrefix = '';

  beforeEach(async () => {
    etcdPrefix = `/ai-admin-${randomUUID()}`;
    server = await startIsolatedAdminApp(ADMIN_KEY, etcdPrefix);
  });

  afterEach(async () => await server?.exit());

  test('test_crud', async () => {
    const auth = bearerAuthHeader(ADMIN_KEY);

    const listBefore = await adminGet(GUARDRAILS_URL, auth);
    expect(listBefore.status).toBe(200);
    expect(listBefore.data.total).toBe(0);

    const createResp = await adminPost(
      GUARDRAILS_URL,
      buildRegexGuardrailBody('test_guardrail'),
      auth,
    );
    expect(createResp.status).toBe(201);
    expect(createResp.data.value.name).toBe('test_guardrail');
    expect(createResp.data.value.type).toBe('regex');
    expect(createResp.data.value.config.pattern).toBe('blocked phrase');

    const id = extractIdFromStorageKey(createResp.data.key);

    const listAfterCreate = await adminGet(GUARDRAILS_URL, auth);
    expect(listAfterCreate.status).toBe(200);
    expect(listAfterCreate.data.total).toBe(1);

    const updateResp = await adminPut(
      `${GUARDRAILS_URL}/${id}`,
      buildRegexGuardrailBody(
        'updated_guardrail',
        'updated phrase',
        'updated block reason',
      ),
      auth,
    );
    expect(updateResp.status).toBe(200);
    expect(updateResp.data.value.name).toBe('updated_guardrail');
    expect(updateResp.data.value.config.pattern).toBe('updated phrase');
    expect(updateResp.data.value.config.block_reason).toBe(
      'updated block reason',
    );

    const getResp = await adminGet(`${GUARDRAILS_URL}/${id}`, auth);
    expect(getResp.status).toBe(200);
    expect(getResp.data.value.name).toBe('updated_guardrail');

    const deleteResp = await adminDelete(`${GUARDRAILS_URL}/${id}`, auth);
    expect(deleteResp.status).toBe(200);
    expect(deleteResp.data.deleted).toBe(1);

    const listAfterDelete = await adminGet(GUARDRAILS_URL, auth);
    expect(listAfterDelete.status).toBe(200);
    expect(listAfterDelete.data.total).toBe(0);
  });

  test('test_put_status_codes', async () => {
    const auth = bearerAuthHeader(ADMIN_KEY);
    const body = buildRegexGuardrailBody('put_guardrail');

    const firstPut = await adminPut(
      `${GUARDRAILS_URL}/put-guardrail-fixed-id`,
      body,
      auth,
    );
    expect(firstPut.status).toBe(201);

    const secondPut = await adminPut(
      `${GUARDRAILS_URL}/put-guardrail-fixed-id`,
      body,
      auth,
    );
    expect(secondPut.status).toBe(200);
  });

  test('test_invalid_schema_rejected', async () => {
    const auth = bearerAuthHeader(ADMIN_KEY);

    const createResp = await adminPost(
      GUARDRAILS_URL,
      {
        name: 'invalid_guardrail',
        type: 'regex',
        config: {},
      },
      auth,
    );

    expect(createResp.status).toBe(400);
    expect(createResp.data.error_msg).toContain('JSON schema validation error');
  });

  test('test_delete_referenced_guardrail_rejected', async () => {
    const auth = bearerAuthHeader(ADMIN_KEY);

    const guardrailResp = await adminPost(
      GUARDRAILS_URL,
      buildRegexGuardrailBody('referenced_guardrail'),
      auth,
    );
    expect(guardrailResp.status).toBe(201);

    const guardrailId = extractIdFromStorageKey(guardrailResp.data.key);

    const policyResp = await adminPost(
      POLICIES_URL,
      buildPolicyBody('guardrail_ref_policy', guardrailId),
      auth,
    );
    expect(policyResp.status).toBe(201);

    const deleteResp = await adminDelete(
      `${GUARDRAILS_URL}/${guardrailId}`,
      auth,
    );
    expect(deleteResp.status).toBe(400);
    expect(deleteResp.data.error_msg).toBe(
      'guardrail is still referenced by policies',
    );

    const getResp = await adminGet(`${GUARDRAILS_URL}/${guardrailId}`, auth);
    expect(getResp.status).toBe(200);
  });
});
