import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { X } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { GuardrailForm } from '@/components/guardrails/guardrail-form';
import { PageHeader } from '@/components/layout/page-header';
import { Button } from '@/components/ui/button';
import type { Guardrail } from '@/lib/api/types';
import { useCreateGuardrail } from '@/lib/queries/guardrails';

export const Route = createFileRoute('/_layout/guardrails/create')({
  component: GuardrailCreatePage,
});

function GuardrailCreatePage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const createGuardrail = useCreateGuardrail();

  async function handleSubmit(data: Guardrail) {
    await createGuardrail.mutateAsync(data);
    navigate({ to: '/guardrails' });
  }

  return (
    <div className="flex h-full flex-col">
      <PageHeader>
        <h1 className="flex-1 text-xl font-semibold">{t('guardrails.title')}</h1>
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
              {t('guardrails.createTitle')}
            </h2>
            <p className="mt-1 text-sm text-muted-foreground">
              {t('guardrails.createDesc')}
            </p>
          </div>

          <GuardrailForm
            onSubmit={handleSubmit}
            isPending={createGuardrail.isPending}
            error={createGuardrail.error?.message}
            onCancel={() => navigate({ to: '/guardrails' })}
            submitLabel={t('guardrails.createGuardrail')}
          />
        </div>
      </div>
    </div>
  );
}