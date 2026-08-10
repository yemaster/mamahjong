import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { App as AntApp, ConfigProvider } from "antd";
import zhCN from "antd/locale/zh_CN";
import "antd/dist/reset.css";
import { AdminApp } from "./AdminApp";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      staleTime: 15_000,
    },
  },
});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ConfigProvider
      locale={zhCN}
      theme={{
        token: {
          colorPrimary: "#167c5a",
          borderRadius: 8,
          sizeUnit: 4,
          sizeStep: 4,
        },
        components: {
          Layout: {
            bodyBg: "#f3f5f7",
            headerBg: "#ffffff",
          },
          Card: {
            paddingLG: 24,
          },
          Table: {
            cellPaddingBlock: 14,
            cellPaddingInline: 16,
          },
        },
      }}
    >
      <AntApp>
        <QueryClientProvider client={queryClient}>
          <AdminApp />
        </QueryClientProvider>
      </AntApp>
    </ConfigProvider>
  </React.StrictMode>,
);
