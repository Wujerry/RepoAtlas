import { api } from "./api";
import { ResourceCache } from "./resource-cache";

export const overviewDetails = new ResourceCache((id) => api.getProject(id), 0);
export const overviewGit = new ResourceCache((id) => api.gitStatus(id), 15_000);
export const overviewEnvironment = new ResourceCache((id) => api.inspectProjectEnvironment(id), 60_000);
export const overviewTools = new ResourceCache(() => api.listExternalTools(), 60_000, 1);
// Root documents are capped at 1 MiB by Core; retain at most eight of each.
export const overviewReadme = new ResourceCache((id) => api.readProjectReadme(id), 30_000, 8);
export const overviewAgents = new ResourceCache((id) => api.readProjectDocument(id, "AGENTS.md"), 30_000, 8);

export function invalidateOverviewCache() {
  for (const cache of [overviewDetails, overviewGit, overviewEnvironment, overviewReadme, overviewAgents]) cache.clear();
}
