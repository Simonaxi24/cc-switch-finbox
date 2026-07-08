import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Search, RefreshCw, Download, ExternalLink, Key, X } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { toast } from "sonner";
import {
  useFinboxSkills,
  useInstallFromFinbox,
  useRefreshFinboxCache,
} from "@/hooks/useFinbox";
import { finboxApi } from "@/lib/api";
import type { AppId } from "@/lib/api/types";

interface FinboxMarketplacePanelProps {
  currentApp: AppId;
}

export function FinboxMarketplacePanel({
  currentApp,
}: FinboxMarketplacePanelProps) {
  const { t } = useTranslation();
  const [searchQuery, setSearchQuery] = useState("");
  const [cookieInput, setCookieInput] = useState("");
  const [showCookieInput, setShowCookieInput] = useState(false);
  const [hasCookie, setHasCookie] = useState(false);
  const {
    data: skills,
    isLoading,
    error,
  } = useFinboxSkills(searchQuery || undefined);
  const installMutation = useInstallFromFinbox();
  const refreshMutation = useRefreshFinboxCache();

  // 检查 cookie 状态（首次加载时）
  useState(() => {
    finboxApi.hasSsoCookie().then(setHasCookie);
  });

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

  const handleSetCookie = async () => {
    if (!cookieInput.trim()) return;
    try {
      await finboxApi.setSsoCookie(cookieInput.trim());
      setHasCookie(true);
      setShowCookieInput(false);
      setCookieInput("");
      toast.success(t("skills.finboxCookieSet"));
    } catch (err) {
      toast.error(`${t("skills.finboxCookieFailed")}: ${err}`);
    }
  };

  const handleClearCookie = async () => {
    try {
      await finboxApi.setSsoCookie("");
      setHasCookie(false);
      toast.info(t("skills.finboxCookieCleared"));
    } catch (err) {
      toast.error(`${err}`);
    }
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
      {/* SSO Cookie 配置栏 */}
      <div className="flex items-center gap-2 rounded-lg border border-dashed border-amber-500/50 bg-amber-50/30 px-3 py-2 dark:bg-amber-950/20">
        <Key className="h-4 w-4 shrink-0 text-amber-600" />
        <span className="flex-1 text-xs text-muted-foreground">
          {hasCookie
            ? t("skills.finboxCookieConfigured")
            : t("skills.finboxCookieNeeded")}
        </span>
        {showCookieInput ? (
          <div className="flex items-center gap-1">
            <Input
              value={cookieInput}
              onChange={(e) => setCookieInput(e.target.value)}
              placeholder="sso_session_ticket=xxx; ..."
              className="h-7 w-48 text-xs"
            />
            <Button size="sm" variant="default" className="h-7 text-xs" onClick={handleSetCookie}>
              {t("skills.save")}
            </Button>
            <Button size="sm" variant="ghost" className="h-7 w-7 px-0" onClick={() => setShowCookieInput(false)}>
              <X className="h-3 w-3" />
            </Button>
          </div>
        ) : (
          <Button
            size="sm"
            variant={hasCookie ? "outline" : "default"}
            className="h-7 text-xs"
            onClick={() => hasCookie ? handleClearCookie() : setShowCookieInput(true)}
          >
            {hasCookie ? t("skills.finboxClearCookie") : t("skills.finboxSetCookie")}
          </Button>
        )}
      </div>

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