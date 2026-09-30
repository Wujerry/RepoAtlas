import { MotionConfig } from "framer-motion";
import { useCallback, useEffect, useRef, useState } from "react";
import { dictionaries, resolveLocale, type MessageKey } from "./i18n";
import { api } from "./lib/api";
import { connectQuickSearchRequests, quickSearchApi } from "./lib/quick-search";
import type { AppSettings, ProjectSummary } from "./types";
import { CommandPalette } from "./components/CommandPalette";
import { QuickSearchShortcut } from "./components/QuickSearchShortcut";

/** A small entry point: no project tree, task monitors, onboarding or index refresh. */
export default function QuickSearchApp() {
  const [settings, setSettings] = useState<AppSettings>({ theme: "system", locale: "system", uiFont: "", consoleFont: "" });
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [generation, setGeneration] = useState(0);
  const currentGeneration = useRef(0);
  const [visible, setVisible] = useState(false);
  const [error, setError] = useState("");
  const [systemLight, setSystemLight] = useState(() => matchMedia("(prefers-color-scheme: light)").matches);
  const locale = resolveLocale(settings.locale, navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en");
  const t = useCallback((key: MessageKey) => dictionaries[locale][key], [locale]);

  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: light)");
    const update = () => setSystemLight(media.matches);
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  useEffect(() => {
    document.documentElement.dataset.theme = settings.theme === "system" ? (systemLight ? "light" : "dark") : settings.theme;
    document.documentElement.lang = locale === "zh" ? "zh-CN" : "en";
    document.documentElement.style.setProperty("--ui-font", settings.uiFont?.trim() ? `"${settings.uiFont.trim()}", var(--font-sans)` : "var(--font-sans)");
    document.title = t("qsTitle");
  }, [settings, systemLight, locale, t]);
  useEffect(() => {
    let active = true;
    const connection = connectQuickSearchRequests(async request => {
      if (!active || request.action !== "focus") return;
      const requestGeneration = ++currentGeneration.current;
      setError(""); setProjects([]); setGeneration(requestGeneration); setVisible(true);
      const results = await Promise.allSettled([api.getSettings(), api.listProjects({ section: "recent", limit: 8 })]);
      if (!active || currentGeneration.current !== requestGeneration) return;
      const [preferences, recent] = results;
      if (preferences.status === "fulfilled") setSettings(preferences.value);
      if (recent.status === "fulfilled") setProjects(recent.value);
      const failures = results.filter(result => result.status === "rejected").map(result => String(result.reason));
      if (failures.length) setError(failures.join(" · "));
    }, failure => { if (active) setError(failure.message); });
    void connection.catch(failure => { if (active) { setError(String(failure)); setVisible(true); } });
    return () => { active = false; currentGeneration.current++; void connection.then(dispose => dispose(), () => undefined); };
  }, []);
  async function close() {
    // An old navigation/close completion cannot dismiss a later global invocation.
    if (currentGeneration.current !== generation) return;
    try { await quickSearchApi.hide(); if (currentGeneration.current === generation) setVisible(false); }
    catch (failure) { if (currentGeneration.current === generation) setError(String(failure)); }
  }
  return <MotionConfig reducedMotion="user"><div className="quick-search-root">
    {error && <p className="quick-search-notice" role="alert">{error}</p>}
    <CommandPalette key={generation} standalone open={visible} onClose={() => void close()} t={t} projects={projects}
      actions={[{ id: "open-ai-history", title: t("ahMore"), run: async () => { await quickSearchApi.openMain({ kind: "sessions" }); } }]}
      onProject={async projectId => { await quickSearchApi.openMain({ kind: "project", projectId }); }}
      onSession={async (session, messageIndex) => { await quickSearchApi.openMain({ kind: "sessions", sessionId: session.id, messageIndex }); }}
      onSources={async () => { await quickSearchApi.openMain({ kind: "sessions", sources: true }); }}
      shortcutHint={<QuickSearchShortcut t={t} compact />} />
  </div></MotionConfig>;
}
