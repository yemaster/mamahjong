import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import React, { lazy, useState } from "react";
import ReactDOM from "react-dom/client";
import { SplashScreen } from "./components/SplashScreen";
import "./styles/global.css";

const App = lazy(() => import("./App").then((m) => ({ default: m.App })));

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, staleTime: 15_000 },
  },
});

function Root() {
  const [showApp, setShowApp] = useState(false);

  if (!showApp) {
    return <SplashScreen onEnter={() => setShowApp(true)} />;
  }

  return (
    <QueryClientProvider client={queryClient}>
      <React.Suspense
        fallback={
          <div
            style={{
              height: "100vh",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              background: "var(--color-bg)",
              color: "var(--color-text-dim)",
              fontFamily: "var(--font-game)",
            }}
          >
            加载中…
          </div>
        }
      >
        <App />
      </React.Suspense>
    </QueryClientProvider>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
