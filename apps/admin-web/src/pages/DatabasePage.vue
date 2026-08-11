<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import Avatar from "primevue/avatar";
import Button from "primevue/button";
import Card from "primevue/card";
import Column from "primevue/column";
import DataTable from "primevue/datatable";
import Tag from "primevue/tag";
import { adminApi } from "../api";
import PageShell from "../components/PageShell.vue";
import { useResource } from "../composables/useResource";

const router = useRouter();
const resource = useResource(adminApi.database);
const total = computed(() => (resource.data.value?.tables ?? []).reduce((sum, table) => sum + table.records, 0));
const paths: Record<string, string> = {
  mamahjong_users: "/users", mamahjong_characters: "/characters", mamahjong_tablecloths: "/tablecloths",
  mamahjong_music_tracks: "/music", match_archive: "/matches", audit_log: "/audit",
};
const summaries = computed(() => [
  { label: "存储引擎", value: resource.data.value?.engine ?? "-", icon: "pi pi-database" },
  { label: "连接状态", value: resource.data.value?.status ?? "-", icon: "pi pi-shield" },
  { label: "记录总数", value: total.value, icon: "pi pi-list" },
]);
</script>

<template>
  <PageShell title="数据库" :error="resource.error.value" :loading="resource.loading.value">
    <template #actions><Button icon="pi pi-refresh" severity="secondary" variant="outlined" aria-label="刷新" :loading="resource.loading.value" @click="resource.reload" /></template>
    <div class="grid">
      <div v-for="item in summaries" :key="item.label" class="col-12 md:col-4"><Card class="h-full"><template #content><div class="flex justify-content-between gap-3"><div><div class="text-color-secondary mb-2">{{ item.label }}</div><div class="text-2xl font-semibold">{{ item.value }}</div></div><Avatar :icon="item.icon" size="large" shape="circle" class="bg-primary-50 text-primary" /></div></template></Card></div>
    </div>
    <Card><template #title>连接信息</template><template #content><div class="grid">
      <div class="col-12 md:col-4"><div class="text-color-secondary mb-2">引擎</div><div class="font-medium">{{ resource.data.value?.engine ?? '-' }}</div></div>
      <div class="col-12 md:col-4"><div class="text-color-secondary mb-2">持久化</div><Tag :severity="resource.data.value?.persistent ? 'success' : 'warn'" :value="resource.data.value?.persistent ? '已启用' : '未启用'" /></div>
      <div class="col-12 md:col-4"><div class="text-color-secondary mb-2">连接状态</div><div class="font-medium">{{ resource.data.value?.status ?? '-' }}</div></div>
    </div></template></Card>
    <Card><template #title>数据对象</template><template #content><DataTable :value="resource.data.value?.tables ?? []" data-key="name" sort-field="records" :sort-order="-1">
      <Column field="label" header="数据" />
      <Column field="name" header="存储对象" />
      <Column field="records" header="记录数" sortable style="width: 8rem" />
      <Column field="writable" header="管理方式" style="width: 8rem"><template #body="{ data }"><Tag :severity="data.writable ? 'success' : 'secondary'" :value="data.writable ? '可管理' : '只读'" /></template></Column>
      <Column header="操作" style="width: 7rem"><template #body="{ data }"><Button :label="data.writable ? '管理' : '查看'" variant="text" size="small" @click="router.push(paths[data.name] ?? '/database')" /></template></Column>
      <template #empty>暂无数据对象</template>
    </DataTable></template></Card>
  </PageShell>
</template>
