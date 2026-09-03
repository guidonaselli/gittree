import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

// Tauri expects a fixed dev server port (see src-tauri/tauri.conf.json
// `devUrl`) and a relative build base so the packaged app can load assets
// from `frontendDist` without an absolute-path assumption.
export default defineConfig({
  plugins: [solid()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "es2021",
    minify: "esbuild",
    sourcemap: true,
  },
});
