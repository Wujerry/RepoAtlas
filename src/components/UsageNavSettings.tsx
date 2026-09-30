import { useRef, useState } from "react";
import { providers, type UsageT } from "../lib/subscription-presentation";
import { Switch } from "./ui/switch";

export function useUsageNavigation(hidden: string[], onChange: (hidden: string[]) => Promise<void>) {
  const [state, setState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const flight = useRef(false);
  async function setVisible(provider: string, show: boolean) {
    if (flight.current) return;
    flight.current = true;
    setState("saving");
    try { await onChange(show ? hidden.filter(id => id !== provider) : [...hidden, provider]); setState("saved"); }
    catch { setState("error"); }
    finally { flight.current = false; }
  }
  return { hidden, status: state, setVisible };
}
export type UsageNavigation = ReturnType<typeof useUsageNavigation>;

export function UsageNavSwitch({ provider, navigation, t }: { provider: string; navigation: UsageNavigation; t: UsageT }) {
  return <div className="usage-nav-inline"><span>{t("usageNavShow")}</span>
    <Switch label={`${t("usageNavShow")} · ${providers[provider]?.name ?? provider}`}
      checked={!navigation.hidden.includes(provider)} disabled={navigation.status === "saving"}
      onCheckedChange={show => void navigation.setVisible(provider, show)} />
  </div>;
}

export function UsageNavFeedback({ navigation, t }: { navigation: UsageNavigation; t: UsageT }) {
  return <span className="usage-nav-feedback" role={navigation.status === "error" ? "alert" : "status"}>
    {navigation.status === "saving" ? t("usageNavSaving") : navigation.status === "saved" ? t("settingsSaved") : navigation.status === "error" ? t("settingsSaveFailed") : t("usageNavSettingsScope")}
  </span>;
}
