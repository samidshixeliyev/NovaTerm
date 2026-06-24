import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// NovaTerm UI. When launched under Tauri, the app is served from the bundled
// dist; in the browser it runs against the in-process mock backend.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: "esnext",
    outDir: "dist",
  },
});
