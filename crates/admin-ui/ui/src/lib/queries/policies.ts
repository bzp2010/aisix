import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { useAdminKey } from '@/hooks/use-admin-key';
import { ApiClientError, policiesApi } from '@/lib/api/client';
import type { Policy } from '@/lib/api/types';

export const policyKeys = {
  all: ['policies'] as const,
  list: () => [...policyKeys.all, 'list'] as const,
  detail: (id: string) => [...policyKeys.all, 'detail', id] as const,
};

function requireAdminKey(key: string | null, openModal: () => void) {
  if (!key) {
    openModal();
    throw new Error('Admin API key is required');
  }

  return key;
}

export function usePolicies() {
  const { key, openModal } = useAdminKey();
  return useQuery({
    queryKey: policyKeys.list(),
    queryFn: () => policiesApi.list(key!),
    enabled: !!key,
    retry: (count, err) => {
      if (err instanceof ApiClientError && err.status === 401) {
        openModal();
        return false;
      }
      return count < 2;
    },
  });
}

export function usePolicy(id: string) {
  const { key, openModal } = useAdminKey();
  return useQuery({
    queryKey: policyKeys.detail(id),
    queryFn: () => policiesApi.get(key!, id),
    enabled: !!key && !!id,
    retry: (count, err) => {
      if (err instanceof ApiClientError && err.status === 401) {
        openModal();
        return false;
      }
      return count < 2;
    },
  });
}

export function useCreatePolicy() {
  const qc = useQueryClient();
  const { key, openModal } = useAdminKey();
  return useMutation({
    mutationFn: (data: Policy) =>
      policiesApi.create(requireAdminKey(key, openModal), data),
    onSuccess: () => qc.invalidateQueries({ queryKey: policyKeys.list() }),
    onError: (err) => {
      if (err instanceof ApiClientError && err.status === 401) {
        openModal();
      }
    },
  });
}

export function useUpdatePolicy(id: string) {
  const qc = useQueryClient();
  const { key, openModal } = useAdminKey();
  return useMutation({
    mutationFn: (data: Policy) =>
      policiesApi.update(requireAdminKey(key, openModal), id, data),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: policyKeys.list() });
      qc.invalidateQueries({ queryKey: policyKeys.detail(id) });
    },
    onError: (err) => {
      if (err instanceof ApiClientError && err.status === 401) {
        openModal();
      }
    },
  });
}

export function useDeletePolicy() {
  const qc = useQueryClient();
  const { key, openModal } = useAdminKey();
  return useMutation({
    mutationFn: (id: string) =>
      policiesApi.delete(requireAdminKey(key, openModal), id),
    onSuccess: () => qc.invalidateQueries({ queryKey: policyKeys.list() }),
    onError: (err) => {
      if (err instanceof ApiClientError && err.status === 401) {
        openModal();
      }
    },
  });
}
