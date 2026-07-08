import { invoke } from "@tauri-apps/api/core";

export interface FinboxSkill {
  key: string;
  name: string;
  description?: string;
  downloadUrl?: string;
  category?: string;
}

export interface FinboxSkillDetail {
  key: string;
  name: string;
  description?: string;
  downloadUrl?: string;
  category?: string;
  readme?: string;
}

export const finboxApi = {
  async searchSkills(query?: string): Promise<FinboxSkill[]> {
    return await invoke("search_finbox_skills", { query: query ?? null });
  },

  async getSkillDetail(key: string): Promise<FinboxSkillDetail> {
    return await invoke("get_finbox_skill_detail", { key });
  },

  async installFromFinbox(
    key: string,
    currentApp: string,
  ): Promise<import("./skills").InstalledSkill> {
    return await invoke("install_from_finbox", { key, currentApp });
  },

  async refreshCache(): Promise<boolean> {
    return await invoke("refresh_finbox_cache");
  },

  async setSsoCookie(cookie: string): Promise<boolean> {
    return await invoke("set_finbox_sso_cookie", { cookie });
  },

  async getSsoCookie(): Promise<string | null> {
    return await invoke("get_finbox_sso_cookie");
  },

  async hasSsoCookie(): Promise<boolean> {
    return await invoke("has_finbox_sso_cookie");
  },
};
