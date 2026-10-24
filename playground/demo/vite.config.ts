// The demo page. It uses the component of dist/, as an app that installed
// the package would, so `npm run build` makes dist/ first.

import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const dist = (path: string) => fileURLToPath(new URL(`../dist/${path}`, import.meta.url));

export default defineConfig({
  plugins: [react()],
  // Relative URLs, so that the page works wherever it's served, like
  // under the path of the repository on GitHub Pages.
  base: "./",
  resolve: {
    alias: [
      { find: /^@boltlang\/playground$/, replacement: dist("index.js") },
      { find: /^@boltlang\/playground\/style\.css$/, replacement: dist("playground.css") },
    ],
  },
  build: { outDir: "../out", emptyOutDir: true },
});
