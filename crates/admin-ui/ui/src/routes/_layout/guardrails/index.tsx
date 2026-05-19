import { Link, createFileRoute, useNavigate } from '@tanstack/react-router';
import { createColumnHelper, type ColumnDef } from '@tanstack/react-table';
import { Pencil, Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHeader } from '@/components/layout/page-header';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { DataTable, type RowSelectionState } from '@/components/ui/data-table';
import { Input } from '@/components/ui/input';
import type { Guardrail, ItemResponse } from '@/lib/api/types';
import { useDeleteGuardrail, useGuardrails } from '@/lib/queries/guardrails';

export const Route = createFileRoute('/_layout/guardrails/')({
  component: GuardrailsPage,
});

type GuardrailRow = ItemResponse<Guardrail> & {
  id: string;
  summary: string;
};

const col = createColumnHelper<GuardrailRow>();

function GuardrailsPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data, isLoading, isError } = useGuardrails();
  const deleteGuardrail = useDeleteGuardrail();

  const [search, setSearch] = useState('');
  const [rowSelection, setRowSelection] = useState<RowSelectionState>({});
  const [actionError, setActionError] = useState<string | null>(null);

  const items = (data?.list ?? []).map(
    ({ key, value, ...rest }) =>
      ({
        id: key.replace('/guardrails/', ''),
        key,
        value,
        summary: describeGuardrailConfig(value, t),
        ...rest,
      }) as GuardrailRow,
  );
  const selectedKeys = Object.keys(rowSelection);

  function formatDeleteError(error: unknown) {
    return error instanceof Error ? error.message : t('guardrails.deleteFailed');
  }

  async function handleDeleteOne(id: string) {
    setActionError(null);
    try {
      await deleteGuardrail.mutateAsync(id);
    } catch (error) {
      setActionError(formatDeleteError(error));
    }
  }

  const columns = [
    col.display({
      id: 'select',
      size: 40,
      header: ({ table }) => (
        <Checkbox
          checked={
            table.getIsAllPageRowsSelected() ||
            (table.getIsSomePageRowsSelected() && 'indeterminate')
          }
          onCheckedChange={(value) => table.toggleAllPageRowsSelected(!!value)}
        />
      ),
      cell: ({ row }) => (
        <Checkbox
          checked={row.getIsSelected()}
          onCheckedChange={(value) => row.toggleSelected(!!value)}
        />
      ),
    }),
    col.accessor('id', {
      header: () => t('guardrails.columns.id'),
      size: 280,
      cell: (info) => (
        <span className="font-mono text-xs text-muted-foreground">
          {info.getValue()}
        </span>
      ),
    }),
    col.accessor('value.name', {
      header: () => t('guardrails.columns.name'),
      size: 240,
      cell: (info) => (
        <span className="text-sm font-medium">{info.getValue()}</span>
      ),
    }),
    col.accessor('value.type', {
      header: () => t('guardrails.columns.type'),
      size: 140,
      cell: (info) => (
        <Badge variant="secondary">
          {t(`guardrailTypes.${info.getValue()}`)}
        </Badge>
      ),
    }),
    col.accessor('summary', {
      header: () => t('guardrails.columns.config'),
      cell: (info) => (
        <span className="text-sm text-muted-foreground">{info.getValue()}</span>
      ),
    }),
    col.display({
      id: 'actions',
      size: 96,
      header: () => (
        <div className="text-right">{t('guardrails.columns.action')}</div>
      ),
      cell: ({ row }) => (
        <div className="flex justify-end">
          <Button
            variant="ghost"
            size="icon-lg"
            onClick={() =>
              navigate({
                to: '/guardrails/$id',
                params: { id: row.original.id },
              })
            }
          >
            <Pencil />
          </Button>
          <Button
            variant="destructive"
            size="icon-lg"
            onClick={() => {
              void handleDeleteOne(row.original.id);
            }}
          >
            <Trash2 />
          </Button>
        </div>
      ),
    }),
  ];

  async function handleDeleteSelected() {
    setActionError(null);

    const results = await Promise.allSettled(
      selectedKeys.map(async (id) => {
        await deleteGuardrail.mutateAsync(id);
        return id;
      }),
    );

    const failed = results.flatMap((result, index) =>
      result.status === 'rejected'
        ? [{ id: selectedKeys[index], message: formatDeleteError(result.reason) }]
        : [],
    );

    if (failed.length > 0) {
      const successCount = selectedKeys.length - failed.length;
      setActionError(
        t('guardrails.bulkDeleteFailed', {
          successCount,
          failureCount: failed.length,
          details: failed.map(({ id, message }) => `${id}: ${message}`).join('; '),
        }),
      );
      setRowSelection(
        Object.fromEntries(failed.map(({ id }) => [id, true])) satisfies RowSelectionState,
      );
      return;
    }

    setRowSelection({});
  }

  return (
    <div className="flex h-full flex-col">
      <PageHeader>
        <h1 className="flex-1 text-xl font-semibold">
          {t('guardrails.title')}
        </h1>
        {selectedKeys.length > 0 && (
          <Button
            variant="outline"
            size="lg"
            onClick={handleDeleteSelected}
            disabled={deleteGuardrail.isPending}
          >
            <Trash2 className="mr-1.5 h-4 w-4 text-destructive" />
            {t('common.delete')} ({selectedKeys.length})
          </Button>
        )}
        <Button asChild size="lg">
          <Link to="/guardrails/create">
            <Plus className="mr-1.5 h-4 w-4" />
            {t('guardrails.addGuardrail')}
          </Link>
        </Button>
      </PageHeader>

      <div className="flex-1 overflow-auto p-5">
        {actionError && (
          <p className="mb-4 rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
            {actionError}
          </p>
        )}

        <div className="mb-4 flex items-center justify-between">
          <p className="text-sm text-muted-foreground">
            {isLoading
              ? t('common.loading')
              : t('guardrails.count', { count: items.length })}
          </p>
          <Input
            className="w-80"
            placeholder={t('guardrails.search')}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>

        <DataTable
          columns={columns as ColumnDef<GuardrailRow>[]}
          data={items}
          isLoading={isLoading}
          isError={isError}
          errorMessage={t('guardrails.errorLoad')}
          emptyMessage={
            <span>
              {t('guardrails.empty')}{' '}
              <Link
                to="/guardrails/create"
                className="underline underline-offset-2"
              >
                {t('guardrails.emptyAction')}
              </Link>
            </span>
          }
          rowSelection={rowSelection}
          onRowSelectionChange={setRowSelection}
          getRowId={(row) => row.id}
          globalFilter={search}
        />
      </div>
    </div>
  );
}

function describeGuardrailConfig(
  guardrail: Guardrail,
  t: ReturnType<typeof useTranslation>['t'],
) {
  switch (guardrail.type) {
    case 'regex':
      return t('guardrails.regexSummary', {
        pattern: guardrail.config.pattern,
      });
    case 'bedrock':
      return t('guardrails.bedrockSummary', {
        identifier: guardrail.config.identifier,
        version: guardrail.config.version,
        region: guardrail.config.region,
      });
  }
}
