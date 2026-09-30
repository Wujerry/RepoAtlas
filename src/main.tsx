import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/geist";
import "@fontsource-variable/geist/wght-italic.css";
import "./styles.css";

// Keep the global search window independent of the desktop workbench's initialization.
const App = React.lazy(() => new URLSearchParams(window.location.search).has("quick-search") ? import("./QuickSearchApp") : import("./App"));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <React.Suspense fallback={null}><App /></React.Suspense>
  </React.StrictMode>,
);
