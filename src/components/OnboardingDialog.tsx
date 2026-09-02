import { Dialog } from "@base-ui/react/dialog";
import { CheckCircle, Copy, FolderSimple, Path, SpinnerGap, TerminalWindow } from "@phosphor-icons/react";
import { AnimatePresence, motion } from "framer-motion";
import { useEffect, useMemo, useRef, useState } from "react";
import type { MessageKey } from "../i18n";
import { MOTION, MOTION_EASE } from "../lib/motion";
import { buildAgentScanInstruction, mcpLaunchMode } from "../lib/mcp-setup";
import type { McpSetupInfo, ProjectSummary, ScanProgress, ScanRoot, ToastTone } from "../types";
import { Button } from "./ui/button";

export type OnboardingStep = "welcome" | "mcp" | "manual" | "waiting";

export interface OnboardingDialogProps {
  open: boolean;
  step: OnboardingStep;
  t: (key: MessageKey) => string;
  locale: "zh" | "en";
  mcpInfo?: McpSetupInfo;
  mcpError?: string;
  scanRoots: ScanRoot[];
  selectedRootId?: string;
  scanning: boolean;
  progress: ScanProgress | null;
  scanOutcome?: "success" | "empty" | "cancelled" | "failed";
  scanError?: string;
  waiting: boolean;
  checkFailed: boolean;
  foundProject?: ProjectSummary | null;
  notify: (tone: ToastTone, title: string, detail?: string) => void;
  onStep: (step: OnboardingStep) => void;
  onCopyInstruction: () => Promise<boolean> | boolean;
  onChooseRoot: () => void;
  onSelectRoot: (id: string) => void;
  onStartScan: () => void;
  onCancelScan: () => void;
  onCheckNow: () => void;
  onFinish: () => void;
  onSkip: () => void;
  onOpenChange: (open: boolean) => void;
}

const steps: OnboardingStep[] = ["welcome", "mcp", "waiting"];

