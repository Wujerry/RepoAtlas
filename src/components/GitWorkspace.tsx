import { GitBranch, GitCommit } from "@phosphor-icons/react";
import type { MessageKey } from "../i18n";
import { formatTime } from "../lib/format";
import type { GitOp, GitStatus } from "../types";
import { Button } from "./ui/button";
import { EmptyState, Skeleton } from "./ui/feedback";
import { InlineLoadError } from "./DashboardShared";

export function GitWorkspace({ git, loading, error, busy, commitMessage, setCommitMessage, stagedCount, selectedDiff, diff, diffLoading, t, onRetry, onGit, onDiff }: { git: GitStatus | null; loading: boolean; error?: string; busy: boolean; commitMessage: string; setCommitMessage: (value: string) => void; stagedCount: number; selectedDiff?: { path: string; staged: boolean }; diff: string; diffLoading: boolean; t: (key: MessageKey) => string; onRetry: () => void; onGit: (op: GitOp) => void; onDiff: (path: string, staged: boolean) => void }) {
  if (loading) return <div className="git-layout"><Skeleton className="skeleton-card" /><Skeleton className="skeleton-card" /></div>;
  if (error) return <InlineLoadError title={t("gitLoadFailed")} detail={error} retryLabel={t("retry")} onRetry={onRetry} />;
  if (!git) return <EmptyState title={t("gitUnavailable")} body={t("gitUnavailableHint")} />;
  return <div className="git-layout">
    <section className="content-card git-summary"><div className="section-heading"><div><p className="eyebrow">{t("currentBranch")}</p><h2>{git.snapshot.branch ?? t("detachedHead")}</h2></div><span className={git.snapshot.dirty ? "status-warning" : "status-success"}>{git.snapshot.dirty ? t("dirty") : t("clean")}</span></div>
      <div className="git-stats"><span>{git.files.length} {t("changedFiles")}</span><span>{stagedCount} {t("staged")}</span><span>↑ {git.snapshot.ahead ?? 0}</span><span>↓ {git.snapshot.behind ?? 0}</span></div>
      <div className="action-cluster"><Button onClick={() => onGit({ type: "pull" })} disabled={busy}>{t("pull")}</Button><Button onClick={() => onGit({ type: "push" })} disabled={busy}>{t("push")}</Button><Button onClick={() => onGit({ type: "stashPush" })} disabled={busy}>{t("stash")}</Button><Button onClick={() => onGit({ type: "stashPop" })} disabled={busy}>{t("stashPop")}</Button></div>
      <div className="field-group commit-field"><label htmlFor="git-commit-message">{t("commitMessage")}</label><div className="commit-controls"><input id="git-commit-message" value={commitMessage} onChange={(event) => setCommitMessage(event.target.value)} placeholder={t("commitMessageHint")} /><Button variant="primary" loading={busy} disabled={!commitMessage.trim() || stagedCount === 0 || busy} onClick={() => onGit({ type: "commit", message: commitMessage.trim() })}><GitCommit />{t("commit")}</Button></div>{stagedCount === 0 && <span className="field-hint">{t("stageBeforeCommit")}</span>}</div>
    </section>
    <section className="content-card git-files"><div className="section-heading"><div><p className="eyebrow">{t("workingTree")}</p><h2>{t("changedFiles")}</h2></div></div>
      {git.files.length === 0 ? <EmptyState compact title={t("noChanges")} body={t("cleanWorkspaceHint")} /> : <div className="file-list">{git.files.map((file) => <div className={`file-row ${selectedDiff?.path === file.path && selectedDiff.staged === file.staged ? "active" : ""}`} key={`${file.path}-${file.staged}`}><button onClick={() => onDiff(file.path, file.staged)}><span className="file-status">{file.status}</span><code title={file.path}>{file.path}</code><span>{file.staged ? t("staged") : t("unstaged")}</span></button><Button variant="quiet" onClick={() => onGit({ type: file.staged ? "unstage" : "stage", paths: [file.path] })}>{file.staged ? t("unstage") : t("stage")}</Button></div>)}</div>}
    </section>
    <section className="content-card diff-card"><div className="section-heading"><div><p className="eyebrow">{selectedDiff?.staged ? t("stagedDiff") : t("workingDiff")}</p><h2>{selectedDiff?.path ?? t("selectFile")}</h2></div></div>{diffLoading ? <Skeleton className="skeleton-code" /> : <pre className="diff-pane">{diff || t("selectFileHint")}</pre>}</section>
    <aside className="git-sidebar"><section className="content-card"><div className="section-heading"><div><p className="eyebrow">{t("repository")}</p><h2>{t("branches")}</h2></div></div><div className="compact-list">{git.branches.slice(0, 12).map((branch) => <div key={branch}><GitBranch /><span>{branch}</span></div>)}</div></section>
      <section className="content-card"><div className="section-heading"><div><p className="eyebrow">{t("recentActivity")}</p><h2>{t("history")}</h2></div></div><div className="commit-list">{git.log.slice(0, 10).map((entry) => <div key={entry.sha}><code>{entry.sha.slice(0, 7)}</code><strong>{entry.subject}</strong><span>{entry.author} · {formatTime(entry.committedAt)}</span></div>)}</div></section></aside>
  </div>;
}
