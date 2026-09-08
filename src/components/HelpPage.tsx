import { ArrowLeft, Copy, Database, Info, ShieldCheck, TerminalWindow } from "@phosphor-icons/react";
import { useEffect, useMemo, useState } from "react";
import type { MessageKey } from "../i18n";
import { api } from "../lib/api";
import { buildAgentScanInstruction, buildAgentSetupInstruction, buildMcpConfig } from "../lib/mcp-setup";
import type { McpSetupInfo, ToastTone } from "../types";
import { Button } from "./ui/button";
import { Skeleton } from "./ui/feedback";

export function HelpPage({ locale, t, notify, scanning = false, onReplayOnboarding, onBack }: { locale: "zh" | "en"; t: (key: MessageKey) => string; notify: (tone: ToastTone, title: string, detail?: string) => void; scanning?: boolean; onReplayOnboarding?: () => void; onBack?: () => void }) {
  const [info, setInfo] = useState<McpSetupInfo>();
  const [error, setError] = useState<string>();

  useEffect(() => {
    let active = true;
    api.mcpSetupInfo().then((value) => active && setInfo(value)).catch((reason) => active && setError(String(reason)));
    return () => { active = false; };
  }, []);

  const config = useMemo(() => buildMcpConfig(info), [info]);
  const agentPrompt = useMemo(() => buildAgentSetupInstruction(info, locale), [info, locale]);

  const scanPrompt = useMemo(() => buildAgentScanInstruction(info, locale), [info, locale]);

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      notify("success", t("copied"));
    } catch (reason) {
      notify("error", t("copyFailed"), String(reason));
    }
  }

  return <main id="main-content" className="help-page">
    <header className="help-header">{onBack ? <Button type="button" className="settings-back" variant="quiet" onClick={onBack}><ArrowLeft aria-hidden="true" />{t("backToProjects")}</Button> : null}<p className="eyebrow">RepoAtlas Guide</p><h1 id="help-page-title">{t("help")}</h1><p>{t("helpIntro")}</p></header>
    <div className="help-content">
      <section className="help-card help-mcp">
        <h2>{t("helpInitializePrompt")}</h2>
        <div className="help-code-block"><div><strong>{t("agentHandoff")}</strong><Button disabled={!agentPrompt} onClick={() => void copy(agentPrompt)}><Copy />{t("copyForAgent")}</Button></div><pre>{agentPrompt || error || t("mcpConfigurationBody")}</pre></div>
        <h2>{t("helpScanPrompt")}</h2><p>{t("helpScanPromptHint")}</p>
        <div className="help-code-block"><div><strong>{t("helpScanPrompt")}</strong><Button disabled={!scanPrompt} onClick={() => void copy(scanPrompt)}><Copy />{t("copyForAgent")}</Button></div><pre>{scanPrompt || error || t("workspaceNotFoundHint")}</pre></div>
      </section>
      <section className="help-card"><Info className="help-card-icon" /><div><h2>{t("howItWorks")}</h2><p>{t("howItWorksBody")}</p>{onReplayOnboarding ? <div className="help-card-action"><Button disabled={scanning} onClick={onReplayOnboarding}>{t("onboardingReplay")}</Button><p>{scanning ? t("onboardingReplayBusy") : t("onboardingReplayHint")}</p></div> : null}</div></section>
      <section className="help-card"><ShieldCheck className="help-card-icon" /><div><h2>{t("safetyBoundary")}</h2><p>{t("safetyBoundaryBody")}</p></div></section>
      <section className="help-card help-mcp">
        <div className="help-section-heading"><div><TerminalWindow className="help-card-icon" /><div><p className="eyebrow">MCP · stdio</p><h2>{t("mcpConfiguration")}</h2></div></div><p>{t("mcpConfigurationBody")}</p></div>
        {error ? <p className="help-error">{error}</p> : !info ? <Skeleton className="skeleton-card" /> : <>
          <div className="help-data-line"><Database /><span>{t("sharedDatabase")}</span><code>{info.dbPath}</code></div>
          {!info.binaryPath && <p className="help-warning">{t("workspaceNotFound")}</p>}
          <div className="help-code-block"><div><strong>{t("configurationTemplate")}</strong><Button size="sm" disabled={!config} onClick={() => void copy(config)}><Copy />{t("copyConfig")}</Button></div><pre>{config || t("workspaceNotFoundHint")}</pre></div>

        </>}
      </section>
    </div>
  </main>;
}
