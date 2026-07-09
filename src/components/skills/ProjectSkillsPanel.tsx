import React, { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, ChevronRight, FolderOpen, Loader2, Search, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { TooltipProvider } from "@/components/ui/tooltip";
import { skillsApi } from "@/lib/api";
import type { AppId } from "@/lib/api/types";
import type { InstalledSkill, ProjectSkillEntry, SkillUpdateInfo } from "@/lib/api/skills";
import { toast } from "sonner";
import { InstalledSkillListItem } from "./UnifiedSkillsPanel";

interface ProjectSkillsPanelProps {
  currentApp: AppId;
  updatesMap: Record<string, SkillUpdateInfo>;
  isUpdatingSkillId?: string;
  onToggleApp: (id: string, app: AppId, enabled: boolean) => void;
  onUninstall: (skill: InstalledSkill) => void;
  onUpdate: (skill: InstalledSkill) => void;
}

interface ProjectGroupState {
  entries: ProjectSkillEntry[];
  loading: boolean;
  loaded: boolean;
}

export function ProjectSkillsPanel({
  updatesMap,
  isUpdatingSkillId,
  onToggleApp,
  onUninstall,
  onUpdate,
}: ProjectSkillsPanelProps) {
  const { t } = useTranslation();
  const [projectPaths, setProjectPaths] = useState<string[] | null>(null);
  const [loadingProjects, setLoadingProjects] = useState(false);
  const [expandedProjects, setExpandedProjects] = useState<Set<string>>(new Set());
  const [groups, setGroups] = useState<Record<string, ProjectGroupState>>({});

  const loadProject = useCallback(
    async (projectPath: string, force = false) => {
      const existing = groups[projectPath];
      if (!force && existing?.loaded) return;
      setGroups((prev) => ({
        ...prev,
        [projectPath]: {
          entries: prev[projectPath]?.entries ?? [],
          loading: true,
          loaded: prev[projectPath]?.loaded ?? false,
        },
      }));

      try {
        const entries = await skillsApi.listProjectSkillEntries(projectPath);
        setGroups((prev) => ({
          ...prev,
          [projectPath]: { entries, loading: false, loaded: true },
        }));
      } catch (error) {
        setGroups((prev) => ({
          ...prev,
          [projectPath]: {
            entries: prev[projectPath]?.entries ?? [],
            loading: false,
            loaded: true,
          },
        }));
        toast.error(t("common.error"), { description: String(error) });
      }
    },
    [groups, t],
  );

  const scanProjects = useCallback(async () => {
    setLoadingProjects(true);
    try {
      const projects = await skillsApi.listSkillProjects();
      setProjectPaths(projects);
      setExpandedProjects(new Set(projects));
      await Promise.all(projects.map((projectPath) => loadProject(projectPath, true)));
      if (projects.length === 0) {
        toast.info(t("skills.noProjectSkillsFound"));
      }
    } catch (error) {
      toast.error(t("common.error"), { description: String(error) });
    } finally {
      setLoadingProjects(false);
    }
  }, [loadProject, t]);

  const toggleProject = useCallback(
    async (projectPath: string) => {
      if (expandedProjects.has(projectPath)) {
        setExpandedProjects((prev) => {
          const next = new Set(prev);
          next.delete(projectPath);
          return next;
        });
        return;
      }
      setExpandedProjects((prev) => new Set(prev).add(projectPath));
      await loadProject(projectPath);
    },
    [expandedProjects, loadProject],
  );

  return (
    <div className="mb-4">
      <div className="flex items-center gap-2 mb-3">
        <Button
          type="button"
          variant="default"
          size="sm"
          className="h-8 text-xs gap-1"
          onClick={scanProjects}
          disabled={loadingProjects}
        >
          {loadingProjects ? <Loader2 className="h-3 w-3 animate-spin" /> : <Search className="h-3 w-3" />}
          {t("skills.scanForProjects")}
        </Button>
        {loadingProjects && <span className="text-xs text-muted-foreground">{t("skills.loading")}</span>}
      </div>

      {projectPaths && projectPaths.length === 0 && (
        <div className="text-center py-8 text-sm text-muted-foreground">
          {t("skills.noProjectSkillsFound")}
        </div>
      )}

      {projectPaths && projectPaths.length > 0 && (
        <div className="rounded-lg border bg-muted/30 p-3 space-y-2">
          {projectPaths.map((projectPath) => {
            const group = groups[projectPath];
            const expanded = expandedProjects.has(projectPath);
            const projectName = projectPath.split("/").pop() || projectPath;
            const entries = group?.entries ?? [];

            return (
              <div key={projectPath} className="border rounded-md bg-background">
                <button
                  type="button"
                  className="flex items-center gap-2 w-full text-left text-sm font-medium hover:bg-muted/50 rounded px-2 py-1"
                  onClick={() => toggleProject(projectPath)}
                >
                  <FolderOpen className="h-4 w-4 shrink-0 text-muted-foreground" />
                  <span className="flex-1 truncate">{projectName}</span>
                  <span className="text-xs text-muted-foreground/60 shrink-0 truncate max-w-[240px]">
                    {projectPath}
                  </span>
                  <Badge variant="outline" className="text-[10px] px-1 h-4">
                    {entries.length}
                  </Badge>
                  {expanded ? <ChevronDown className="h-3 w-3 shrink-0" /> : <ChevronRight className="h-3 w-3 shrink-0" />}
                </button>

                {expanded && (
                  <div className="mt-1 pl-4 pr-2 pb-2">
                    {group?.loading ? (
                      <div className="flex items-center gap-1 text-xs text-muted-foreground py-2">
                        <Loader2 className="h-3 w-3 animate-spin" />
                        {t("skills.loading")}
                      </div>
                    ) : entries.length === 0 ? (
                      <div className="text-xs text-muted-foreground/60 py-2">
                        {t("skills.noSkillsInProject")}
                      </div>
                    ) : (
                      <TooltipProvider delayDuration={300}>
                        <div className="rounded-lg border border-border-default overflow-hidden">
                          {entries.map((entry, index) => {
                            if (entry.managed) {
                              return (
                                <InstalledSkillListItem
                                  key={entry.managed.id}
                                  skill={entry.managed}
                                  hasUpdate={!!updatesMap[entry.managed.id]}
                                  isUpdating={isUpdatingSkillId === entry.managed.id}
                                  onToggleApp={onToggleApp}
                                  onUninstall={() => onUninstall(entry.managed!)}
                                  onUpdate={() => onUpdate(entry.managed!)}
                                  isLast={index === entries.length - 1}
                                />
                              );
                            }

                            return (
                              <div
                                key={`${projectPath}:${entry.directory}:${entry.path}`}
                                className="flex items-center justify-between px-3 py-2 border-b border-border-default last:border-b-0"
                              >
                                <div className="min-w-0 flex-1">
                                  <div className="flex items-center gap-2">
                                    <span className="font-medium text-sm truncate">{entry.name}</span>
                                    <Badge variant="secondary" className="text-[10px] px-1 h-4">
                                      {t("skills.projectScope")}
                                    </Badge>
                                  </div>
                                  {entry.description && (
                                    <div className="text-xs text-muted-foreground truncate" title={entry.description}>
                                      {entry.description}
                                    </div>
                                  )}
                                </div>
                                <Button type="button" variant="ghost" size="icon" disabled>
                                  <Trash2 className="h-4 w-4 opacity-40" />
                                </Button>
                              </div>
                            );
                          })}
                        </div>
                      </TooltipProvider>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
