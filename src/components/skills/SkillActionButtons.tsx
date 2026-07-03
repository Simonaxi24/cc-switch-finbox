import { useTranslation } from "react-i18next";
import { RefreshCw, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";

interface SkillActionButtonsProps {
  scope: "global" | "project";
  skillId: string;
  skillName: string;
  onUpdate?: () => void;
  onUninstall?: () => void;
  isUpdating?: boolean;
  isUninstalling?: boolean;
}

export function SkillActionButtons({
  scope,
  skillId,
  skillName,
  onUpdate,
  onUninstall,
  isUpdating,
  isUninstalling,
}: SkillActionButtonsProps) {
  const { t } = useTranslation();

  return (
    <div
      className="flex items-center gap-1"
      data-scope={scope}
      data-skill-id={skillId}
    >
      {onUpdate && (
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="h-7 w-7 hover:text-blue-500 hover:bg-blue-100 dark:hover:text-blue-400 dark:hover:bg-blue-500/10"
          onClick={onUpdate}
          disabled={isUpdating}
          title={t("skills.update")}
          aria-label={`${t("skills.update")} ${skillName}`}
        >
          <RefreshCw
            className={`h-4 w-4 ${isUpdating ? "animate-spin" : ""}`}
          />
        </Button>
      )}
      {onUninstall && (
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="h-7 w-7 hover:text-red-500 hover:bg-red-100 dark:hover:text-red-400 dark:hover:bg-red-500/10"
          onClick={onUninstall}
          disabled={isUninstalling}
          title={t("skills.uninstall")}
          aria-label={`${t("skills.uninstall")} ${skillName}`}
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      )}
    </div>
  );
}
