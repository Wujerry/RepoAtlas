import { Dialog } from "@base-ui/react/dialog";
import { Copy, Play } from "@phosphor-icons/react";
import { useEffect, useId, useRef, useState } from "react";
import type { MessageKey } from "../../i18n";
import { sessionDisplayTitle } from "../../lib/session-presentation";
import { sessionApi, type AgentSession, type SessionResumeSpec } from "../../lib/sessions";
import { Button } from "./button";
import { SessionAgentLabel } from "./session-agent";

type T = (key: MessageKey) => string;

/** Shared mapping for cached source reasons and errors returned by resume validation. */
export function sessionResumeReason(key: string | null, t: T): string {
  const keys: Record<string, MessageKey> = {
    session_cli_unavailable: "ahCliUnavailable",
    session_cli_probe_timeout: "ahCliProbeTimeout",
    session_source_layout_unknown: "ahSourceLayout",
    session_cwd_missing: "ahCwdMissing",
    session_agent_missing: "ahAgentMissing",
    session_source_missing: "ahSourceMissing",
    source_disabled: "qsSourceDisabled",
    session_not_found: "qsSessionMissing",
    invalid_session_id: "ahInvalidId",
  };
  return key && Object.prototype.hasOwnProperty.call(keys, key) ? t(keys[key]) : key ?? "";
}

export interface SessionResumeButtonProps {
  session: AgentSession;
  t: T;
  onError?: (error: string) => void;
  label?: string;
}

/** Also resets when a refreshed session changes the source or validated launch context. */
export function SessionResumeButton(props: SessionResumeButtonProps) {
  const { session } = props;
  const identity = JSON.stringify([
    session.id, session.revision, session.sourceId, session.adapter, session.externalId,
    session.cwd, session.sourceMissing, session.resumeReason, session.capabilities.directResume,
  ]);
  return <SessionResumeControl key={identity} {...props} />;
}

function SessionResumeControl({ session, t, onError, label }: SessionResumeButtonProps) {
  const targetName = useId();
  const reasonId = useId();
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [spec, setSpec] = useState<SessionResumeSpec>();
  const [target, setTarget] = useState<"cli" | "app">("cli");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const guard = useRef(false);
  const mounted = useRef(false);
  const sequence = useRef(0);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; sequence.current += 1; };
  }, []);

  const unavailable = session.sourceMissing ? "session_source_missing" : session.resumeReason;
  const canResume = session.capabilities.directResume && !unavailable;
  const reason = [...new Set([session.sourceMissing ? "session_source_missing" : null, session.resumeReason])]
    .filter((value): value is string => Boolean(value)).map(value => sessionResumeReason(value, t)).join(" ")
    || (!canResume ? t("ahInvalidId") : "");
  const command = target === "app" ? `codex://threads/${session.externalId}` : spec?.command ?? "";

  async function run<Result>(operation: () => Promise<Result>, complete: (value: Result) => void, invalidateSpec = false) {
    if (guard.current || !mounted.current) return;
    guard.current = true;
    const request = ++sequence.current;
    const current = () => mounted.current && sequence.current === request;
    setBusy(true); setError(""); setNotice("");
    try {
      const result = await operation();
      if (current()) complete(result);
    } catch (failure) {
      if (current()) {
        if (invalidateSpec) setSpec(undefined);
        const message = sessionResumeReason(failure instanceof Error ? failure.message : String(failure), t);
        setError(message);
        onError?.(message);
      }
    } finally {
      if (current()) { guard.current = false; setBusy(false); }
    }
  }

  function validate() {
    if (!canResume || guard.current || !mounted.current) return;
    setOpen(true); setSpec(undefined); setTarget("cli");
    void run(() => sessionApi.resumeSpec(session.id), setSpec);
  }

  function launch() {
    // A failed/stale spec must never become a launch or a fallback to a new conversation.
    if (!canResume || !spec || (target === "app" && !spec.app?.canResume)) return;
    void run(() => sessionApi.resume(session.id, target), () => setNotice(t("ahStarted")), true);
  }

  return <span className="search-preview-resume">
    <Button type="button" variant="primary" disabled={!canResume || busy} loading={busy}
      title={reason || undefined} aria-describedby={reason ? reasonId : undefined} onClick={validate}>
      <Play weight="fill" aria-hidden="true" />{label ?? t("ahResume")}
    </Button>
    {reason && <span id={reasonId} className="search-preview-resume-reason" role="status">{reason}</span>}
    <Dialog.Root open={open} onOpenChange={value => { if (!guard.current) setOpen(value); }}>
      <Dialog.Portal>
        <Dialog.Backdrop className="dialog-backdrop search-preview-launch-backdrop" />
        <Dialog.Popup className="dialog-popup search-preview-launch-dialog">
          <Dialog.Title className="dialog-title">{t("ahResume")}</Dialog.Title>
          <Dialog.Description className="dialog-description">{t("ahLaunchHint")}</Dialog.Description>
          <strong>{sessionDisplayTitle(session.title, t("ahUntitled"))}</strong>
          <p><SessionAgentLabel adapter={session.adapter} /> · <code>{session.externalId}</code></p>
          {spec && <>
            <fieldset disabled={busy} className="search-preview-launch-targets">
              <legend>{t("ahLaunchTarget")}</legend>
              <label><input type="radio" name={targetName} checked={target === "cli"} onChange={() => { if (!guard.current) setTarget("cli"); }} /> CLI / Terminal</label>
              {spec.app && <label><input type="radio" name={targetName} disabled={!spec.app.canResume} checked={target === "app"}
                onChange={() => { if (spec.app?.canResume && !guard.current) setTarget("app"); }} />
                <SessionAgentLabel adapter={session.adapter} name={spec.app.name} /> / App
              </label>}
            </fieldset>
            {spec.app && !spec.app.canResume && <p className="search-preview-launch-note">{t("ahAppUnsupported")}</p>}
            <small>{t("ahLaunchCwd")}</small><code className="search-preview-launch-path">{spec.cwd}</code>
            <small>{t("ahLaunchCommand")}</small><pre className="search-preview-launch-command">{command}</pre>
            <Button type="button" variant="quiet" disabled={busy} onClick={() => void run(
              () => navigator.clipboard.writeText(command), () => setNotice(t("ahCopied")),
            )}><Copy aria-hidden="true" />{t("ahCopy")}</Button>
          </>}
          {busy && <p role="status">{t("ahPending")}</p>}
          {error && <p role="alert">{error}</p>}
          {notice && <p role="status">{notice}</p>}
          {error && !spec && <Button type="button" variant="quiet" disabled={busy} onClick={validate}>{t("retry")}</Button>}
          <div className="dialog-actions">
            <Dialog.Close render={<Button type="button" disabled={busy}>{t("close")}</Button>} />
            <Button type="button" variant="primary" disabled={busy || !spec || !canResume} loading={busy} onClick={launch}>
              <Play weight="fill" aria-hidden="true" />{t("ahLaunchNow")}
            </Button>
          </div>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  </span>;
}
