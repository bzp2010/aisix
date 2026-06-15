// ── Shared response wrappers ──────────────────────────────────────────────────
export interface ListResponse<T> {
  total: number;
  list: Array<ItemResponse<T>>;
}

export interface ItemResponse<T> {
  key: string;
  value: T;
  created_index?: number;
  modified_index?: number;
}

export interface DeleteResponse {
  deleted: number;
  key: string;
}

export interface ApiError {
  error_msg: string;
}

// ── RateLimit ─────────────────────────────────────────────────────────────────
export interface RateLimit {
  tpm?: number;
  tpd?: number;
  rpm?: number;
  rpd?: number;
  concurrency?: number;
}

export interface Model {
  name: string;
  provider_id: string;
  model: string;
  timeout?: number;
  rate_limit?: RateLimit;
}

export const GUARDRAIL_TYPE_VARIANTS = ['regex', 'bedrock'] as const;

export type GuardrailType = (typeof GUARDRAIL_TYPE_VARIANTS)[number];

export interface RegexGuardrailConfig {
  pattern: string;
  block_reason?: string;
}

export interface BedrockGuardrailConfig {
  identifier: string;
  version: string;
  region: string;
  access_key_id: string;
  secret_access_key: string;
  session_token?: string;
  endpoint?: string;
}

export type Guardrail =
  | {
      name: string;
      type: 'regex';
      config: RegexGuardrailConfig;
    }
  | {
      name: string;
      type: 'bedrock';
      config: BedrockGuardrailConfig;
    };

export const POLICY_STAGE_VARIANTS = ['input', 'output'] as const;

export type PolicyStage = (typeof POLICY_STAGE_VARIANTS)[number];

export interface GuardrailPolicyActionConfig {
  stages: PolicyStage[];
  guardrail_ids: string[];
}

export interface GuardrailPolicyAction {
  type: 'guardrail';
  config: GuardrailPolicyActionConfig;
}

export type PolicyAction = GuardrailPolicyAction;

export interface Policy {
  name: string;
  enabled: boolean;
  priority: number;
  when: string;
  actions: PolicyAction[];
}

export const PROVIDER_TYPE_VARIANTS = [
  'openai',
  'openrouter',
  'cohere',
  'fireworks-ai',
  'groq',
  'xai',
  'mistral',
  'modelscope',
  'modelscope-cn',
  'siliconflow',
  'siliconflow-cn',
  'stepfun',
  'moonshotai',
  'moonshotai-cn',
  'zhipuai',
  'azure',
  'anthropic',
  'gemini',
  'deepseek',
  'bedrock',
] as const;

export type ProviderType = (typeof PROVIDER_TYPE_VARIANTS)[number];

export interface ApiBaseProviderConfig {
  api_key: string;
  api_base?: string;
}

export interface AzureProviderConfig {
  api_key: string;
  api_base: string;
  api_version?: string;
}

export interface BedrockProviderConfig {
  region: string;
  access_key_id: string;
  secret_access_key: string;
  session_token?: string;
  endpoint?: string;
}

export type Provider =
  | {
      name: string;
      type: 'anthropic';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'azure';
      config: AzureProviderConfig;
    }
  | {
      name: string;
      type: 'deepseek';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'gemini';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'openai';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'openrouter';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'cohere';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'fireworks-ai';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'groq';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'xai';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'mistral';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'modelscope';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'modelscope-cn';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'siliconflow';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'siliconflow-cn';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'stepfun';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'moonshotai';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'moonshotai-cn';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'zhipuai';
      config: ApiBaseProviderConfig;
    }
  | {
      name: string;
      type: 'bedrock';
      config: BedrockProviderConfig;
    };

// ── models.dev Catalog ────────────────────────────────────────────────────────
export interface CatalogProvider {
  id: string;
  name: string;
  api?: string;
  doc?: string;
}

export interface CatalogModel {
  id: string;
  name: string;
}

// ── ApiKey ────────────────────────────────────────────────────────────────────
export interface ApiKey {
  key: string;
  allowed_models: string[];
  rate_limit?: RateLimit;
}
