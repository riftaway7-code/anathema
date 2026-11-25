import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// preload the one font the first screen needs (geist, latin subset). crossorigin is required
// for font preloads even on the same origin.
const preloadFont = {
  name: "preload-geist-latin",
  apply: "build",
  transformIndexHtml: {
    order: "post",
    handler(html, ctx) {
      const file = Object.keys(ctx.bundle ?? {}).find((k) => /geist-latin-wght-normal-[\w-]+\.woff2$/.test(k));
      if (!file) return html;
      return [{ tag: "link", injectTo: "head-prepend", attrs: { rel: "preload", href: `/${file}`, as: "font", type: "font/woff2", crossorigin: "" } }];
    },
  },
};

export default defineConfig({
  plugins: [react(), preloadFont],
  server: { port: 5174 },
});
