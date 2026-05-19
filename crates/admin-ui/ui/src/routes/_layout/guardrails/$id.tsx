import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { Trash2, X } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { GuardrailForm } from '@/components/guardrails/guardrail-form';
import { PageHeader } from '@/components/layout/page-header';
import { Button } from '@/components/ui/button';
import {
  useDeleteGuardrail,
  useGuardrail,
  useUpdateGuardrail,
} from '@/lib/queries/guardrails';

export const Route = createFileRoute('/_layout/guardrails/$id')({
  component: GuardrailEditPage,
});

function GuardrailEditPage() {
  const { t } = useTranslation();
  const { id } = Route.useParams();
  const navigate = useNavigate();
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const { data, isLoading, isError } = useGuardrail(id);
  const updateGuardrail = useUpdateGuardrail(id);
  const deleteGuardrail = useDeleteGuardrail();

  async function handleDelete() {
    if (!confirm(t('guardrails.deleteConfirm', { id }))) return;

    setDeleteError(null);
    try {
      await deleteGuardrail.mutateAsync(id);
      navigate({ to: '/guardrails' });
    } catch (error) {
      setDeleteError(
        error instanceof Error ? error.message : t('guardrails.deleteFailed'),
      );
    }
  }

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        {t('common.loading')}
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-destructive">
        {t('guardrails.errorLoadSingle')}
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <PageHeader>
        <h1 className="flex-1 text-xl font-semibold">
          {t('guardrails.title')}
        </h1>
        <Button
          variant="ghost"
          size="icon"
          onClick={() => navigate({ to: '/guardrails' })}
          aria-label={t('common.close')}
        >
          <X className="h-5 w-5" />
        </Button>
      </PageHeader>

      <div className="flex-1 overflow-auto bg-muted/20 p-5">
        <div className="mx-auto max-w-3xl space-y-6">
          <div>
            <h2 className="text-base font-semibold">
              {t('guardrails.editTitle')}
            </h2>
            <p className="mt-1 font-mono text-xs text-muted-foreground">{id}</p>
          </div>

          {deleteError && (
            <p className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {deleteError}
            </p>
          )}

          <GuardrailForm
            initial={data.value}
            onSubmit={async (payload) => {
              setDeleteError(null);
              await updateGuardrail.mutateAsync(payload);
              navigate({ to: '/guardrails' });
            }}
            isPending={updateGuardrail.isPending}
            error={updateGuardrail.error?.message}
            onCancel={() => navigate({ to: '/guardrails' })}
            submitLabel={t('common.saveChanges')}
            extraActions={
              <Button
                type="button"
                variant="outline"
                onClick={handleDelete}
                disabled={deleteGuardrail.isPending}
              >
                <Trash2 className="mr-1.5 h-4 w-4 text-destructive" />
                {t('guardrails.deleteGuardrail')}
              </Button>
            }
          />
        </div>
      </div>
    </div>
  );
}
