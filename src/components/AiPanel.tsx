import { Brain, ChatCircleDots, FloppyDisk, Key, Sparkle, Trash, WarningCircle } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import type { MessageKey } from "../i18n";
import { api } from "../lib/api";
import type { AiMemoryItem, AiSummary, AnalysisPlan, ProviderProfile, ToastTone } from "../types";
import { Button } from "./ui/button";
import { ConfirmDialog } from "./ui/confirm";
import { EmptyState, Skeleton } from "./ui/feedback";

type AiPanelProps = {
  projectId: string;
  t: (key: MessageKey) => string;
  notify: (tone: ToastTone, title: string, detail?: string) => void;
  onOpenSettings?: () => void;
};

type LoadState = "loading" | "error" | "ready";
type PendingAiAction = { kind: "summary" } | { kind: "ask"; question: string };

export function AiPanel({ projectId, t, notify, onOpenSettings }: AiPanelProps) {
  const [providers, setProviders] = useState<ProviderProfile[]>([]);
  const [providerId, setProviderId] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [summary, setSummary] = useState<AiSummary | null>(null);
  const [evidenceSummary, setEvidenceSummary] = useState<AiSummary | null>(null);
  const [summaries, setSummaries] = useState<AiSummary[]>([]);
  const [memory, setMemory] = useState<AiMemoryItem[]>([]);
  const [memoryDraft, setMemoryDraft] = useState("");
  const [question, setQuestion] = useState("");
  const [answer, setAnswer] = useState("");
  const [busy, setBusy] = useState<"summary" | "ask" | "memory" | null>(null);
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [loadError, setLoadError] = useState<string | null>(null);
  const [memoryToDelete, setMemoryToDelete] = useState<AiMemoryItem | null>(null);
  const [deletingMemoryId, setDeletingMemoryId] = useState<string | null>(null);
  const [analysisPlan, setAnalysisPlan] = useState<AnalysisPlan | null>(null);
  const [pendingAiAction, setPendingAiAction] = useState<PendingAiAction | null>(null);
  const deletingMemoryRef = useRef<string | null>(null);

  async function load(): Promise<boolean> {
    setLoadState("loading");
    setLoadError(null);
    try {
      const [nextProviders, nextSummary, nextMemory, nextSummaries] = await Promise.all([api.listProviderProfiles(), api.latestSummary(projectId), api.listMemory(projectId), api.listSummaries(projectId)]);
      setProviders(nextProviders); setSummary(nextSummary); setMemory(nextMemory); setSummaries(nextSummaries);
      setProviderId((current) => current && nextProviders.some((provider) => provider.id === current) ? current : nextProviders[0]?.id ?? "");
      setLoadState("ready");
      return true;
    } catch (error) {
      const detail = errorDetail(error, t("unknownAiError"));
      setLoadError(detail);
      setLoadState("error");
      notify("error", t("knowledgeLoadFailed"), detail);
      return false;
    }
  }

  useEffect(() => { setAnswer(""); setQuestion(""); void load(); }, [projectId]);

  async function prepareAnalysis(action: PendingAiAction) {
    if (!providerId) return;
    setBusy(action.kind);
    try {
      setAnalysisPlan(await api.analysisPlan(projectId, providerId));
      setPendingAiAction(action);
    } catch (error) {
      notify("error", t("analysisPlanFailed"), errorDetail(error, t("unknownAiError")));
    } finally {
      setBusy(null);
    }
  }

  async function generateSummary() {
    if (!providerId) return; setBusy("summary");
    try {
      const next = await api.summarizeProject(projectId, providerId, apiKey);
      setSummary(next);
      setSummaries(await api.listSummaries(projectId));
      notify("success", t("summaryReady"));
      return true;
    }
    catch (error) { notify("error", t("summaryFailed"), errorDetail(error, t("unknownAiError"))); return false; }
    finally { setBusy(null); }
  }

  async function ask(preparedQuestion: string) {
    if (!providerId || !preparedQuestion.trim()) return; setBusy("ask");
    try { setAnswer(await api.askProject(projectId, providerId, apiKey, preparedQuestion.trim())); setQuestion(""); return true; }
    catch (error) { notify("error", t("questionFailed"), errorDetail(error, t("unknownAiError"))); return false; }
    finally { setBusy(null); }
  }

  async function saveMemory() {
    const value = memoryDraft.trim(); if (!value) return; setBusy("memory");
    try {
      await api.addMemory(projectId, value);
      setMemoryDraft("");
      if (await load()) notify("success", t("memorySaved"));
    } catch (error) { notify("error", t("memorySaveFailed"), errorDetail(error, t("unknownAiError"))); }
    finally { setBusy(null); }
  }

  async function confirmAnalysis() {
    const action = pendingAiAction;
    if (!action) return;
    const succeeded = action.kind === "summary" ? await generateSummary() : await ask(action.question);
    if (succeeded) {
      setPendingAiAction(null);
      setAnalysisPlan(null);
    }
  }

  async function confirmDeleteMemory() {
    const target = memoryToDelete;
    if (!target || deletingMemoryRef.current) return;

    deletingMemoryRef.current = target.id;
    setMemoryToDelete(null);
    setDeletingMemoryId(target.id);
    try {
      await api.deleteMemory(target.id);
      await load();
    } catch (error) {
      notify("error", t("memoryDeleteFailed"), errorDetail(error, t("unknownAiError")));
    } finally {
      deletingMemoryRef.current = null;
      setDeletingMemoryId(null);
    }
  }

  if (loadState === "loading") return <div className="knowledge-layout"><Skeleton className="skeleton-card skeleton-wide" /><Skeleton className="skeleton-card" /></div>;
  if (loadState === "error") return <div className="knowledge-layout"><section className="content-card knowledge-error" role="alert">
    <WarningCircle weight="duotone" aria-hidden="true" />
    <h2>{t("knowledgeUnavailable")}</h2>
    <p>{t("knowledgeUnavailableHint")}</p>
    {loadError && <p className="muted-copy">{loadError}</p>}
    <Button variant="primary" onClick={() => void load()}>{t("retry")}</Button>
  </section></div>;
  if (providers.length === 0) return <EmptyState title={t("noProviders")} body={t("noProvidersHint")} actions={onOpenSettings ? <Button variant="primary" type="button" onClick={onOpenSettings}>{t("openAiSettings")}</Button> : undefined} />;

  return <div className="knowledge-layout">
    <section className="content-card knowledge-toolbar"><div className="section-heading"><div><p className="eyebrow">{t("providerContext")}</p><h2>{t("projectKnowledge")}</h2></div><Brain weight="duotone" /></div>
      <div className="provider-bar"><label className="field-group"><span>{t("aiProviders")}</span><select value={providerId} onChange={(event) => setProviderId(event.target.value)}>{providers.map((provider) => <option key={provider.id} value={provider.id}>{provider.name} · {provider.model}</option>)}</select></label><label className="field-group"><span><Key />{t("apiKey")}</span><input type="password" autoComplete="off" value={apiKey} placeholder={t("apiKeyHint")} onChange={(event) => setApiKey(event.target.value)} /></label></div>
      <p className="privacy-note">{t("providerPrivacyHint")}</p>
    </section>
    <section className="content-card summary-card"><div className="section-heading"><div><p className="eyebrow">{t("regenerable")}</p><h2>{t("summary")}</h2></div><Button variant="primary" loading={busy === "summary"} onClick={() => void prepareAnalysis({ kind: "summary" })}><Sparkle weight="fill" />{summary ? t("regenerate") : t("generateSummary")}</Button></div>
      {summary ? <div className="summary-copy"><p>{summary.text}</p><span>{summary.model ?? "AI"} · {formatDate(summary.createdAt)}</span></div> : <EmptyState compact title={t("noSummary")} body={t("noSummaryHint")} />}
      {summaries.length > 0 && <div className="summary-history"><span>{t("summaryHistory")}</span>{summaries.map((item) => <div key={item.id}><p>{item.text}</p><div className="summary-history-actions"><Button size="sm" variant="quiet" onClick={() => setEvidenceSummary(item)}>{t("evidenceSnapshot")}</Button><Button size="sm" variant="quiet" onClick={() => setSummary(item)}>{t("restoreConversation")}</Button><Button size="sm" variant="quiet" onClick={() => void api.acceptSummaryMemory(item.id).then(() => load()).then(() => notify("success", t("memorySaved"))).catch((error) => notify("error", t("memorySaveFailed"), errorDetail(error, t("unknownAiError"))))}>{t("acceptAsMemory")}</Button></div></div>)}</div>}
    </section>
    <section className="content-card qa-card"><div className="section-heading"><div><p className="eyebrow">{t("conversation")}</p><h2>{t("projectQa")}</h2></div><ChatCircleDots weight="duotone" /></div>
      {answer && <div className="answer-bubble">{answer}</div>}
      <form className="question-form" onSubmit={(event) => { event.preventDefault(); void prepareAnalysis({ kind: "ask", question: question.trim() }); }}><label><span className="sr-only">{t("askHint")}</span><textarea value={question} placeholder={t("askHint")} onChange={(event) => setQuestion(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); void prepareAnalysis({ kind: "ask", question: question.trim() }); } }} /></label><Button type="submit" variant="primary" loading={busy === "ask"} disabled={!question.trim()}>{t("ask")}</Button></form>
    </section>
    <section className="content-card memory-card"><div className="section-heading"><div><p className="eyebrow">{t("userControlled")}</p><h2>{t("aiMemory")}</h2></div><FloppyDisk weight="duotone" /></div>
      <form className="inline-form" onSubmit={(event) => { event.preventDefault(); void saveMemory(); }}><label><span>{t("memoryDraft")}</span><input value={memoryDraft} onChange={(event) => setMemoryDraft(event.target.value)} /></label><Button type="submit" loading={busy === "memory"} disabled={!memoryDraft.trim()}>{t("save")}</Button></form>
      {memory.length === 0 ? <p className="muted-copy">{t("noMemory")}</p> : <div className="memory-list">{memory.map((item) => <div key={item.id}><span>{item.text}</span><Button variant="quiet" size="icon" aria-label={t("remove")} loading={deletingMemoryId === item.id} disabled={Boolean(deletingMemoryId)} onClick={() => setMemoryToDelete(item)}><Trash /></Button></div>)}</div>}
    </section>
    <ConfirmDialog
      open={Boolean(pendingAiAction && analysisPlan)}
      title={t("reviewAiPlan")}
      body={analysisPlan ? formatAnalysisPlan(analysisPlan, t) : ""}
      confirmLabel={t("sendForAnalysis")}
      cancelLabel={t("cancel")}
      busy={Boolean(pendingAiAction && busy === pendingAiAction.kind)}
      onOpenChange={(open) => { if (!open && !busy) { setPendingAiAction(null); setAnalysisPlan(null); } }}
      onConfirm={confirmAnalysis}
    />
    <ConfirmDialog
      open={Boolean(memoryToDelete)}
      title={t("removeAiMemory")}
      body={memoryToDelete ? `${memoryToDelete.text}\n${t("irreversibleHint")}` : ""}
      confirmLabel={t("remove")}
      cancelLabel={t("cancel")}
      onOpenChange={(open) => {
        if (!open && !deletingMemoryRef.current) setMemoryToDelete(null);
      }}
      onConfirm={() => void confirmDeleteMemory()}
    />
    <ConfirmDialog
      open={Boolean(evidenceSummary)}
      title={t("evidenceSnapshot")}
      body={evidenceSummary?.evidenceSnapshot || t("noEvidenceSnapshot")}
      confirmLabel={t("close")}
      cancelLabel={t("close")}
      onOpenChange={(open) => !open && setEvidenceSummary(null)}
      onConfirm={() => setEvidenceSummary(null)}
    />
  </div>;
}

function formatDate(value: string) { try { return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(value)); } catch { return value; } }

function formatAnalysisPlan(plan: AnalysisPlan, t: (key: MessageKey) => string) {
  const destination = `${plan.isLocal ? t("localProcessing") : t("externalProcessing")}: ${plan.providerName} · ${plan.model}${plan.baseUrl ? `\n${plan.baseUrl}` : ""}`;
  const evidence = plan.evidenceFiles.length > 0 ? plan.evidenceFiles.map((file) => `• ${file}`).join("\n") : "—";
  return `${t("analysisPlanHint")}\n\n${destination}\n\n${t("evidenceScope")}\n${evidence}\n\n${t("characterCount")}: ${plan.characterCount.toLocaleString()}\n${t("secretsRedacted")}: ${plan.suspiciousSecretCount}`;
}

function errorDetail(error: unknown, fallback: string) {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  return fallback;
}
