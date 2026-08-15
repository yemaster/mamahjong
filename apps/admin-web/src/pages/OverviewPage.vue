<script setup lang="ts">
import { computed } from "vue";
import Card from "primevue/card";
import Column from "primevue/column";
import DataTable from "primevue/datatable";
import Tag from "primevue/tag";
import { adminApi } from "../api";
import PageShell from "../components/PageShell.vue";
import { useResource } from "../composables/useResource";
import { actionLabel, formatDateTime } from "../format";

const resource = useResource(adminApi.overview);
const statistics = computed(() => [
  { label: "用户", value: resource.data.value?.user_count ?? 0, suffix: "人" },
  { label: "等待房间", value: resource.data.value?.waiting_room_count ?? 0, suffix: "间" },
  { label: "进行房间", value: resource.data.value?.playing_room_count ?? 0, suffix: "间" },
  { label: "对局", value: resource.data.value?.match_count ?? 0, suffix: "局" },
  { label: "角色", value: resource.data.value?.character_count ?? 0, suffix: "" },
  { label: "桌布", value: resource.data.value?.tablecloth_count ?? 0, suffix: "" },
  { label: "音乐", value: resource.data.value?.music_count ?? 0, suffix: "" },
]);
</script>

<template>
  <PageShell title="运营概览" :error="resource.error.value" :loading="resource.loading.value">
    <div class="grid">
      <div v-for="item in statistics" :key="item.label" class="col-12 sm:col-6 lg:col-3">
        <Card class="h-full overview-stat-card">
          <template #content>
            <div class="text-color-secondary mb-2">{{ item.label }}</div>
            <div class="text-3xl font-semibold">{{ item.value }}<span class="text-base font-normal ml-1">{{ item.suffix }}</span></div>
          </template>
        </Card>
      </div>
    </div>

    <Card>
      <template #title>最近审计</template>
      <template #content>
        <DataTable :value="resource.data.value?.recent_audit ?? []" scrollable table-style="min-width: 45rem">
          <Column field="occurred_at" header="时间" style="width: 12rem"><template #body="{ data }">{{ formatDateTime(data.occurred_at) }}</template></Column>
          <Column field="action" header="操作" style="width: 11rem"><template #body="{ data }">{{ actionLabel(data.action) }}</template></Column>
          <Column field="detail" header="说明" />
          <Column field="outcome" header="结果" style="width: 6rem"><template #body="{ data }"><Tag :severity="data.outcome === 'success' ? 'success' : 'danger'" :value="data.outcome === 'success' ? '成功' : '失败'" /></template></Column>
          <template #empty>暂无审计记录</template>
        </DataTable>
      </template>
    </Card>
  </PageShell>
</template>
