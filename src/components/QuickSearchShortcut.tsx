import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { MessageKey } from "../i18n";
import { onQuickSearchStatusChanged, quickSearchApi, type QuickSearchStatus } from "../lib/quick-search";
import { Button } from "./ui/button";

export function QuickSearchShortcut({ t, compact = false }: { t: (key: MessageKey) => string; compact?: boolean }) {
  const [status, setStatus] = useState<QuickSearchStatus>();
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (!isTauri()) return;
    let active = true;
    void quickSearchApi.status().then(value => { if (active) setStatus(value); }).catch(failure => { if (active) setError(String(failure)); });
    const listener = onQuickSearchStatusChanged(value => { if (active) { setStatus(value); setError(""); } });
    void listener.catch(failure => { if (active) setError(String(failure)); });
    return () => { active = false; void listener.then(dispose => dispose(), () => undefined); };
  }, []);
  if (!isTauri()) return null;
  async function run(operation: () => Promise<unknown>) {
    setBusy(true); setError("");
    try { await operation(); setStatus(await quickSearchApi.status()); }
    catch (failure) { setError(String(failure)); }
    finally { setBusy(false); }
  }
  const failure = error || status?.error;
  const shortcut = status?.shortcut ?? (/mac/i.test(navigator.userAgent) ? "Cmd+Shift+K" : "Ctrl+Shift+K");
  if (compact) return <span title={failure ? `${t("qsShortcutUnavailable")} ${failure}` : t("qsShortcut")}>
    {status?.registered && !failure ? shortcut : <span role="status">{t("qsShortcutUnavailable")}</span>}
  </span>;
  return <section className="quick-search-settings" aria-label={t("qsShortcut")}>
    <h3>{t("qsShortcut")}</h3><p><code>{shortcut}</code> · {t("qsTitle")}</p>
    {(!status?.registered || failure) && <p role="status">{t("qsShortcutUnavailable")} {failure}</p>}
    <div className="quick-search-settings-actions">
      <Button loading={busy} onClick={() => void run(() => quickSearchApi.show())}>{t("qsOpenWindow")}</Button>
      <Button disabled={busy} onClick={() => void run(() => quickSearchApi.retryRegistration())}>{t("qsRetryShortcut")}</Button>
    </div>
  </section>;
}
