<script setup lang="ts">
import { computed, ref } from "vue";
import Button from "primevue/button";
import Column from "primevue/column";
import DataTable from "primevue/datatable";
import IconField from "primevue/iconfield";
import InputIcon from "primevue/inputicon";
import InputText from "primevue/inputtext";
import Select from "primevue/select";
import Tag from "primevue/tag";
import { adminApi } from "../api";
import PageShell from "../components/PageShell.vue";
import { useResource } from "../composables/useResource";
import { actionLabel, categoryLabel, formatDateTime } from "../format";

const resource = useResource(adminApi.audit);
const search = ref("");
const category = ref<string>();
const categories = [
  { label: "认证", value: "auth" }, { label: "房间", value: "room" },
  { label: "匹配", value: "matchmaking" }, { label: "对局", value: "game" },
  { label: "管理", value: "admin" },
];
const events = computed(() => {
  const keyword = search.value.trim().toLocaleLowerCase();
  return (resource.data.value?.events ?? []).filter((event) =>
    (!category.value || event.category === category.value) &&
    (!keyword || `${event.action} ${actionLabel(event.action)} ${event.detail} ${event.target_id ?? ""}`.toLocaleLowerCase().includes(keyword)),
  );
});
</script>

<template>
  <PageShell title="审计日志" :error="resource.error.value" :loading="resource.loading.value">
    <DataTable :value="events" data-key="sequence" paginator :rows="20" :rows-per-page-options="[10, 20, 50]" sort-field="sequence" :sort-order="-1" scrollable table-style="min-width: 68rem" current-page-report-template="共 {totalRecords} 项">
      <template #header>
        <div class="management-toolbar">
          <div class="management-filters">
            <Select v-model="category" :options="categories" option-label="label" option-value="value" show-clear placeholder="全部类别" />
            <IconField><InputIcon class="pi pi-search" /><InputText v-model="search" placeholder="搜索日志" /></IconField>
          </div>
          <div class="management-actions"><Button icon="pi pi-refresh" severity="secondary" variant="text" aria-label="刷新" :loading="resource.loading.value" @click="resource.reload" /></div>
        </div>
      </template>
      <Column field="sequence" header="序号" sortable style="width: 6rem" />
      <Column field="occurred_at" header="时间" style="width: 12rem"><template #body="{ data }">{{ formatDateTime(data.occurred_at) }}</template></Column>
      <Column field="category" header="类别" style="width: 7rem"><template #body="{ data }">{{ categoryLabel(data.category) }}</template></Column>
      <Column field="action" header="操作" style="width: 11rem"><template #body="{ data }">{{ actionLabel(data.action) }}</template></Column>
      <Column field="target_id" header="目标" style="width: 15rem" />
      <Column field="detail" header="说明" />
      <Column field="outcome" header="结果" style="width: 6rem"><template #body="{ data }"><Tag :severity="data.outcome === 'success' ? 'success' : 'danger'" :value="data.outcome === 'success' ? '成功' : '失败'" /></template></Column>
      <template #empty>暂无审计记录</template>
    </DataTable>
  </PageShell>
</template>
