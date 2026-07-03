import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { finboxApi } from "@/lib/api/finbox";

export function useFinboxSkills(query?: string) {
  return useQuery({
    queryKey: ["finbox-skills", query],
    queryFn: () => finboxApi.searchSkills(query),
    staleTime: 30 * 60 * 1000, // 30 minutes
  });
}

export function useFinboxSkillDetail(key: string | null) {
  return useQuery({
    queryKey: ["finbox-skill-detail", key],
    queryFn: () => finboxApi.getSkillDetail(key!),
    enabled: !!key,
  });
}

export function useInstallFromFinbox() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ key, currentApp }: { key: string; currentApp: string }) =>
      finboxApi.installFromFinbox(key, currentApp),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["installed-skills"] });
    },
  });
}

export function useRefreshFinboxCache() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => finboxApi.refreshCache(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["finbox-skills"] });
    },
  });
}
