<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useToast } from "primevue/usetoast";
import Button from "primevue/button";
import Drawer from "primevue/drawer";
import Skeleton from "primevue/skeleton";
import Toolbar from "primevue/toolbar";
import type { MenuItem } from "primevue/menuitem";
import { useAdminSession } from "../session";
import AdminNavigation from "./AdminNavigation.vue";

const router = useRouter();
const toast = useToast();
const session = useAdminSession();
const mobileNavigationVisible = ref(false);
const isMobile = ref(false);
const loggingOut = ref(false);
let mediaQuery: MediaQueryList | undefined;

const navigation: MenuItem[] = [
  {
    key: "workspace",
    label: "工作台",
    icon: "pi pi-chart-bar",
    items: [{ label: "总览", icon: "pi pi-chart-bar", route: "/" }],
  },
  {
    key: "operations",
    label: "运营管理",
    icon: "pi pi-briefcase",
    items: [
      { label: "对局", icon: "pi pi-history", route: "/matches" },
      { label: "房间", icon: "pi pi-home", route: "/rooms" },
      { label: "用户", icon: "pi pi-users", route: "/users" },
    ],
  },
  {
    key: "assets",
    label: "素材管理",
    icon: "pi pi-folder",
    items: [
      { label: "资源库", icon: "pi pi-folder-open", route: "/assets" },
      { label: "角色", icon: "pi pi-id-card", route: "/characters" },
      { label: "桌布", icon: "pi pi-image", route: "/tablecloths" },
      { label: "音乐", icon: "pi pi-headphones", route: "/music" },
    ],
  },
  {
    key: "system",
    label: "系统管理",
    icon: "pi pi-cog",
    items: [
      { label: "数据库", icon: "pi pi-database", route: "/database" },
      { label: "审计日志", icon: "pi pi-shield", route: "/audit" },
    ],
  },
];

function updateViewport(event: MediaQueryList | MediaQueryListEvent) {
  isMobile.value = event.matches;
  if (!event.matches) mobileNavigationVisible.value = false;
}

async function logout() {
  loggingOut.value = true;
  try {
    await session.signOut();
    await router.replace({ name: "login" });
  } catch (error) {
    toast.add({ severity: "error", summary: "退出失败", detail: error instanceof Error ? error.message : "请求失败", life: 3000 });
  } finally {
    loggingOut.value = false;
  }
}

onMounted(() => {
  mediaQuery = window.matchMedia("(max-width: 1023px)");
  updateViewport(mediaQuery);
  mediaQuery.addEventListener("change", updateViewport);
});

onBeforeUnmount(() => {
  mediaQuery?.removeEventListener("change", updateViewport);
});
</script>

<template>
  <div class="admin-shell">
    <aside class="admin-sidebar" aria-label="后台导航">
      <AdminNavigation :model="navigation" />

      <div class="admin-sidebar-footer">
        <span class="admin-user-avatar"><i class="pi pi-user" aria-hidden="true" /></span>
        <div class="min-w-0">
          <div class="font-medium overflow-hidden text-overflow-ellipsis white-space-nowrap">{{ session.identity.value?.nickname }}</div>
          <div class="text-xs text-color-secondary mt-1">管理员</div>
        </div>
        <Button icon="pi pi-sign-out" severity="secondary" variant="text" aria-label="退出登录" :loading="loggingOut" @click="logout" />
      </div>
    </aside>

    <div class="admin-workspace">
      <Toolbar v-if="isMobile" class="admin-mobile-bar border-noround border-x-none border-top-none">
        <template #start>
          <div class="flex align-items-center gap-2">
            <Button icon="pi pi-bars" severity="secondary" variant="text" aria-label="打开菜单" aria-controls="admin-navigation" :aria-expanded="mobileNavigationVisible" @click="mobileNavigationVisible = true" />
            <strong>管理后台</strong>
          </div>
        </template>
        <template #end>
          <Button icon="pi pi-sign-out" severity="secondary" variant="text" aria-label="退出登录" :loading="loggingOut" @click="logout" />
        </template>
      </Toolbar>

      <main class="admin-content">
        <RouterView v-slot="{ Component }">
          <Suspense>
            <component :is="Component" />
            <template #fallback>
              <div class="route-loading page-content" aria-label="正在加载">
                <div class="page-loading-panel">
                  <Skeleton width="12rem" height="2.25rem" />
                  <Skeleton width="65%" height="1rem" />
                  <Skeleton width="100%" height="18rem" border-radius="0.75rem" />
                </div>
              </div>
            </template>
          </Suspense>
        </RouterView>
      </main>
    </div>
  </div>

  <Drawer id="admin-navigation" v-model:visible="mobileNavigationVisible" header="管理后台" block-scroll class="w-18rem">
    <AdminNavigation :model="navigation" @navigate="mobileNavigationVisible = false" />
  </Drawer>
</template>
