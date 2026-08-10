import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  base: "/admin/",
  plugins: [react()],
  build: {
    chunkSizeWarningLimit: 600,
  },
  server: {
    proxy: {
      "/api": "http://127.0.0.1:8080",
    },
  },
  test: {
    server: {
      deps: {
        inline: [/^antd/, /^@ant-design/],
      },
    },
  },
});
