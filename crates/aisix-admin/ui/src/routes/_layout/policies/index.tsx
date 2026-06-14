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
import type { ItemResponse, Policy } from '@/lib/api/types';
import { useDeletePolicy, usePolicies } from '@/lib/queries/policies';

export const Route = createFileRoute('/_layout/policies/')({
  component: PoliciesPage,
});

type PolicyRow = ItemResponse<Policy> & {
  id: string;
  actionSummary: string;
};

const col = createColumnHelper<PolicyRow>();

function PoliciesPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data, isLoading, isError } = usePolicies();
  const deletePolicy = useDeletePolicy();

  const [search, setSearch] = useState('');
  const [rowSelection, setRowSelection] = useState<RowSelectionState>({});
  const [actionError, setActionError] = useState<string | null>(null);

  const items = (data?.list ?? []).map(
    ({ key, value, ...rest }) =>
      ({
        id: key.replace('/policies/', ''),
        key,
        value,
        actionSummary: describePolicyActions(value, t),
        ...rest,
      }) as PolicyRow,
  );
  const selectedKeys = Object.keys(rowSelection);

  function formatDeleteError(error: unknown) {
    return error instanceof Error ? error.message : t('policies.deleteFailed');
  }

  async function handleDeleteOne(id: string) {
    setActionError(null);
    try {
      await deletePolicy.mutateAsync(id);
    } catch (error) {
      setActionError(formatDeleteError(error));
    }
  }

  async function handleDeleteSelected() {
    setActionError(null);

    const results = await Promise.allSettled(
      selectedKeys.map(async (id) => {
        await deletePolicy.mutateAsync(id);
        return id;
      }),
    );

    const failed = results.flatMap((result, index) =>
      result.status === 'rejected'
        ? [
            {
              id: selectedKeys[index],
              message: formatDeleteError(result.reason),
            },
          ]
        : [],
    );

    if (failed.length > 0) {
      const successCount = selectedKeys.length - failed.length;
      setActionError(
        t('policies.bulkDeleteFailed', {
          successCount,
          failureCount: failed.length,
          details: failed
            .map(({ id, message }) => `${id}: ${message}`)
            .join('; '),
        }),
      );
      setRowSelection(
        Object.fromEntries(
          failed.map(({ id }) => [id, true]),
        ) satisfies RowSelectionState,
      );
      return;
    }

    setRowSelection({});
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
      header: () => t('policies.columns.id'),
      size: 260,
      cell: (info) => (
        <span className="font-mono text-xs text-muted-foreground">
          {info.getValue()}
        </span>
      ),
    }),
    col.accessor('value.name', {
      header: () => t('policies.columns.name'),
      size: 220,
      cell: (info) => (
        <span className="text-sm font-medium">{info.getValue()}</span>
      ),
    }),
    col.accessor('value.enabled', {
      header: () => t('policies.columns.enabled'),
      size: 120,
      cell: (info) => (
        <Badge variant={info.getValue() ? 'default' : 'outline'}>
          {info.getValue() ? t('policies.enabled') : t('policies.disabled')}
        </Badge>
      ),
    }),
    col.accessor('value.priority', {
      header: () => t('policies.columns.priority'),
      size: 100,
      cell: (info) => (
        <Badge variant="secondary" className="font-mono text-xs">
          {info.getValue()}
        </Badge>
      ),
    }),
    col.accessor('value.when', {
      header: () => t('policies.columns.when'),
      size: 320,
      cell: (info) => (
        <span
          className="block max-w-md truncate font-mono text-xs text-muted-foreground"
          title={info.getValue()}
        >
          {info.getValue()}
        </span>
      ),
    }),
    col.accessor('actionSummary', {
      header: () => t('policies.columns.actions'),
      cell: (info) => (
        <span
          className="block max-w-lg truncate text-sm text-muted-foreground"
          title={info.getValue()}
        >
          {info.getValue()}
        </span>
      ),
    }),
    col.display({
      id: 'actions',
      size: 96,
      header: () => (
        <div className="text-right">{t('policies.columns.action')}</div>
      ),
      cell: ({ row }) => (
        <div className="flex justify-end">
          <Button
            variant="ghost"
            size="icon-lg"
            onClick={() =>
              navigate({
                to: '/policies/$id',
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

  return (
    <div className="flex h-full flex-col">
      <PageHeader>
        <h1 className="flex-1 text-xl font-semibold">{t('policies.title')}</h1>
        {selectedKeys.length > 0 && (
          <Button
            variant="outline"
            size="lg"
            onClick={handleDeleteSelected}
            disabled={deletePolicy.isPending}
          >
            <Trash2 className="mr-1.5 h-4 w-4 text-destructive" />
            {t('common.delete')} ({selectedKeys.length})
          </Button>
        )}
        <Button asChild size="lg">
          <Link to="/policies/create">
            <Plus className="mr-1.5 h-4 w-4" />
            {t('policies.addPolicy')}
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
              : t('policies.count', { count: items.length })}
          </p>
          <Input
            className="w-80"
            placeholder={t('policies.search')}
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
        </div>

        <DataTable
          columns={columns as ColumnDef<PolicyRow>[]}
          data={items}
          isLoading={isLoading}
          isError={isError}
          errorMessage={t('policies.errorLoad')}
          emptyMessage={
            <span>
              {t('policies.empty')}{' '}
              <Link
                to="/policies/create"
                className="underline underline-offset-2"
              >
                {t('policies.emptyAction')}
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

function describePolicyActions(
  policy: Policy,
  t: ReturnType<typeof useTranslation>['t'],
) {
  if (policy.actions.length === 0) {
    return '—';
  }

  return policy.actions
    .map((action) => {
      const stages = action.config.stages
        .map((stage) => t(`policyStages.${stage}`))
        .join('/');
      return `${stages}: ${action.config.guardrail_ids.join(', ')}`;
    })
    .join(' | ');
}
