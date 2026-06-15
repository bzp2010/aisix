import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { useAdminKey } from '@/hooks/use-admin-key';
import { ApiClientError, catalogApi } from '@/lib/api/client';

export const catalogKeys = {
  all: ['catalog'] as const,
  providers: () => [...catalogKeys.all, 'providers'] as const,
  providerModels: (id: string | undefined) => [...catalogKeys.all, 'models', id] as const,
};

export function useCatalogProviderModels(providerId: string | undefined) {
  const { key, openModal } = useAdminKey();
  return useQuery({
    queryKey: catalogKeys.providerModels(providerId),
    queryFn: () => catalogApi.getProviderModels(key!, providerId!),
    enabled: !!key && !!providerId,
    staleTime: 1000 * 60 * 60,
    retry: (count, err) => {
      if (err instanceof ApiClientError && err.status === 401) {
        openModal();
        return false;
      }
      return count < 2;
    },
  });
}

export function useRefreshCatalog() {
  const qc = useQueryClient();
  const { key } = useAdminKey();
  return useMutation({
    mutationFn: () => catalogApi.refresh(key!),
    onSuccess: () => qc.invalidateQueries({ queryKey: catalogKeys.all }),
  });
}
