import { lazy, Suspense, useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Result, Spin } from "antd";
import { adminApi, ApiError } from "./api";
import { navigate, useAdminRoute } from "./routing";

const AdminLayout = lazy(() =>
  import("./components/AdminLayout").then((module) => ({
    default: module.AdminLayout,
  })),
);
const LoginPage = lazy(() =>
  import("./pages/LoginPage").then((module) => ({
    default: module.LoginPage,
  })),
);
const OverviewPage = lazy(() =>
  import("./pages/OverviewPage").then((module) => ({
    default: module.OverviewPage,
  })),
);
const UsersPage = lazy(() =>
  import("./pages/UsersPage").then((module) => ({
    default: module.UsersPage,
  })),
);
const RoomsPage = lazy(() =>
  import("./pages/RoomsPage").then((module) => ({
    default: module.RoomsPage,
  })),
);
const CharactersPage = lazy(() =>
  import("./pages/CharactersPage").then((module) => ({
    default: module.CharactersPage,
  })),
);
const TableclothsPage = lazy(() =>
  import("./pages/TableclothsPage").then((module) => ({
    default: module.TableclothsPage,
  })),
);
const AuditPage = lazy(() =>
  import("./pages/AuditPage").then((module) => ({
    default: module.AuditPage,
  })),
);

export function AdminApp() {
  const route = useAdminRoute();
  const queryClient = useQueryClient();
  const identity = useQuery({
    queryKey: ["admin", "identity"],
    queryFn: adminApi.identity,
    retry: false,
  });
  const authenticated = identity.isSuccess;
  useEffect(() => {
    if (identity.isLoading) {
      return;
    }
    if (authenticated && route === "/login") {
      navigate("/");
    } else if (!authenticated && route !== "/login") {
      navigate("/login");
    }
  }, [authenticated, identity.isLoading, route]);
  useEffect(() => {
    const refreshIdentity = () => {
      void queryClient.invalidateQueries({
        queryKey: ["admin", "identity"],
      });
    };
    window.addEventListener("mamahjong-admin-unauthorized", refreshIdentity);
    return () =>
      window.removeEventListener(
        "mamahjong-admin-unauthorized",
        refreshIdentity,
      );
  }, [queryClient]);

  if (identity.isLoading) {
    return <Spin fullscreen tip="正在加载" />;
  }
  if (identity.error && !(identity.error instanceof ApiError)) {
    return <Result status="error" title="管理端不可用" />;
  }

  let page = <OverviewPage />;
  if (route === "/users") {
    page = <UsersPage identity={identity.data!} />;
  } else if (route === "/rooms") {
    page = <RoomsPage identity={identity.data!} />;
  } else if (route === "/characters") {
    page = <CharactersPage identity={identity.data!} />;
  } else if (route === "/tablecloths") {
    page = <TableclothsPage identity={identity.data!} />;
  } else if (route === "/audit") {
    page = <AuditPage />;
  }

  return (
    <Suspense fallback={<Spin fullscreen tip="正在加载" />}>
      {authenticated ? (
        <AdminLayout identity={identity.data} route={route} navigate={navigate}>
          {page}
        </AdminLayout>
      ) : (
        <LoginPage identityQuery={identity} />
      )}
    </Suspense>
  );
}
