<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
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
import type { AdminMatchSummary } from "../types";

const router = useRouter();
const resource = useResource(adminApi.matches);
const search = ref("");
const family = ref<string>();
const selected = ref<AdminMatchSummary[]>([]);
const dateFormatter = new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" });
const families = computed(() => Array.from(new Set((resource.data.value?.matches ?? []).map((item) => item.rule_family).filter((value): value is string => Boolean(value)))).map((value) => ({ label: value, value })));
const rows = computed(() => {
  const keyword = search.value.trim().toLocaleLowerCase();
  return (resource.data.value?.matches ?? []).filter((item) =>
    (!family.value || item.rule_family === family.value) &&
    (!keyword || item.match_id.toLocaleLowerCase().includes(keyword) || item.seats.some((seat) => seat.nickname.toLocaleLowerCase().includes(keyword))),
  );
});
</script>

<template>
  <PageShell title="对局记录" :error="resource.error.value" :loading="resource.loading.value">
    <DataTable v-model:selection="selected" :value="rows" data-key="match_id" paginator :rows="20" :rows-per-page-options="[10, 20, 50]" sort-field="finished_at_ms" :sort-order="-1" scrollable table-style="min-width: 72rem" current-page-report-template="共 {totalRecords} 局">
      <template #header>
        <div class="management-toolbar">
          <div class="management-filters">
            <IconField><InputIcon class="pi pi-search" /><InputText v-model="search" placeholder="搜索对局" /></IconField>
            <Select v-model="family" :options="families" option-label="label" option-value="value" show-clear placeholder="全部规则" />
            <Tag v-if="selected.length" severity="info" :value="`已选择 ${selected.length} 项`" />
          </div>
          <div class="management-actions"><Button icon="pi pi-refresh" severity="secondary" variant="text" aria-label="刷新" :loading="resource.loading.value" @click="resource.reload" /></div>
        </div>
      </template>
      <Column selection-mode="multiple" header-style="width: 3rem" />
          <Column field="finished_at_ms" header="结束时间" sortable style="width: 12rem"><template #body="{ data }">{{ dateFormatter.format(data.finished_at_ms) }}</template></Column>
          <Column field="match_id" header="对局编号" style="width: 15rem" />
          <Column header="类型" style="width: 7rem"><template #body="{ data }"><Tag severity="secondary" :value="data.friend_match ? '好友房' : '匹配'" /></template></Column>
          <Column header="规则" style="width: 13rem"><template #body="{ data }">{{ data.rule_name ?? ([data.rule_family, data.variant, data.match_length].filter(Boolean).join(' · ') || '未知') }}</template></Column>
          <Column field="hand_count" header="局数" style="width: 6rem" />
          <Column header="玩家"><template #body="{ data }">{{ data.seats.map((seat: { rank: number; nickname: string }) => `${seat.rank}.${seat.nickname}`).join('　') }}</template></Column>
          <Column header="操作" style="width: 7rem"><template #body="{ data }"><Button label="详情" icon="pi pi-eye" variant="text" size="small" @click="router.push({ name: 'match-detail', params: { matchId: data.match_id } })" /></template></Column>
          <template #empty>暂无对局</template>
    </DataTable>
  </PageShell>
</template>
