import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { Trash2, X } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHeader } from '@/components/layout/page-header';
import { PolicyForm } from '@/components/policies/policy-form';
import { Button } from '@/components/ui/button';
import {
  useDeletePolicy,
  usePolicy,
  useUpdatePolicy,
} from '@/lib/queries/policies';

export const Route = createFileRoute('/_layout/policies/$id')({
  component: PolicyEditPage,
});

function PolicyEditPage() {
  const { t } = useTranslation();
  const { id } = Route.useParams();
  const navigate = useNavigate();
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const { data, isLoading, isError } = usePolicy(id);
  const updatePolicy = useUpdatePolicy(id);
  const deletePolicy = useDeletePolicy();

  async function handleDelete() {
    if (!confirm(t('policies.deleteConfirm', { id }))) return;

    setDeleteError(null);
    try {
      await deletePolicy.mutateAsync(id);
      navigate({ to: '/policies' });
    } catch (error) {
      setDeleteError(
        error instanceof Error ? error.message : t('policies.deleteFailed'),
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
        {t('policies.errorLoadSingle')}
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <PageHeader>
        <h1 className="flex-1 text-xl font-semibold">{t('policies.title')}</h1>
        <Button
          variant="ghost"
          size="icon"
          onClick={() => navigate({ to: '/policies' })}
          aria-label={t('common.close')}
        >
          <X className="h-5 w-5" />
        </Button>
      </PageHeader>

      <div className="flex-1 overflow-auto bg-muted/20 p-5">
        <div className="mx-auto max-w-4xl space-y-6">
          <div>
            <h2 className="text-base font-semibold">
              {t('policies.editTitle')}
            </h2>
            <p className="mt-1 font-mono text-xs text-muted-foreground">{id}</p>
          </div>

          {deleteError && (
            <p className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {deleteError}
            </p>
          )}

          <PolicyForm
            key={id}
            initial={data.value}
            onSubmit={async (payload) => {
              setDeleteError(null);
              await updatePolicy.mutateAsync(payload);
              navigate({ to: '/policies' });
            }}
            isPending={updatePolicy.isPending}
            error={updatePolicy.error?.message}
            onCancel={() => navigate({ to: '/policies' })}
            submitLabel={t('common.saveChanges')}
            extraActions={
              <Button
                type="button"
                variant="outline"
                onClick={handleDelete}
                disabled={deletePolicy.isPending}
              >
                <Trash2 className="mr-1.5 h-4 w-4 text-destructive" />
                {t('policies.deletePolicy')}
              </Button>
            }
          />
        </div>
      </div>
    </div>
  );
}
