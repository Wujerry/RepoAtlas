import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**", "**/crates/**"],
    },
  },
  worker: {
    format: "es",
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes("node_modules")) return undefined;
          if (id.includes("@phosphor-icons")) return "icons";
          if (id.includes("@base-ui") || id.includes("@floating-ui") || id.includes("@radix-ui")) return "ui";
          if (id.includes("@tauri-apps")) return "tauri";
          if (id.includes("framer-motion")) return "motion";
          if (id.includes("react-markdown") || id.includes("remark-gfm")) return "markdown";
          if (id.includes("@tanstack/react-virtual")) return "virtual";
          return undefined;
        },
      },
    },
  },
}));
