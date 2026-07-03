import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Search, RefreshCw, Download, ExternalLink } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { toast } from "sonner";
import {
  useFinboxSkills,
  useInstallFromFinbox,
  useRefreshFinboxCache,
} from "@/hooks/useFinbox";
import type { AppId } from "@/lib/api/types";

interface FinboxMarketplacePanelProps {
  currentApp: AppId;
}

export function FinboxMarketplacePanel({
  currentApp,
}: FinboxMarketplacePanelProps) {
  const { t } = useTranslation();
  const [searchQuery, setSearchQuery] = useState("");
  const {
    data: skills,
    isLoading,
    error,
  } = useFinboxSkills(searchQuery || undefined);
  const installMutation = useInstallFromFinbox();
  const refreshMutation = useRefreshFinboxCache();

  const handleInstall = (key: string) => {
    installMutation.mutate(
      { key, currentApp },
      {
        onSuccess: () => toast.success(t("skills.installSuccess")),
        onError: (err) =>
          toast.error(`${t("skills.installFailed")}: ${err.message}`),
      },
    );
  };

  const handleRefresh = () => {
    refreshMutation.mutate(undefined, {
      onSuccess: () => toast.success(t("skills.cacheRefreshed")),
      onError: (err) =>
        toast.error(`${t("skills.cacheRefreshFailed")}: ${err.message}`),
    });
  };

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
        <p>{t("skills.finboxLoadError")}</p>
        <Button variant="outline" onClick={handleRefresh} className="mt-4">
          <RefreshCw className="mr-2 h-4 w-4" />
          {t("skills.retry")}
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder={t("skills.searchFinbox")}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-9"
          />
        </div>
        <Button
          variant="outline"
          size="icon"
          onClick={handleRefresh}
          disabled={refreshMutation.isPending}
        >
          <RefreshCw
            className={`h-4 w-4 ${refreshMutation.isPending ? "animate-spin" : ""}`}
          />
        </Button>
      </div>

      {isLoading ? (
        <div className="flex items-center justify-center py-12 text-muted-foreground">
          <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
          {t("skills.loading")}
        </div>
      ) : !skills?.length ? (
        <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
          <p>{t("skills.noFinboxSkills")}</p>
        </div>
      ) : (
        <div className="space-y-2">
          {skills.map((skill) => (
            <div
              key={skill.key}
              className="flex items-center justify-between rounded-lg border p-3"
            >
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="truncate font-medium">{skill.name}</span>
                  {skill.category && (
                    <Badge variant="secondary" className="text-xs">
                      {skill.category}
                    </Badge>
                  )}
                </div>
                {skill.description && (
                  <p className="mt-1 line-clamp-2 text-sm text-muted-foreground">
                    {skill.description}
                  </p>
                )}
              </div>
              <div className="ml-4 flex items-center gap-2">
                {skill.downloadUrl && (
                  <a
                    href={skill.downloadUrl}
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <Button variant="ghost" size="icon">
                      <ExternalLink className="h-4 w-4" />
                    </Button>
                  </a>
                )}
                <Button
                  size="sm"
                  onClick={() => handleInstall(skill.key)}
                  disabled={installMutation.isPending}
                >
                  <Download className="mr-1 h-3 w-3" />
                  {t("skills.install")}
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
