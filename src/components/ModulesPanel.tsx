import { useState } from "react";
import { ArrowSquareOut, CaretRight, Code, Folder, Sparkle, TerminalWindow } from "@phosphor-icons/react";
import type { MessageKey } from "../i18n";
import { api } from "../lib/api";
import { formatTime } from "../lib/format";
import type { ExternalTool, ProjectDetail, ProjectModule, ToastTone } from "../types";
import { Button } from "./ui/button";
import { ActionMenu } from "./ui/menu";

export function ModulesPanel({ detail, t, notify, onRefresh, onOpenProject, onRunTask, ides, agents }: {
  detail: ProjectDetail; t: (key: MessageKey) => string;
  notify: (tone: ToastTone, title: string, detail?: string) => void;
  onRefresh: () => void | Promise<void>; onOpenProject?: (id: string) => void;
  onRunTask: (id: string) => void; ides: ExternalTool[]; agents: ExternalTool[];
}) {
  const [busy, setBusy] = useState<string>();
  const modules = detail.modules ?? [];
  if (!modules.length && !detail.project.directoryGroup) return null;
  async function act(key: string, action: () => Promise<unknown>, refresh = false) {
    setBusy(key);
    try { await action(); if (refresh) await onRefresh(); notify("success", t("moduleActionDone")); }
    catch (error) { notify("error", t("moduleActionFailed"), String(error)); }
    finally { setBusy(undefined); }
  }
  async function launch(module: ProjectModule, action: (path: string) => Promise<void>) {
    const path = await api.resolveModulePath(detail.project.id, module.id);
    await action(path);
  }
  return <section className="modules-panel" aria-label={t("modules")}>
    <div className="modules-heading"><div><h2>{detail.project.directoryGroup ? t("directoryGroup") : t("modules")}<span className="modules-count">{modules.length}</span></h2><p>{t("modulesHint")}</p></div>
      <Button loading={busy === "group"} disabled={!!busy} onClick={() => void act("group", () => api.setDirectoryGroup(detail.project.id, !detail.project.directoryGroup), true)}>{t(detail.project.directoryGroup ? "restoreProjectMode" : "useDirectoryGroup")}</Button>
    </div>
    {modules.map(module => <details className="module-row" key={module.id}>
      <summary><CaretRight className="module-chevron" aria-hidden="true" /><Folder aria-hidden="true" /><strong>{module.relativePath}</strong><span className="badge">{t(module.projectId ? "moduleIndependent" : module.evidence === "manifest-candidate" ? "moduleCandidate" : "module")}</span><span className="module-stack">{[...module.languages, ...module.frameworks].join(" · ")}</span>{module.availability !== "ready" && <span>{t("unavailable")}</span>}</summary>
      <div className="module-body">
        <code>{module.canonicalPath}</code>
        <p>{t("moduleEvidence")}: {module.facts.filter(f => f.kind === "manifest").map(f => f.source).join(", ")} · {formatTime(module.observedAt)}</p>
        {module.runtimeRequirements.map((runtime, index) => <p key={index}>{runtime.label}: {runtime.constraint ?? "—"}</p>)}
        <div className="module-actions">
          <Button disabled={!!busy || module.availability !== "ready"} onClick={() => void act(module.id, () => launch(module, path => api.openInTerminal(path)))}><TerminalWindow />{t("openTerminal")}</Button>
          {ides.length > 0 && <ActionMenu trigger={<Button disabled={!!busy || module.availability !== "ready"}><Code />IDE</Button>} items={ides.map(tool => ({ label: tool.name, onClick: () => void act(module.id, () => launch(module, path => api.openInIde(path, tool.id))) }))} />}
          {agents.length > 0 && <ActionMenu trigger={<Button disabled={!!busy || module.availability !== "ready"}><Sparkle />Agent</Button>} items={agents.map(tool => ({ label: tool.name, onClick: () => void act(module.id, () => launch(module, path => api.openInAgent(path, tool.id))) }))} />}
          {module.projectId ? <Button disabled={!onOpenProject} onClick={() => onOpenProject?.(module.projectId!)}><ArrowSquareOut />{t("moduleOpenProject")}</Button> : <Button loading={busy === module.id} disabled={!!busy || module.availability !== "ready"} onClick={() => void act(module.id, () => api.promoteModule(detail.project.id, module.id), true)}>{t("modulePromote")}</Button>}
        </div>
        {!module.projectId && <div className="module-tasks">{module.taskIds.map(id => detail.tasks.find(task => task.id === id)).filter(task => !!task).map(task => <Button disabled={!!busy || module.availability !== "ready"} key={task.id} title={task.cwd ?? module.canonicalPath} onClick={() => onRunTask(task.id)}>{task.name}</Button>)}</div>}
      </div>
    </details>)}
    <p className="modules-note">{t("directoryGroupHint")}</p>
  </section>;
}
