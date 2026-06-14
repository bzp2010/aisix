import { Link } from '@tanstack/react-router';
import { Plus, RefreshCw, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import {
  POLICY_STAGE_VARIANTS,
  type Guardrail,
  type Policy,
  type PolicyStage,
} from '@/lib/api/types';
import { useGuardrails } from '@/lib/queries/guardrails';

export interface PolicyFormProps {
  initial?: Policy;
  onSubmit: (data: Policy) => void | Promise<void>;
  onCancel: () => void;
  isPending: boolean;
  error?: string;
  submitLabel: string;
  extraActions?: React.ReactNode;
}

type PolicyActionDraft = {
  stages: PolicyStage[];
  guardrail_ids: string[];
};

type PolicyFormState = {
  name: string;
  enabled: boolean;
  priority: string;
  when: string;
  actions: PolicyActionDraft[];
};

type GuardrailOption = {
  id: string;
  name?: string;
  type?: Guardrail['type'];
  exists: boolean;
};

const POLICY_STAGES = Array.from(POLICY_STAGE_VARIANTS);

function createEmptyAction(): PolicyActionDraft {
  return {
    stages: [...POLICY_STAGES],
    guardrail_ids: [],
  };
}

function normalizeStages(stages: PolicyStage[] | undefined): PolicyStage[] {
  const selected = new Set(stages ?? POLICY_STAGES);
  return POLICY_STAGES.filter((stage) => selected.has(stage));
}

function buildInitialState(initial?: Policy): PolicyFormState {
  return {
    name: initial?.name ?? '',
    enabled: initial?.enabled ?? true,
    priority: String(initial?.priority ?? 0),
    when: initial?.when ?? '',
    actions:
      initial && initial.actions.length > 0
        ? initial.actions.map((action) => ({
            stages: normalizeStages(action.config.stages),
            guardrail_ids: [...action.config.guardrail_ids],
          }))
        : [createEmptyAction()],
  };
}

function parsePriority(raw: string): number | undefined {
  const trimmed = raw.trim();
  if (!trimmed) {
    return 0;
  }

  if (!/^-?\d+$/.test(trimmed)) {
    return undefined;
  }

  return Number(trimmed);
}

export function PolicyForm({
  initial,
  onSubmit,
  onCancel,
  isPending,
  error,
  submitLabel,
  extraActions,
}: PolicyFormProps) {
  const { t } = useTranslation();
  const [clientError, setClientError] = useState<string>();
  const [state, setState] = useState(() => buildInitialState(initial));
  const guardrailsQuery = useGuardrails();

  const guardrailOptionsById = new Map<string, GuardrailOption>();
  for (const { key, value } of guardrailsQuery.data?.list ?? []) {
    const id = key.replace('/guardrails/', '');
    guardrailOptionsById.set(id, {
      id,
      name: value.name,
      type: value.type,
      exists: true,
    });
  }

  for (const action of state.actions) {
    for (const guardrailId of action.guardrail_ids) {
      if (!guardrailOptionsById.has(guardrailId)) {
        guardrailOptionsById.set(guardrailId, {
          id: guardrailId,
          exists: false,
        });
      }
    }
  }

  const guardrailOptions = Array.from(guardrailOptionsById.values()).sort(
    (a, b) => a.id.localeCompare(b.id),
  );

  let guardrailHint = t('policies.form.guardrailsHint');
  if (guardrailsQuery.isLoading) {
    guardrailHint = t('policies.form.guardrailsLoading');
  } else if (guardrailsQuery.isError) {
    guardrailHint = t('policies.form.guardrailsLoadError');
  } else if (guardrailOptions.length === 0) {
    guardrailHint = t('policies.form.guardrailsEmpty');
  }

  function updateState(updater: (current: PolicyFormState) => PolicyFormState) {
    setClientError(undefined);
    setState((current) => updater(current));
  }

  function updateAction(
    index: number,
    updater: (current: PolicyActionDraft) => PolicyActionDraft,
  ) {
    updateState((current) => ({
      ...current,
      actions: current.actions.map((action, actionIndex) =>
        actionIndex === index ? updater(action) : action,
      ),
    }));
  }

  function handleToggleStage(
    index: number,
    stage: PolicyStage,
    checked: boolean,
  ) {
    updateAction(index, (action) => {
      const selected = new Set(action.stages);
      if (checked) {
        selected.add(stage);
      } else {
        selected.delete(stage);
      }

      return {
        ...action,
        stages: POLICY_STAGES.filter((item) => selected.has(item)),
      };
    });
  }

  function handleToggleGuardrail(
    index: number,
    guardrailId: string,
    checked: boolean,
  ) {
    updateAction(index, (action) => ({
      ...action,
      guardrail_ids: checked
        ? action.guardrail_ids.includes(guardrailId)
          ? action.guardrail_ids
          : [...action.guardrail_ids, guardrailId]
        : action.guardrail_ids.filter((id) => id !== guardrailId),
    }));
  }

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const name = state.name.trim();
    if (!name) {
      setClientError(t('policies.form.nameRequired'));
      return;
    }

    const when = state.when.trim();
    if (!when) {
      setClientError(t('policies.form.whenRequired'));
      return;
    }

    const priority = parsePriority(state.priority);
    if (priority == null) {
      setClientError(t('policies.form.priorityInvalid'));
      return;
    }

    if (state.actions.length === 0) {
      setClientError(t('policies.form.actionsRequired'));
      return;
    }

    if (state.actions.some((action) => action.stages.length === 0)) {
      setClientError(t('policies.form.stagesRequired'));
      return;
    }

    if (state.actions.some((action) => action.guardrail_ids.length === 0)) {
      setClientError(t('policies.form.guardrailsRequired'));
      return;
    }

    setClientError(undefined);
    await onSubmit({
      name,
      enabled: state.enabled,
      priority,
      when,
      actions: state.actions.map((action) => ({
        type: 'guardrail',
        config: {
          stages: action.stages,
          guardrail_ids: action.guardrail_ids,
        },
      })),
    });
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-5">
      <section className="space-y-4 rounded-xl border bg-card p-5">
        <h3 className="text-sm font-semibold">
          {t('policies.form.basicInfo')}
        </h3>

        <div className="grid gap-4 md:grid-cols-2">
          <Field label={t('policies.form.nameLabel')} className="md:col-span-2">
            <Input
              required
              value={state.name}
              onChange={(event) => {
                updateState((current) => ({
                  ...current,
                  name: event.target.value,
                }));
              }}
              placeholder={t('policies.form.namePlaceholder')}
            />
          </Field>

          <Field label={t('policies.form.enabledLabel')}>
            <label className="flex min-h-10 items-center gap-3 rounded-lg border bg-background px-3 py-2">
              <Checkbox
                checked={state.enabled}
                onCheckedChange={(checked) => {
                  updateState((current) => ({
                    ...current,
                    enabled: checked === true,
                  }));
                }}
              />
              <span className="text-sm">
                {state.enabled ? t('policies.enabled') : t('policies.disabled')}
              </span>
            </label>
          </Field>

          <Field
            label={t('policies.form.priorityLabel')}
            hint={t('policies.form.priorityHint')}
          >
            <Input
              type="number"
              step="1"
              value={state.priority}
              onChange={(event) => {
                updateState((current) => ({
                  ...current,
                  priority: event.target.value,
                }));
              }}
              placeholder="0"
            />
          </Field>

          <Field label={t('policies.form.whenLabel')} className="md:col-span-2">
            <Textarea
              required
              className="min-h-24 font-mono text-sm"
              value={state.when}
              onChange={(event) => {
                updateState((current) => ({
                  ...current,
                  when: event.target.value,
                }));
              }}
              placeholder={t('policies.form.whenPlaceholder')}
            />
          </Field>
        </div>
      </section>

      <section className="space-y-4 rounded-xl border bg-card p-5">
        <div className="flex items-center justify-between gap-3">
          <h3 className="text-sm font-semibold">
            {t('policies.form.actions')}
          </h3>
          <Button
            type="button"
            variant="outline"
            onClick={() => {
              updateState((current) => ({
                ...current,
                actions: [...current.actions, createEmptyAction()],
              }));
            }}
          >
            <Plus className="mr-1.5 h-4 w-4" />
            {t('policies.form.addAction')}
          </Button>
        </div>

        <div className="space-y-4">
          {state.actions.map((action, index) => (
            <div
              key={`action-${index}`}
              className="space-y-4 rounded-xl border bg-muted/20 p-4"
            >
              <div className="flex items-center justify-between gap-3">
                <div className="flex items-center gap-2">
                  <h4 className="text-sm font-semibold">
                    {t('policies.form.actionTitle', { index: index + 1 })}
                  </h4>
                  <Badge variant="secondary">
                    {t('policies.form.guardrailAction')}
                  </Badge>
                </div>

                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    if (state.actions.length <= 1) {
                      return;
                    }

                    updateState((current) => ({
                      ...current,
                      actions: current.actions.filter(
                        (_, actionIndex) => actionIndex !== index,
                      ),
                    }));
                  }}
                  disabled={state.actions.length <= 1}
                >
                  <Trash2 className="mr-1.5 h-4 w-4 text-destructive" />
                  {t('policies.form.removeAction')}
                </Button>
              </div>

              <Field label={t('policies.form.stagesLabel')}>
                <div className="flex flex-wrap gap-3">
                  {POLICY_STAGES.map((stage) => (
                    <label
                      key={stage}
                      className="flex min-h-10 items-center gap-3 rounded-lg border bg-background px-3 py-2"
                    >
                      <Checkbox
                        checked={action.stages.includes(stage)}
                        onCheckedChange={(checked) => {
                          handleToggleStage(index, stage, checked === true);
                        }}
                      />
                      <span className="text-sm">
                        {t(`policyStages.${stage}`)}
                      </span>
                    </label>
                  ))}
                </div>
              </Field>

              <Field
                label={t('policies.form.guardrailsLabel')}
                hint={guardrailHint}
              >
                <div className="mb-3 flex flex-wrap gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => {
                      void guardrailsQuery.refetch();
                    }}
                    disabled={guardrailsQuery.isFetching}
                  >
                    <RefreshCw
                      className={
                        guardrailsQuery.isFetching
                          ? 'mr-1.5 h-4 w-4 animate-spin'
                          : 'mr-1.5 h-4 w-4'
                      }
                    />
                    {t('policies.form.guardrailsRefresh')}
                  </Button>

                  <Button asChild variant="outline" size="sm">
                    <Link
                      to="/guardrails/create"
                      target="_blank"
                      rel="noreferrer"
                    >
                      <Plus className="mr-1.5 h-4 w-4" />
                      {t('policies.form.guardrailsCreate')}
                    </Link>
                  </Button>
                </div>

                {guardrailOptions.length === 0 ? (
                  <p className="rounded-md border border-dashed px-3 py-2 text-sm text-muted-foreground">
                    {guardrailHint}
                  </p>
                ) : (
                  <div className="grid gap-2 md:grid-cols-2">
                    {guardrailOptions.map((option) => (
                      <label
                        key={option.id}
                        className="flex items-start gap-3 rounded-lg border bg-background px-3 py-3"
                      >
                        <Checkbox
                          checked={action.guardrail_ids.includes(option.id)}
                          onCheckedChange={(checked) => {
                            handleToggleGuardrail(
                              index,
                              option.id,
                              checked === true,
                            );
                          }}
                        />

                        <div className="min-w-0 space-y-1">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="font-mono text-xs text-muted-foreground">
                              {option.id}
                            </span>
                            {option.type && (
                              <Badge variant="secondary">
                                {t(`guardrailTypes.${option.type}`)}
                              </Badge>
                            )}
                            {!option.exists && (
                              <Badge variant="destructive">
                                {t('policies.form.guardrailsMissing')}
                              </Badge>
                            )}
                          </div>

                          <p className="text-sm font-medium">
                            {option.name ?? option.id}
                          </p>

                          {!option.exists && (
                            <p className="text-xs text-destructive">
                              {t('policies.form.guardrailsMissingHint')}
                            </p>
                          )}
                        </div>
                      </label>
                    ))}
                  </div>
                )}

                {guardrailOptions.length > 0 &&
                  action.guardrail_ids.length === 0 && (
                    <p className="mt-2 text-xs text-muted-foreground">
                      {t('policies.form.guardrailsNoSelection')}
                    </p>
                  )}
              </Field>
            </div>
          ))}
        </div>
      </section>

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
          <Button type="submit" disabled={isPending}>
            {isPending ? t('common.saving') : submitLabel}
          </Button>
        </div>
      </div>
    </form>
  );
}

function Field({
  label,
  hint,
  className,
  children,
}: {
  label: string;
  hint?: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div className={className ? `space-y-1.5 ${className}` : 'space-y-1.5'}>
      <Label className="text-xs font-medium">{label}</Label>
      {children}
      {hint && <p className="text-xs text-muted-foreground">{hint}</p>}
    </div>
  );
}
