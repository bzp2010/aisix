import { createFileRoute } from '@tanstack/react-router';
import { RefreshCw } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHeader } from '@/components/layout/page-header';
import { Button } from '@/components/ui/button';
import { useRefreshCatalog } from '@/lib/queries/catalog';

export const Route = createFileRoute('/_layout/settings')({
  component: SettingsPage,
});

function SettingsPage() {
  const { t } = useTranslation();
  const refreshCatalog = useRefreshCatalog();
  const [refreshMessage, setRefreshMessage] = useState<{
    type: 'success' | 'error';
    text: string;
  } | null>(null);

  async function handleCatalogRefresh() {
    setRefreshMessage(null);
    try {
      await refreshCatalog.mutateAsync();
      setRefreshMessage({ type: 'success', text: t('settings.data.catalogRefreshSuccess') });
    } catch {
      setRefreshMessage({ type: 'error', text: t('settings.data.catalogRefreshError') });
    }
  }

  return (
    <div className="flex h-full flex-col">
      <PageHeader>
        <h1 className="flex-1 text-xl font-semibold">{t('settings.title')}</h1>
      </PageHeader>

      <div className="flex-1 overflow-auto bg-muted/20 p-5">
        <div className="mx-auto max-w-3xl space-y-4">
          <section className="rounded-xl border bg-card">
            <div className="border-b px-5 py-3">
              <h2 className="text-sm font-semibold">{t('settings.data.title')}</h2>
            </div>

            <div className="flex items-center justify-between px-5 py-4">
              <div className="space-y-0.5">
                <p className="text-sm font-medium">{t('settings.data.catalogTitle')}</p>
                <p className="text-xs text-muted-foreground">
                  {t('settings.data.catalogDescription')}
                </p>
                {refreshMessage && (
                  <p
                    className={`text-xs ${
                      refreshMessage.type === 'success'
                        ? 'text-green-600 dark:text-green-400'
                        : 'text-destructive'
                    }`}
                  >
                    {refreshMessage.text}
                  </p>
                )}
              </div>
              <Button
                variant="outline"
                size="sm"
                onClick={() => void handleCatalogRefresh()}
                disabled={refreshCatalog.isPending}
                className="shrink-0"
              >
                <RefreshCw
                  className={refreshCatalog.isPending ? 'animate-spin' : undefined}
                />
                {refreshCatalog.isPending
                  ? t('settings.data.catalogRefreshing')
                  : t('settings.data.catalogRefresh')}
              </Button>
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
