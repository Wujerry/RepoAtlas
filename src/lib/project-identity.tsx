import { Code, Cube, FileCSharp, FileCpp, FileJs, FilePy, FileRs, FileTs, FileVue } from "@phosphor-icons/react";
import type { ReactNode } from "react";
import type { ProjectSummary } from "../types";

export function languageFallback(project: Pick<ProjectSummary, "languages" | "frameworks">): string {
  const language = project.languages[0]?.toLowerCase();
  const framework = project.frameworks[0]?.toLowerCase();
  if (framework?.includes("react")) return "react";
  if (framework?.includes("vue")) return "vue";
  if (framework?.includes("flutter")) return "dart";
  return language || "generic";
}

export function LanguageGlyph({ language, className }: { language: string; className?: string }) {
  const key = language.toLowerCase();
  const props = { className, weight: "duotone" as const, "aria-hidden": true };
  if (key.includes("typescript") || key === "ts") return <FileTs {...props} />;
  if (key.includes("javascript") || key === "js" || key === "react") return <FileJs {...props} />;
  if (key.includes("python") || key === "py") return <FilePy {...props} />;
  if (key.includes("rust") || key === "rs") return <FileRs {...props} />;
  if (key.includes("vue")) return <FileVue {...props} />;
  if (key.includes("c#") || key.includes("csharp") || key === "f#") return <FileCSharp {...props} />;
  if (key.includes("c++") || key === "cpp") return <FileCpp {...props} />;
  if (key.includes("go")) return <Cube {...props} />;
  return <Code {...props} />;
}

export function IdeGlyph({ ide }: { ide: string }): ReactNode {
  if (ide === "cursor") {
    return <span className="brand-mark brand-cursor" aria-hidden="true" />;
  }
  if (ide === "vscode") {
    return <span className="brand-mark brand-vscode" aria-hidden="true" />;
  }
  if (ide === "zed") {
    return <span className="brand-mark brand-zed" aria-hidden="true" />;
  }
  if (ide === "jetbrains") {
    return <span className="brand-mark brand-jetbrains" aria-hidden="true" />;
  }
  return <Code aria-hidden="true" />;
}
