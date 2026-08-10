import { useSyncExternalStore } from "react";

export type AdminRoute =
  | "/"
  | "/login"
  | "/users"
  | "/rooms"
  | "/characters"
  | "/tablecloths"
  | "/audit";

function currentRoute(): AdminRoute {
  const value = window.location.pathname.replace(/^\/admin/, "") || "/";
  return ["/", "/login", "/users", "/rooms", "/characters", "/tablecloths", "/audit"].includes(value)
    ? (value as AdminRoute)
    : "/";
}

function subscribe(callback: () => void) {
  window.addEventListener("popstate", callback);
  return () => window.removeEventListener("popstate", callback);
}

export function useAdminRoute(): AdminRoute {
  return useSyncExternalStore(subscribe, currentRoute, (): AdminRoute => "/");
}

export function navigate(route: AdminRoute) {
  window.history.pushState(null, "", `/admin${route === "/" ? "/" : route}`);
  window.dispatchEvent(new PopStateEvent("popstate"));
}
