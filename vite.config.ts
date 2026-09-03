import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "node:path";

// Clide ships two windows: the dashboard and the recording HUD.
// They are separate documents so the HUD stays tiny and cheap to show.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": resolve(__dirname, "src") },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "safari15",
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        hud: resolve(__dirname, "hud.html"),
      },
    },
  },
});
