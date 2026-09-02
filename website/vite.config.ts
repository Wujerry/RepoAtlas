import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "node:path";

export default defineConfig({
  root: resolve(__dirname),
  base: "/RepoAtlas/",
  plugins: [react(), tailwindcss()],
  build: {
    outDir: resolve(__dirname, "dist"),
    emptyOutDir: true,
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        zh: resolve(__dirname, "zh/index.html"),
      },
    },
  },
});
