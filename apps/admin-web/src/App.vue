<script setup lang="ts">
import { onBeforeUnmount, onMounted, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import ConfirmDialog from "primevue/confirmdialog";
import Message from "primevue/message";
import ProgressSpinner from "primevue/progressspinner";
import Toast from "primevue/toast";
import { useAdminSession } from "./session";

const route = useRoute();
const router = useRouter();
const session = useAdminSession();

function synchronizeRoute() {
  if (session.loading.value) return;
  if (session.identity.value && route.name === "login") void router.replace({ name: "overview" });
  if (!session.identity.value && route.name !== "login") void router.replace({ name: "login" });
}

function handleUnauthorized() {
  session.expire();
  synchronizeRoute();
}

watch([session.loading, session.identity, () => route.name], synchronizeRoute);
onMounted(async () => {
  window.addEventListener("mamahjong-admin-unauthorized", handleUnauthorized);
  await session.initialize();
  synchronizeRoute();
});
onBeforeUnmount(() => window.removeEventListener("mamahjong-admin-unauthorized", handleUnauthorized));
</script>

<template>
  <Toast position="top-right" />
  <ConfirmDialog />
  <div v-if="session.loading.value" class="app-loading flex align-items-center justify-content-center min-h-screen">
    <ProgressSpinner aria-label="正在加载" />
  </div>
  <div v-else-if="session.error.value" class="flex align-items-center justify-content-center min-h-screen p-4">
    <Message severity="error" :closable="false">管理端不可用</Message>
  </div>
  <RouterView v-else />
</template>
