import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { LandingPage } from "./LandingPage";
import "./site.css";

const locale = document.documentElement.lang.startsWith("zh") ? "zh" : "en";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <LandingPage locale={locale} />
  </StrictMode>,
);
