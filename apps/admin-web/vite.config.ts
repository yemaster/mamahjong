import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  base: "/admin/",
  plugins: [vue()],
  build: {
    chunkSizeWarningLimit: 600,
  },
  server: {
    proxy: {
      "/api": "http://127.0.0.1:8080",
      "/user-assets": "http://127.0.0.1:8080",
    },
  },
});
