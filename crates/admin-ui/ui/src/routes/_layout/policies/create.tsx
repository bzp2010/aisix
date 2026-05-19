import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { X } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { PageHeader } from '@/components/layout/page-header';
import { PolicyForm } from '@/components/policies/policy-form';
import { Button } from '@/components/ui/button';
import type { Policy } from '@/lib/api/types';
import { useCreatePolicy } from '@/lib/queries/policies';

export const Route = createFileRoute('/_layout/policies/create')({
  component: PolicyCreatePage,
});

function PolicyCreatePage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const createPolicy = useCreatePolicy();

  async function handleSubmit(data: Policy) {
    await createPolicy.mutateAsync(data);
    navigate({ to: '/policies' });
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
              {t('policies.createTitle')}
            </h2>
            <p className="mt-1 text-sm text-muted-foreground">
              {t('policies.createDesc')}
            </p>
          </div>

          <PolicyForm
            onSubmit={handleSubmit}
            isPending={createPolicy.isPending}
            error={createPolicy.error?.message}
            onCancel={() => navigate({ to: '/policies' })}
            submitLabel={t('policies.createPolicy')}
          />
        </div>
      </div>
    </div>
  );
}
