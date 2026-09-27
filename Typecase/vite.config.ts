import path from "path";
import { fileURLToPath } from "url";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// https://vite.dev/config/
export default defineConfig({
  // Tauri expects a fixed port and fails if it is already in use.
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  // Environment variables prefixed with TAURI_ are exposed to the frontend.
  envPrefix: ["VITE_", "TAURI_ENV_"],
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src"),
    },
  },
  // rustTargetDir holds the debug/release binaries for tauri dev/build.
  clearScreen: false,
  build: {
    // Necessary for tauri on Windows (mobile devtools do not support modulepreload).
    target: "chrome105",
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
  },
});
