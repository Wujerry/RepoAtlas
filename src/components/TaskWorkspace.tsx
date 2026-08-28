import { ArrowDown, ArrowUp, CaretDown, CaretUp, Copy, Play, Plus, Square, Trash, X } from "@phosphor-icons/react";
import { useEffect, useRef, useState, type MouseEvent } from "react";
import type { MessageKey } from "../i18n";
import { parseAnsi } from "../lib/ansi";
import { formatTime } from "../lib/format";
import type { ProjectDetail, TaskRun, ToastTone } from "../types";
import { Button } from "./ui/button";
import { ConfirmDialog } from "./ui/confirm";
import { EmptyState, Skeleton } from "./ui/feedback";
import { formatCommand, InlineLoadError, statusKey } from "./DashboardShared";

export function TaskWorkspace({ tasks, runs, loading, error, running, activeRunId, log, t, notify, onRetry, onSaveTasks, onRun, onStop, onClearLog, onHistory, onSendInput }: { tasks: ProjectDetail["tasks"]; runs: TaskRun[]; loading: boolean; error?: string; running?: TaskRun; activeRunId?: string; log: string; t: (key: MessageKey) => string; notify: (tone: ToastTone, title: string, detail?: string) => void; onRetry?: () => void; onSaveTasks: (tasks: ProjectDetail["tasks"]) => Promise<void>; onRun: (id: string) => void; onStop: () => void; onClearLog: () => void; onHistory: (run: TaskRun) => void; onSendInput?: (runId: string, text: string) => void | Promise<void>; }) {
  type TaskDraft = { name: string; description: string; kind: string; executable: string; argv: string[]; cwd: string };
  const [draft, setDraft] = useState<TaskDraft>({ name: "", description: "", kind: "run", executable: "", argv: [], cwd: "" });
  const [editor, setEditor] = useState<{ mode: "new" } | { mode: "edit"; taskId: string } | null>(null);
  const [savingTask, setSavingTask] = useState(false);
  const [consoleOpen, setConsoleOpen] = useState(false);
  const [inputDraft, setInputDraft] = useState("");
  const [sendingInput, setSendingInput] = useState(false);
  const [pendingRemoval, setPendingRemoval] = useState<ProjectDetail["tasks"][number]>();
  const [removingTask, setRemovingTask] = useState(false);
  const logPaneRef = useRef<HTMLPreElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const editorRef = useRef<HTMLFormElement>(null);
  const editorNameRef = useRef<HTMLInputElement>(null);
  const editorReturnFocusRef = useRef<HTMLButtonElement | null>(null);
  const activeRun = runs.find((run) => run.id === activeRunId);
  const consoleTitle = running?.kind ?? activeRun?.kind ?? t("taskConsole");
  useEffect(() => {
    if (!running) return;
    setConsoleOpen(true);
    window.setTimeout(() => inputRef.current?.focus(), 0);
  }, [running?.id]);
  useEffect(() => {
    const pane = logPaneRef.current;
    if (!pane || !consoleOpen) return;
    pane.scrollTop = pane.scrollHeight;
  }, [consoleOpen, log]);
  useEffect(() => {
    if (!editor) return;
    const frame = requestAnimationFrame(() => {
      editorRef.current?.scrollIntoView({ block: "nearest", inline: "nearest" });
      editorNameRef.current?.focus();
    });
    return () => cancelAnimationFrame(frame);
  }, [editor]);
  useEffect(() => {
    if (editor?.mode === "edit" && !tasks.some((task) => task.id === editor.taskId)) setEditor(null);
  }, [editor, tasks]);

  function openNewTask(event: MouseEvent<HTMLButtonElement>) {
    editorReturnFocusRef.current = event.currentTarget;
    const preferredExecutable = tasks.find((task) => task.inferred)?.executable ?? tasks[0]?.executable ?? "";
    setDraft({ name: "", description: "", kind: "run", executable: preferredExecutable, argv: [], cwd: "" });
    setEditor({ mode: "new" });
  }

  function openTaskEditor(task: ProjectDetail["tasks"][number], event: MouseEvent<HTMLButtonElement>) {
    editorReturnFocusRef.current = event.currentTarget;
    setDraft({ name: task.name, description: task.description ?? "", kind: task.kind, executable: task.executable, argv: [...task.argv], cwd: task.cwd ?? "" });
    setEditor({ mode: "edit", taskId: task.id });
  }

  function closeTaskEditor() {
    const returnFocus = editorReturnFocusRef.current;
    const editorCard = editorRef.current?.closest<HTMLElement>(".task-card");
    setEditor(null);
    setDraft({ name: "", description: "", kind: "run", executable: "", argv: [], cwd: "" });
    requestAnimationFrame(() => (editorCard?.querySelector<HTMLButtonElement>("[data-task-edit]") ?? returnFocus)?.focus());
  }

  async function saveTask() {
    if (!editor || savingTask) return;
    const name = draft.name.trim();
    const executable = draft.executable.trim();
    if (!name || !executable) return;
    // Keep argv as an array all the way to Rust. This preserves arguments that
    // contain spaces (for example, a message or a path) without reparsing a
    // shell-like string and accidentally changing the command semantics.
    const argv = [...draft.argv];
    const next = editor.mode === "edit"
      ? tasks.map((task) => task.id === editor.taskId ? { ...task, name, description: draft.description.trim() || null, kind: draft.kind, executable, argv, cwd: draft.cwd.trim() || null, inferred: false, shellMode: false } : task)
      : [...tasks, { id: crypto.randomUUID(), name, description: draft.description.trim() || null, kind: draft.kind, executable, argv, cwd: draft.cwd.trim() || null, inferred: false, shellMode: false }];
    setSavingTask(true);
    try {
      await onSaveTasks(next);
      closeTaskEditor();
      notify("success", t("taskSaved"));
    } catch (error) { notify("error", t("saveFailed"), String(error)); }
    finally { setSavingTask(false); }
  }
  function updateArgument(index: number, value: string) {
    setDraft((current) => ({ ...current, argv: current.argv.map((argument, itemIndex) => itemIndex === index ? value : argument) }));
  }
  function addArgument() {
    setDraft((current) => ({ ...current, argv: [...current.argv, ""] }));
  }
  function removeArgument(index: number) {
    setDraft((current) => ({ ...current, argv: current.argv.filter((_, itemIndex) => itemIndex !== index) }));
  }
  function moveArgument(index: number, direction: -1 | 1) {
    setDraft((current) => {
      const nextIndex = index + direction;
      if (nextIndex < 0 || nextIndex >= current.argv.length) return current;
      const argv = [...current.argv];
      [argv[index], argv[nextIndex]] = [argv[nextIndex], argv[index]];
      return { ...current, argv };
    });
  }
  async function copyOutput() {
    try {
      await navigator.clipboard.writeText(log);
      notify("success", t("outputCopied"));
    } catch (error) { notify("error", t("copyFailed"), String(error)); }
  }
  async function sendInput() {
    const value = inputDraft;
    if (!running || !onSendInput || sendingInput) return;
    setSendingInput(true);
    try {
      const payload = value.endsWith("\n") ? value : `${value}\n`;
      await onSendInput(running.id, payload);
      setInputDraft("");
    } catch (error) {
      notify("error", t("inputFailed"), String(error));
    } finally {
      setSendingInput(false);
      inputRef.current?.focus();
    }
  }

  function renderTaskEditor(inline = false) {
    const editingTask = editor?.mode === "edit" ? tasks.find((task) => task.id === editor.taskId) : undefined;
    return (
      <form id={editingTask ? `task-editor-${editingTask.id}` : "task-editor-new"} ref={editorRef} aria-label={editor?.mode === "edit" ? t("editTask") : t("addTask")} className={`task-editor${inline ? " task-editor-inline" : " task-editor-new"}`} onKeyDown={(event) => { if (event.key === "Escape" && !savingTask) { event.preventDefault(); closeTaskEditor(); } }} onSubmit={(event) => { event.preventDefault(); void saveTask(); }}>
        <div className="task-editor-heading">
          <span className="task-icon"><Play weight="fill" /></span>
          <div><p className="eyebrow">{editingTask?.kind ?? t("customTask")}</p><h3>{editor?.mode === "edit" ? editingTask?.name || t("editTask") : t("addTask")}</h3></div>
        </div>
        <div className="settings-fields">
          <label className="field-group"><span>{t("taskName")}</span><input ref={editorNameRef} value={draft.name} onChange={(event) => setDraft({ ...draft, name: event.target.value })} /></label>
          <label className="field-group field-group-wide"><span>{t("taskDescription")}</span><input value={draft.description} onChange={(event) => setDraft({ ...draft, description: event.target.value })} placeholder={t("taskDescriptionHint")} /></label>
          <label className="field-group"><span>{t("taskKind")}</span><input value={draft.kind} onChange={(event) => setDraft({ ...draft, kind: event.target.value })} /></label>
          <label className="field-group"><span>{t("executable")}</span><input value={draft.executable} onChange={(event) => setDraft({ ...draft, executable: event.target.value })} /></label>
           <div className="field-group field-group-wide task-arguments-field">
             <div className="task-arguments-heading"><span>{t("argumentsLabel")}</span><Button type="button" size="sm" variant="quiet" onClick={addArgument}><Plus aria-hidden="true" />{t("addArgument")}</Button></div>
             {draft.argv.length === 0 ? <p className="task-arguments-empty">{t("noArguments")}</p> : <div className="task-arguments-list" role="list" aria-label={t("argumentsLabel")}>
               {draft.argv.map((argument, index) => <div className="task-argument-row" role="listitem" key={`argument-${index}`}>
                 <span className="task-argument-index" aria-hidden="true">{index + 1}</span>
                 <label className="sr-only" htmlFor={`task-argument-${index}`}>{`${t("argumentsLabel")} ${index + 1}`}</label>
                 <input id={`task-argument-${index}`} value={argument} onChange={(event) => updateArgument(index, event.target.value)} placeholder={t("argumentPlaceholder")} />
                 <div className="task-argument-actions">
                   <Button type="button" size="icon" variant="quiet" aria-label={`${t("moveArgumentUp")} ${index + 1}`} disabled={index === 0} onClick={() => moveArgument(index, -1)}><ArrowUp aria-hidden="true" /></Button>
                   <Button type="button" size="icon" variant="quiet" aria-label={`${t("moveArgumentDown")} ${index + 1}`} disabled={index === draft.argv.length - 1} onClick={() => moveArgument(index, 1)}><ArrowDown aria-hidden="true" /></Button>
                   <Button type="button" size="icon" variant="quiet" aria-label={`${t("removeArgument")} ${index + 1}`} onClick={() => removeArgument(index)}><X aria-hidden="true" /></Button>
                 </div>
               </div>)}
             </div>}
             <span className="field-hint">{t("argumentHint")}</span>
           </div>
          <label className="field-group field-group-wide"><span>{t("workingDirectory")}</span><input value={draft.cwd} onChange={(event) => setDraft({ ...draft, cwd: event.target.value })} /></label>
        </div>
        <div className="settings-actions">
          <Button type="submit" variant="primary" loading={savingTask} disabled={savingTask || !draft.name.trim() || !draft.executable.trim()}>{t("save")}</Button>
          <Button type="button" disabled={savingTask} onClick={closeTaskEditor}>{t("cancel")}</Button>
        </div>
      </form>
    );
  }
  return (
    <div className="task-layout">
      <div className="task-main">
        <section className="content-card task-catalog">
          <div className="section-heading">
            <div><p className="eyebrow">{t("detectedTasks")}</p><h2>{t("tasks")}</h2></div>
            <div className="task-heading-actions">
              <span>{tasks.length}</span>
              <Button variant="quiet" disabled={Boolean(editor) || savingTask} onClick={openNewTask}>{t("addTask")}</Button>
            </div>
          </div>
          {editor?.mode === "new" && renderTaskEditor()}
          {tasks.length === 0 && editor?.mode !== "new" ? (
            <EmptyState compact title={t("noTasks")} body={t("noTasksHint")} actions={<Button variant="primary" onClick={openNewTask}>{t("addTask")}</Button>} />
          ) : (
            <div className="task-grid">
              {tasks.map((task) => {
                const isEditing = editor?.mode === "edit" && editor.taskId === task.id;
                return (
                <article className={"task-card" + (running?.taskId === task.id ? " is-running" : "") + (isEditing ? " is-editing" : "")} key={task.id}>
                  {isEditing ? renderTaskEditor(true) : <>
                    <span className="task-icon"><Play weight="fill" /></span>
                    <div className="task-card-copy">
                      <div className="task-card-kicker">
                        <span className="task-kind-label">{task.kind}</span>
                      </div>
                      <strong>{task.name || task.kind}</strong>
                      {task.description && <p className="task-card-description">{task.description}</p>}
                      <code>{formatCommand(task.executable, task.argv)}</code>
                    </div>
                    <div className="task-card-actions">
                      <Button variant="quiet" data-task-edit aria-expanded={false} aria-controls={`task-editor-${task.id}`} disabled={Boolean(editor) || savingTask} onClick={(event) => openTaskEditor(task, event)}>{t("edit")}</Button>
                      <Button variant="danger" disabled={removingTask || Boolean(editor) || savingTask} onClick={() => setPendingRemoval(task)}>{t("remove")}</Button>
                      <Button variant="primary" disabled={Boolean(running) || Boolean(editor) || savingTask} onClick={() => onRun(task.id)}><Play weight="fill" />{t("run")}</Button>
                    </div>
                  </>}
                </article>
              );})}
            </div>
          )}
        </section>
        <aside className="content-card run-history">
          <div className="section-heading"><div><p className="eyebrow">{t("recentActivity")}</p><h2>{t("history")}</h2></div></div>
          {loading ? <Skeleton className="skeleton-list" /> : error ? <InlineLoadError title={t("tasksLoadFailed")} detail={error} retryLabel={t("retry")} onRetry={onRetry} /> : runs.length === 0 ? <p className="muted-copy">{t("noRunHistory")}</p> : (
            <div className="run-list">
              {runs.map((run) => (
                <button className={activeRunId === run.id ? "active" : ""} key={run.id} onClick={() => { setConsoleOpen(true); onHistory(run); }}>
                  <span className={"run-dot run-" + run.status} />
                  <strong>{run.kind}</strong>
                  <span>{t(statusKey(run.status))}</span>
                  <time>{formatTime(run.startedAt)}</time>
                </button>
              ))}
            </div>
          )}
        </aside>
      </div>
      <section className={"task-console " + (consoleOpen ? "is-open" : "is-collapsed")} aria-label={t("taskConsole")}>
        <div className="task-console-bar">
          <button className="task-console-toggle" type="button" aria-expanded={consoleOpen} aria-label={consoleOpen ? t("collapseOutput") : t("expandOutput")} onClick={() => setConsoleOpen((open) => !open)}>
            {consoleOpen ? <CaretDown aria-hidden="true" /> : <CaretUp aria-hidden="true" />}
            <span className={"run-dot run-" + (running ? "running" : (activeRun?.status ?? "idle"))} />
            <strong>{consoleTitle}</strong>
            <span className="muted-copy">{running ? t("live") : t("output")}</span>
          </button>
          <div className="task-console-actions">
            {running && <Button variant="danger" onClick={onStop}><Square weight="fill" />{t("stop")}</Button>}
            <Button variant="quiet" disabled={!log} onClick={() => { onClearLog(); notify("info", t("outputCleared")); }}><Trash />{t("clearOutput")}</Button>
            <Button variant="quiet" disabled={!log} onClick={() => void copyOutput()}><Copy />{t("copyOutput")}</Button>
          </div>
        </div>
        {consoleOpen && (
          <div className="task-console-body">
            <pre ref={logPaneRef} className="log-pane">{log ? parseAnsi(log).map((span, index) => <span className={span.className} key={`${index}-${span.text.length}`}>{span.text}</span>) : t("logEmpty")}</pre>
            <form className="task-console-input" onSubmit={(event) => { event.preventDefault(); void sendInput(); }}>
              <label className="sr-only" htmlFor="task-console-input">{t("taskInput")}</label>
              <input
                id="task-console-input"
                ref={inputRef}
                value={inputDraft}
                onChange={(event) => setInputDraft(event.target.value)}
                placeholder={running ? t("taskInputHint") : t("logEmpty")}
                disabled={!running || sendingInput}
                autoComplete="off"
                spellCheck={false}
              />
              <Button type="submit" variant="primary" disabled={!running || sendingInput} loading={sendingInput}>{t("sendInput")}</Button>
            </form>
          </div>
        )}
      </section>
      <ConfirmDialog open={Boolean(pendingRemoval)} title={t("confirmRemoveTask")} body={pendingRemoval ? `${pendingRemoval.name}\n${formatCommand(pendingRemoval.executable, pendingRemoval.argv)}` : ""} confirmLabel={t("remove")} cancelLabel={t("cancel")} busy={removingTask} onOpenChange={(open) => !open && !removingTask && setPendingRemoval(undefined)} onConfirm={async () => { if (!pendingRemoval) return; setRemovingTask(true); try { await onSaveTasks(tasks.filter((item) => item.id !== pendingRemoval.id)); notify("success", t("taskRemoved")); setPendingRemoval(undefined); } catch (error) { notify("error", t("saveFailed"), String(error)); } finally { setRemovingTask(false); } }} />
    </div>
  );
}
