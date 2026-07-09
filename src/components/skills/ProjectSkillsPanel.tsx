import React, { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  ChevronDown,
  ChevronRight,
  Download,
  FolderOpen,
  Loader2,
  Search,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { TooltipProvider } from "@/components/ui/tooltip";
import { skillsApi } from "@/lib/api";
import type { AppId } from "@/lib/api/types";
import type {
  ImportSkillSelection,
  InstalledSkill,
  SkillUpdateInfo,
  UnmanagedSkill,
} from "@/lib/api/skills";
import { useImportSkillsFromApps } from "@/hooks/useSkills";
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
  managed: InstalledSkill[];
  unmanaged: UnmanagedSkill[];
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
  const importMutation = useImportSkillsFromApps();
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
          managed: prev[projectPath]?.managed ?? [],
          unmanaged: prev[projectPath]?.unmanaged ?? [],
          loading: true,
          loaded: prev[projectPath]?.loaded ?? false,
        },
      }));

      try {
        const [managed, unmanaged] = await Promise.all([
          skillsApi.getInstalled(projectPath),
          skillsApi.scanUnmanaged(projectPath),
        ]);
        setGroups((prev) => ({
          ...prev,
          [projectPath]: { managed, unmanaged, loading: false, loaded: true },
        }));
      } catch (error) {
        setGroups((prev) => ({
          ...prev,
          [projectPath]: {
            managed: prev[projectPath]?.managed ?? [],
            unmanaged: prev[projectPath]?.unmanaged ?? [],
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

  const takeOverSkill = useCallback(
    async (projectPath: string, skill: UnmanagedSkill) => {
      const imports: ImportSkillSelection[] = [
        {
          directory: skill.directory,
          apps: {
            claude: true,
            codex: false,
            gemini: false,
            opencode: false,
            openclaw: false,
            hermes: false,
          },
        },
      ];

      try {
        const imported = await importMutation.mutateAsync({ imports, projectPath });
        setGroups((prev) => {
          const group = prev[projectPath] ?? { managed: [], unmanaged: [], loading: false, loaded: true };
          return {
            ...prev,
            [projectPath]: {
              ...group,
              managed: [...group.managed, ...imported],
              unmanaged: group.unmanaged.filter((item) => item.directory !== skill.directory),
            },
          };
        });
        toast.success(t("skills.importSuccess", { count: imported.length }), { closeButton: true });
      } catch (error) {
        toast.error(t("common.error"), { description: String(error) });
      }
    },
    [importMutation, t],
  );

  const takeOverAll = useCallback(
    async (projectPath: string) => {
      const unmanaged = groups[projectPath]?.unmanaged ?? [];
      if (unmanaged.length === 0) return;
      const imports: ImportSkillSelection[] = unmanaged.map((skill) => ({
        directory: skill.directory,
        apps: {
          claude: true,
          codex: false,
          gemini: false,
          opencode: false,
          openclaw: false,
          hermes: false,
        },
      }));

      try {
        const imported = await importMutation.mutateAsync({ imports, projectPath });
        setGroups((prev) => {
          const group = prev[projectPath] ?? { managed: [], unmanaged: [], loading: false, loaded: true };
          return {
            ...prev,
            [projectPath]: {
              ...group,
              managed: [...group.managed, ...imported],
              unmanaged: [],
            },
          };
        });
        toast.success(t("skills.importSuccess", { count: imported.length }), { closeButton: true });
      } catch (error) {
        toast.error(t("common.error"), { description: String(error) });
      }
    },
    [groups, importMutation, t],
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
            const managedCount = group?.managed.length ?? 0;
            const unmanagedCount = group?.unmanaged.length ?? 0;

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
                    {managedCount}/{unmanagedCount}
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
                    ) : (
                      <>
                        {managedCount > 0 && (
                          <div className="mb-3">
                            <div className="text-xs font-medium text-muted-foreground mb-1">
                              {t("skills.installedProjectSkills", { count: managedCount })}
                            </div>
                            <TooltipProvider delayDuration={300}>
                              <div className="rounded-lg border border-border-default overflow-hidden">
                                {group?.managed.map((skill, index) => (
                                  <InstalledSkillListItem
                                    key={skill.id}
                                    skill={skill}
                                    hasUpdate={!!updatesMap[skill.id]}
                                    isUpdating={isUpdatingSkillId === skill.id}
                                    onToggleApp={onToggleApp}
                                    onUninstall={() => onUninstall(skill)}
                                    onUpdate={() => onUpdate(skill)}
                                    isLast={index === managedCount - 1}
                                  />
                                ))}
                              </div>
                            </TooltipProvider>
                          </div>
                        )}

                        <div className="flex items-center justify-between mb-2">
                          <span className="text-xs text-muted-foreground">
                            {t("skills.projectSkillsCount", { count: unmanagedCount })}
                          </span>
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            className="h-7 text-xs gap-1"
                            onClick={() => takeOverAll(projectPath)}
                            disabled={unmanagedCount === 0 || importMutation.isPending}
                          >
                            <Download className="h-3 w-3" />
                            {t("skills.importToCcSwitch")}
                          </Button>
                        </div>

                        {unmanagedCount === 0 ? (
                          <div className="text-xs text-muted-foreground/60 py-2">
                            {managedCount === 0 ? t("skills.noSkillsInProject") : t("skills.noUnmanagedFound")}
                          </div>
                        ) : (
                          <div className="space-y-1">
                            {group?.unmanaged.map((skill) => (
                              <div
                                key={`${projectPath}:${skill.directory}:${skill.path}`}
                                className="flex items-center justify-between rounded py-1 px-1 hover:bg-muted/30 text-sm"
                              >
                                <div className="flex items-center gap-1.5 min-w-0 flex-1">
                                  <span className="font-medium truncate text-sm">{skill.name}</span>
                                  {skill.description && (
                                    <span className="text-xs text-muted-foreground/60 truncate">
                                      {skill.description}
                                    </span>
                                  )}
                                </div>
                                <Button
                                  type="button"
                                  variant="ghost"
                                  size="sm"
                                  className="h-7 text-xs"
                                  onClick={() => takeOverSkill(projectPath, skill)}
                                  disabled={importMutation.isPending}
                                >
                                  {t("skills.importToCcSwitch")}
                                </Button>
                              </div>
                            ))}
                          </div>
                        )}
                      </>
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
