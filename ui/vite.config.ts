import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri dev server: fixed port, not exposed externally.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: "127.0.0.1",
  },
  build: {
    // WebView2 (Windows) and WKWebView (macOS 11+). Big Sur can be stuck on Safari 14's WebKit
    // (e.g. a 2014 MacBook Air), which rejects newer syntax such as private methods; one
    // unparsable line leaves the window blank.
    target: ["chrome105", "safari14"],
    minify: "esbuild",
    sourcemap: false,
  },
});
