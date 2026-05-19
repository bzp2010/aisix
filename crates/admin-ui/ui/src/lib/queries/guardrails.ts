import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { useAdminKey } from '@/hooks/use-admin-key';
import { ApiClientError, guardrailsApi } from '@/lib/api/client';
import type { Guardrail } from '@/lib/api/types';

export const guardrailKeys = {
  all: ['guardrails'] as const,
  list: () => [...guardrailKeys.all, 'list'] as const,
  detail: (id: string) => [...guardrailKeys.all, 'detail', id] as const,
};

export function useGuardrails() {
  const { key, openModal } = useAdminKey();
  return useQuery({
    queryKey: guardrailKeys.list(),
    queryFn: () => guardrailsApi.list(key!),
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

export function useGuardrail(id: string) {
  const { key, openModal } = useAdminKey();
  return useQuery({
    queryKey: guardrailKeys.detail(id),
    queryFn: () => guardrailsApi.get(key!, id),
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

export function useCreateGuardrail() {
  const qc = useQueryClient();
  const { key, openModal } = useAdminKey();
  return useMutation({
    mutationFn: (data: Guardrail) => guardrailsApi.create(key!, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: guardrailKeys.list() }),
    onError: (err) => {
      if (err instanceof ApiClientError && err.status === 401) {
        openModal();
      }
    },
  });
}

export function useUpdateGuardrail(id: string) {
  const qc = useQueryClient();
  const { key, openModal } = useAdminKey();
  return useMutation({
    mutationFn: (data: Guardrail) => guardrailsApi.update(key!, id, data),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: guardrailKeys.list() });
      qc.invalidateQueries({ queryKey: guardrailKeys.detail(id) });
    },
    onError: (err) => {
      if (err instanceof ApiClientError && err.status === 401) {
        openModal();
      }
    },
  });
}

export function useDeleteGuardrail() {
  const qc = useQueryClient();
  const { key, openModal } = useAdminKey();
  return useMutation({
    mutationFn: (id: string) => guardrailsApi.delete(key!, id),
    onSuccess: () => qc.invalidateQueries({ queryKey: guardrailKeys.list() }),
    onError: (err) => {
      if (err instanceof ApiClientError && err.status === 401) {
        openModal();
      }
    },
  });
}
