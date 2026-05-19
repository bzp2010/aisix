import { useForm } from '@tanstack/react-form';
import { Eye, EyeOff } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from '@/components/ui/input-group';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import {
  GUARDRAIL_TYPE_VARIANTS,
  type Guardrail,
  type GuardrailType,
} from '@/lib/api/types';

export interface GuardrailFormProps {
  initial?: Guardrail;
  onSubmit: (data: Guardrail) => void | Promise<void>;
  onCancel: () => void;
  isPending: boolean;
  error?: string;
  submitLabel: string;
  extraActions?: React.ReactNode;
}

const GUARDRAIL_TYPES = Array.from(GUARDRAIL_TYPE_VARIANTS);

function trimOptional(value: string): string | undefined {
  const trimmed = value.trim();
  return trimmed ? trimmed : undefined;
}

export function GuardrailForm({
  initial,
  onSubmit,
  onCancel,
  isPending,
  error,
  submitLabel,
  extraActions,
}: GuardrailFormProps) {
  const { t } = useTranslation();
  const [clientError, setClientError] = useState<string>();

  const form = useForm({
    defaultValues: {
      name: initial?.name ?? '',
      type: initial?.type ?? ('regex' as GuardrailType),
      pattern: initial?.type === 'regex' ? initial.config.pattern : '',
      block_reason:
        initial?.type === 'regex' ? (initial.config.block_reason ?? '') : '',
      identifier: initial?.type === 'bedrock' ? initial.config.identifier : '',
      version: initial?.type === 'bedrock' ? initial.config.version : '',
      region: initial?.type === 'bedrock' ? initial.config.region : '',
      access_key_id:
        initial?.type === 'bedrock' ? initial.config.access_key_id : '',
      secret_access_key:
        initial?.type === 'bedrock' ? initial.config.secret_access_key : '',
      session_token:
        initial?.type === 'bedrock' ? (initial.config.session_token ?? '') : '',
      endpoint:
        initial?.type === 'bedrock' ? (initial.config.endpoint ?? '') : '',
    },
    onSubmit: async ({ value }) => {
      const name = value.name.trim();
      if (!name) {
        setClientError(t('guardrails.form.nameRequired'));
        return;
      }

      if (value.type === 'regex') {
        const pattern = value.pattern.trim();
        if (!pattern) {
          setClientError(t('guardrails.form.patternRequired'));
          return;
        }

        setClientError(undefined);
        await onSubmit({
          name,
          type: 'regex',
          config: {
            pattern,
            ...(trimOptional(value.block_reason)
              ? { block_reason: trimOptional(value.block_reason) }
              : {}),
          },
        });
        return;
      }

      const identifier = value.identifier.trim();
      const version = value.version.trim();
      const region = value.region.trim();
      const accessKeyId = value.access_key_id.trim();
      const secretAccessKey = value.secret_access_key.trim();

      if (!identifier) {
        setClientError(t('guardrails.form.identifierRequired'));
        return;
      }

      if (!version) {
        setClientError(t('guardrails.form.versionRequired'));
        return;
      }

      if (!region) {
        setClientError(t('guardrails.form.regionRequired'));
        return;
      }

      if (!accessKeyId) {
        setClientError(t('guardrails.form.accessKeyIdRequired'));
        return;
      }

      if (!secretAccessKey) {
        setClientError(t('guardrails.form.secretAccessKeyRequired'));
        return;
      }

      setClientError(undefined);
      await onSubmit({
        name,
        type: 'bedrock',
        config: {
          identifier,
          version,
          region,
          access_key_id: accessKeyId,
          secret_access_key: secretAccessKey,
          ...(trimOptional(value.session_token)
            ? { session_token: trimOptional(value.session_token) }
            : {}),
          ...(trimOptional(value.endpoint)
            ? { endpoint: trimOptional(value.endpoint) }
            : {}),
        },
      });
    },
  });

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        e.stopPropagation();
        form.handleSubmit();
      }}
      className="space-y-5"
    >
      <section className="space-y-4 rounded-xl border bg-card p-5">
        <h3 className="text-sm font-semibold">
          {t('guardrails.form.basicInfo')}
        </h3>

        <div className="grid gap-4 md:grid-cols-2">
          <form.Field name="name">
            {(field) => (
              <Field label={t('guardrails.form.nameLabel')}>
                <Input
                  required
                  value={field.state.value}
                  onChange={(e) => {
                    setClientError(undefined);
                    field.handleChange(e.target.value);
                  }}
                  onBlur={field.handleBlur}
                  placeholder={t('guardrails.form.namePlaceholder')}
                />
              </Field>
            )}
          </form.Field>

          <form.Field name="type">
            {(field) => (
              <Field label={t('guardrails.form.typeLabel')}>
                <Select
                  value={field.state.value}
                  onValueChange={(next) => {
                    setClientError(undefined);
                    field.handleChange(next as GuardrailType);
                  }}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue
                      placeholder={t('guardrails.form.typePlaceholder')}
                    />
                  </SelectTrigger>
                  <SelectContent align="start" position="popper">
                    {GUARDRAIL_TYPES.map((guardrailType) => (
                      <SelectItem key={guardrailType} value={guardrailType}>
                        {t(`guardrailTypes.${guardrailType}`)}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
            )}
          </form.Field>
        </div>
      </section>

      <form.Subscribe selector={(state) => state.values.type}>
        {(guardrailType) => (
          <section className="space-y-4 rounded-xl border bg-card p-5">
            <h3 className="text-sm font-semibold">
              {t('guardrails.form.config')}
            </h3>

            {guardrailType === 'regex' ? (
              <div className="grid gap-4 md:grid-cols-2">
                <form.Field name="pattern">
                  {(field) => (
                    <Field label={t('guardrails.form.patternLabel')}>
                      <Input
                        required
                        value={field.state.value}
                        onChange={(e) => {
                          setClientError(undefined);
                          field.handleChange(e.target.value);
                        }}
                        onBlur={field.handleBlur}
                        placeholder={t('guardrails.form.patternPlaceholder')}
                      />
                    </Field>
                  )}
                </form.Field>

                <form.Field name="block_reason">
                  {(field) => (
                    <Field label={t('guardrails.form.blockReasonLabel')}>
                      <Input
                        value={field.state.value}
                        onChange={(e) => field.handleChange(e.target.value)}
                        onBlur={field.handleBlur}
                        placeholder={t(
                          'guardrails.form.blockReasonPlaceholder',
                        )}
                      />
                    </Field>
                  )}
                </form.Field>
              </div>
            ) : (
              <div className="grid gap-4 md:grid-cols-2">
                <form.Field name="identifier">
                  {(field) => (
                    <Field label={t('guardrails.form.identifierLabel')}>
                      <Input
                        required
                        value={field.state.value}
                        onChange={(e) => {
                          setClientError(undefined);
                          field.handleChange(e.target.value);
                        }}
                        onBlur={field.handleBlur}
                        placeholder={t('guardrails.form.identifierPlaceholder')}
                      />
                    </Field>
                  )}
                </form.Field>

                <form.Field name="version">
                  {(field) => (
                    <Field label={t('guardrails.form.versionLabel')}>
                      <Input
                        required
                        value={field.state.value}
                        onChange={(e) => {
                          setClientError(undefined);
                          field.handleChange(e.target.value);
                        }}
                        onBlur={field.handleBlur}
                        placeholder={t('guardrails.form.versionPlaceholder')}
                      />
                    </Field>
                  )}
                </form.Field>

                <form.Field name="region">
                  {(field) => (
                    <Field
                      label={t('guardrails.form.regionLabel')}
                      hint={t('guardrails.form.regionHint')}
                    >
                      <Input
                        required
                        value={field.state.value}
                        onChange={(e) => {
                          setClientError(undefined);
                          field.handleChange(e.target.value);
                        }}
                        onBlur={field.handleBlur}
                        placeholder="us-east-1"
                      />
                    </Field>
                  )}
                </form.Field>

                <form.Field name="endpoint">
                  {(field) => (
                    <Field
                      label={t('guardrails.form.endpointLabel')}
                      hint={t('guardrails.form.endpointHint')}
                    >
                      <Input
                        value={field.state.value}
                        onChange={(e) => field.handleChange(e.target.value)}
                        onBlur={field.handleBlur}
                        placeholder="https://bedrock-runtime.us-east-1.amazonaws.com"
                      />
                    </Field>
                  )}
                </form.Field>

                <form.Field name="access_key_id">
                  {(field) => (
                    <Field label={t('guardrails.form.accessKeyIdLabel')}>
                      <Input
                        required
                        value={field.state.value}
                        onChange={(e) => {
                          setClientError(undefined);
                          field.handleChange(e.target.value);
                        }}
                        onBlur={field.handleBlur}
                        placeholder="AKIA..."
                        autoComplete="off"
                      />
                    </Field>
                  )}
                </form.Field>

                <form.Field name="secret_access_key">
                  {(field) => (
                    <Field label={t('guardrails.form.secretAccessKeyLabel')}>
                      <SecretInput
                        required
                        value={field.state.value}
                        onChange={(e) => {
                          setClientError(undefined);
                          field.handleChange(e.target.value);
                        }}
                        onBlur={field.handleBlur}
                        placeholder={t(
                          'guardrails.form.secretAccessKeyPlaceholder',
                        )}
                        autoComplete="new-password"
                        showLabel={t('guardrails.form.showSecret')}
                        hideLabel={t('guardrails.form.hideSecret')}
                      />
                    </Field>
                  )}
                </form.Field>

                <form.Field name="session_token">
                  {(field) => (
                    <Field
                      label={t('guardrails.form.sessionTokenLabel')}
                      hint={t('guardrails.form.sessionTokenHint')}
                    >
                      <Input
                        value={field.state.value}
                        onChange={(e) => field.handleChange(e.target.value)}
                        onBlur={field.handleBlur}
                        placeholder={t(
                          'guardrails.form.sessionTokenPlaceholder',
                        )}
                        autoComplete="off"
                      />
                    </Field>
                  )}
                </form.Field>
              </div>
            )}
          </section>
        )}
      </form.Subscribe>

      {(clientError ?? error) && (
        <p className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {clientError ?? error}
        </p>
      )}

      <div className="flex items-center justify-between">
        {extraActions ?? <span />}
        <div className="flex gap-2">
          <Button type="button" variant="outline" onClick={onCancel}>
            {t('common.cancel')}
          </Button>
          <form.Subscribe selector={(state) => state.isSubmitting}>
            {(isSubmitting) => (
              <Button type="submit" disabled={isSubmitting || isPending}>
                {isSubmitting || isPending ? t('common.saving') : submitLabel}
              </Button>
            )}
          </form.Subscribe>
        </div>
      </div>
    </form>
  );
}

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <Label className="text-xs font-medium">{label}</Label>
      {children}
      {hint && <p className="text-xs text-muted-foreground">{hint}</p>}
    </div>
  );
}

type SecretInputProps = Omit<
  React.ComponentProps<typeof InputGroupInput>,
  'type'
> & {
  showLabel: string;
  hideLabel: string;
};

function SecretInput({
  showLabel,
  hideLabel,
  disabled,
  ...props
}: SecretInputProps) {
  const [isVisible, setIsVisible] = useState(false);

  return (
    <InputGroup>
      <InputGroupInput
        {...props}
        disabled={disabled}
        type={isVisible ? 'text' : 'password'}
      />
      <InputGroupAddon align="inline-end" className="pr-1">
        <Tooltip>
          <TooltipTrigger asChild>
            <InputGroupButton
              type="button"
              variant="ghost"
              size="icon-sm"
              disabled={disabled}
              aria-label={isVisible ? hideLabel : showLabel}
              onClick={() => setIsVisible((visible) => !visible)}
            >
              {isVisible ? <EyeOff /> : <Eye />}
            </InputGroupButton>
          </TooltipTrigger>
          <TooltipContent>
            <p>{isVisible ? hideLabel : showLabel}</p>
          </TooltipContent>
        </Tooltip>
      </InputGroupAddon>
    </InputGroup>
  );
}
