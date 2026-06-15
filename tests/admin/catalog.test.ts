import {
  bearerAuthHeader,
  catalogGet,
  catalogPost,
  startIsolatedAdminApp,
} from '../utils/admin.js';
import { App } from '../utils/setup.js';

const ADMIN_KEY = 'test_catalog_key';

describe('models-dev catalog', () => {
  let server: App | undefined;

  beforeEach(async () => {
    server = await startIsolatedAdminApp(ADMIN_KEY);
  });

  afterEach(async () => await server?.exit());

  describe('auth', () => {
    test('list_providers_requires_auth', async () => {
      const resp = await catalogGet('/providers');
      expect(resp.status).toBe(401);
    });

    test('get_provider_models_requires_auth', async () => {
      const resp = await catalogGet('/providers/openai/models');
      expect(resp.status).toBe(401);
    });

    test('refresh_requires_auth', async () => {
      const resp = await catalogPost('/refresh');
      expect(resp.status).toBe(401);
    });
  });

  describe('list_providers', () => {
    test('returns_array', async () => {
      const auth = bearerAuthHeader(ADMIN_KEY);
      const resp = await catalogGet('/providers', auth);
      expect(resp.status).toBe(200);
      expect(Array.isArray(resp.data)).toBe(true);
    });

    test('provider_entry_shape', async () => {
      const auth = bearerAuthHeader(ADMIN_KEY);
      const resp = await catalogGet('/providers', auth);
      expect(resp.status).toBe(200);
      // If the catalog loaded successfully, validate provider shape
      if ((resp.data as unknown[]).length > 0) {
        const first = resp.data[0] as Record<string, unknown>;
        expect(typeof first.id).toBe('string');
        expect(typeof first.name).toBe('string');
      }
    });
  });

  describe('get_provider_models', () => {
    test('unknown_provider_returns_404', async () => {
      const auth = bearerAuthHeader(ADMIN_KEY);
      const resp = await catalogGet(
        '/providers/this-provider-does-not-exist-xyz/models',
        auth,
      );
      expect(resp.status).toBe(404);
      expect(typeof resp.data.error_msg).toBe('string');
    });

    test('known_provider_returns_array_when_catalog_loaded', async () => {
      const auth = bearerAuthHeader(ADMIN_KEY);
      const listResp = await catalogGet('/providers', auth);
      expect(listResp.status).toBe(200);

      const providers = listResp.data as { id: string }[];
      if (providers.length === 0) {
        // Catalog not loaded (no network in test env), skip data assertions
        return;
      }

      const firstId = providers[0].id;
      const modelsResp = await catalogGet(`/providers/${firstId}/models`, auth);
      expect(modelsResp.status).toBe(200);
      expect(Array.isArray(modelsResp.data)).toBe(true);

      const models = modelsResp.data as { id: string; name: string }[];
      if (models.length > 0) {
        expect(typeof models[0].id).toBe('string');
        expect(typeof models[0].name).toBe('string');
      }
    });
  });

  describe('refresh', () => {
    test('returns_200_with_message', async () => {
      const auth = bearerAuthHeader(ADMIN_KEY);
      const resp = await catalogPost('/refresh', auth);
      expect(resp.status).toBe(200);
      expect(typeof resp.data.message).toBe('string');
    });

    test('list_providers_still_valid_after_refresh', async () => {
      const auth = bearerAuthHeader(ADMIN_KEY);
      const refreshResp = await catalogPost('/refresh', auth);
      expect(refreshResp.status).toBe(200);
      const listResp = await catalogGet('/providers', auth);
      expect(listResp.status).toBe(200);
      expect(Array.isArray(listResp.data)).toBe(true);
    });
  });
});