export function OnboardingDialog({
  open,
  step,
  t,
  locale,
  mcpInfo,
  mcpError,
  scanRoots,
  selectedRootId,
  scanning,
  progress,
  scanOutcome,
  scanError,
  waiting,
  checkFailed,
  foundProject,
  notify,
  onStep,
  onCopyInstruction,
  onChooseRoot,
  onSelectRoot,
  onStartScan,
  onCancelScan,
  onCheckNow,
  onFinish,
  onSkip,
  onOpenChange,
}: OnboardingDialogProps) {
  const [copied, setCopied] = useState(false);
  const mode = mcpLaunchMode(mcpInfo);
  const mcpLoading = !mcpInfo && !mcpError;
  const instruction = useMemo(() => buildAgentScanInstruction(mcpInfo, locale), [locale, mcpInfo]);
  const selectedRoot = scanRoots.find((root) => root.id === selectedRootId) ?? scanRoots[0];
  const closeBlocked = scanning;
  const currentIndex = step === "manual" ? 1 : Math.max(0, steps.indexOf(step));
  const previousIndex = useRef(0);
  const direction = currentIndex >= previousIndex.current ? 1 : -1;
  previousIndex.current = currentIndex;
  const recommendedStep: OnboardingStep = mcpLoading || mode !== "unavailable" ? "mcp" : "manual";

  useEffect(() => {
    setCopied(false);
  }, [step, open]);

  async function copyInstruction() {
    const ok = await onCopyInstruction();
    if (ok) {
      setCopied(true);
      notify("success", t("onboardingCopiedAdd"));
      onStep("waiting");
      return;
    }
    notify("error", t("copyFailed"));
  }

  function requestClose(nextOpen: boolean) {
    if (nextOpen) {
      onOpenChange(true);
      return;
    }
    if (closeBlocked) {
      notify("warning", t("onboardingCloseBlocked"));
      return;
    }
    onSkip();
  }

  return (
    <Dialog.Root open={open} onOpenChange={requestClose}>
      <Dialog.Portal>
        <Dialog.Backdrop className="dialog-backdrop" />
        <Dialog.Popup className="dialog-popup onboarding-dialog" aria-describedby="onboarding-copy">
          <p className="eyebrow">{t("onboardingWelcomeEyebrow")}</p>
          <Dialog.Title className="dialog-title">{t("onboardingTitle")}</Dialog.Title>
          <ol className="onboarding-steps" aria-label={t("onboardingTitle")}>
            {steps.map((value, index) => {
              const label = value === "welcome" ? t("onboardingStepWelcome") : value === "mcp" ? t("onboardingStepMcp") : t("onboardingStepWaiting");
              const active = step === value || (step === "manual" && value === "mcp");
              return (
                <li key={value} className={active ? "is-active" : index < currentIndex ? "is-done" : ""}>
                  <span>{index + 1}</span>
                  {label}
                </li>
              );
            })}
          </ol>

          <AnimatePresence mode="wait" initial={false}>
            <motion.div key={step} initial={{ opacity: 0, x: 8 * direction }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -8 * direction }} transition={{ duration: MOTION.state, ease: MOTION_EASE }}>
          {step === "welcome" ? (
            <div className="onboarding-copy" id="onboarding-copy">
              <p>{t("onboardingWelcomeBody")}</p>
              <p>{t("onboardingWelcomeHint")}</p>
            </div>
          ) : null}

          {step === "mcp" ? (
            <div className="onboarding-copy" id="onboarding-copy">
              <h3>{t("onboardingMcpTitle")}</h3>
              <p>{t("onboardingMcpBody")}</p>
              {mcpError ? <p className="onboarding-error" role="alert">{mcpError}</p> : null}
              {mode === "installed" ? <p className="onboarding-status">{t("onboardingMcpInstalled")}</p> : null}
              {mode === "development" ? <p className="onboarding-warning">{t("onboardingMcpDevelopment")}</p> : null}
              {!mcpLoading && mode === "unavailable" ? <p className="onboarding-warning">{t("onboardingMcpUnavailable")}</p> : null}
              <p>{t("onboardingMcpLaterHint")}</p>
              {instruction ? <pre className="onboarding-instruction">{instruction}</pre> : null}
            </div>
          ) : null}

          {step === "manual" ? (
            <div className="onboarding-copy" id="onboarding-copy">
              <h3>{t("onboardingManualTitle")}</h3>
              <p>{t("onboardingManualBody")}</p>
              <p>{t("onboardingScanHint")}</p>
              {scanRoots.length > 0 ? (
                <div className="onboarding-roots" role="list">
                  {scanRoots.map((root) => (
                    <button
                      key={root.id}
                      type="button"
                      role="listitem"
                      className={root.id === selectedRoot?.id ? "is-selected" : ""}
                      onClick={() => onSelectRoot(root.id)}
                    >
                      <FolderSimple aria-hidden="true" />
                      <code>{root.path}</code>
                    </button>
                  ))}
                </div>
              ) : null}
              {scanning ? (
                <div className="onboarding-scan" role="status" aria-live="polite">
                  <SpinnerGap className="button-spinner" aria-hidden="true" />
                  <div>
                    <strong>{t("scanning")}</strong>
                    <span>{progress?.discovered ?? 0} {t("found")} · {progress?.visited ?? 0} {t("visited")}</span>
                    <code>{progress?.currentPath ?? progress?.rootPath ?? t("preparingScan")}</code>
                  </div>
                </div>
              ) : null}
              {scanOutcome === "success" ? <p className="onboarding-status">{t("onboardingScanComplete")}</p> : null}
              {scanOutcome === "empty" ? <p className="onboarding-warning">{t("onboardingScanEmpty")}</p> : null}
              {scanOutcome === "cancelled" ? <p className="onboarding-warning">{t("onboardingScanCancelled")}</p> : null}
              {scanOutcome === "failed" && scanError ? <p className="onboarding-error" role="alert">{scanError}</p> : null}
            </div>
          ) : null}

          {step === "waiting" ? (
            <div className="onboarding-copy" id="onboarding-copy">
              {foundProject ? (
                <>
                  <h3>{t("onboardingFoundTitle")}</h3>
                  <p><strong>{foundProject.displayName}</strong></p>
                  <code className="onboarding-path">{foundProject.canonicalPath}</code>
                </>
              ) : (
                <>
                  <h3>{t("onboardingWaitingTitle")}</h3>
                  <p>{t("onboardingWaitingBody")}</p>
                  <p className="onboarding-status" role="status" aria-live="polite">
                    {waiting ? t("onboardingChecking") : t("onboardingCheckNow")}
                  </p>
                  {checkFailed ? <p className="onboarding-warning">{t("onboardingCheckFailed")}</p> : null}
                </>
              )}
            </div>
          ) : null}

            </motion.div>
          </AnimatePresence>
          <div className="dialog-actions onboarding-actions">
            {step === "welcome" ? (
              <>
                <Button onClick={onSkip}>{t("onboardingSkip")}</Button>
                <Button variant="primary" onClick={() => onStep(recommendedStep)}>{t("onboardingContinue")}</Button>
              </>
            ) : null}
            {step === "mcp" ? (
              <>
                <Button onClick={() => onStep("welcome")}>{t("onboardingBack")}</Button>
                <Button onClick={() => onStep("manual")}>{t("onboardingManualFallback")}</Button>
                <Button variant="primary" disabled={mcpLoading || mode === "unavailable"} onClick={() => void copyInstruction()}>
                  <Copy aria-hidden="true" />{copied ? t("onboardingCopiedAdd") : t("onboardingCopyAdd")}
                </Button>
              </>
            ) : null}
            {step === "manual" ? (
              <>
                <Button disabled={scanning} onClick={() => onStep("mcp")}>{t("onboardingBack")}</Button>
                {scanning ? <Button onClick={onCancelScan}>{t("cancel")}</Button> : <Button onClick={onChooseRoot}><FolderSimple aria-hidden="true" />{t("onboardingChooseRoot")}</Button>}
                {scanOutcome === "empty" || scanOutcome === "cancelled" || scanOutcome === "failed" ? (
                  <>
                    <Button onClick={onChooseRoot}>{t("onboardingChooseAnotherRoot")}</Button>
                    <Button onClick={onFinish}>{t("onboardingFinishAnyway")}</Button>
                  </>
                ) : (
                  <Button variant="primary" disabled={scanning || !selectedRoot} onClick={onStartScan}>
                    <Path aria-hidden="true" />{t("onboardingStartScan")}
                  </Button>
                )}
              </>
            ) : null}
            {step === "waiting" ? (
              <>
                <Button disabled={Boolean(foundProject)} onClick={() => onStep("mcp")}>{t("onboardingBack")}</Button>
                {foundProject ? (
                  <Button variant="primary" onClick={onFinish}><CheckCircle aria-hidden="true" />{t("onboardingEnterLibrary")}</Button>
                ) : (
                  <>
                    <Button onClick={onSkip}>{t("onboardingSkip")}</Button>
                    <Button variant="primary" onClick={onCheckNow}><TerminalWindow aria-hidden="true" />{t("onboardingCheckNow")}</Button>
                  </>
                )}
              </>
            ) : null}
          </div>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
