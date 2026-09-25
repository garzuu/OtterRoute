import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// In sviluppo /api va alla porta admin del gateway (OTR_ADMIN_LISTEN).
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: { "/api": "http://127.0.0.1:9090" },
  },
});
