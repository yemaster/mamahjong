<script setup lang="ts">
import { computed, ref } from "vue";
import Button from "primevue/button";
import Column from "primevue/column";
import DataTable from "primevue/datatable";
import Select from "primevue/select";
import Tag from "primevue/tag";
import { adminApi } from "../api";
import PageShell from "../components/PageShell.vue";
import { useAdminActions } from "../composables/useAdminActions";
import { useResource } from "../composables/useResource";
import { useAdminSession } from "../session";
import type { AdminRoom, RoomLifecycle } from "../types";
import { completeAdminBatch } from "../batchActions";

const session = useAdminSession();
const actions = useAdminActions();
const resource = useResource(adminApi.rooms);
const lifecycle = ref<RoomLifecycle>();
const selected = ref<AdminRoom[]>([]);
const lifecycleOptions = [
  { label: "等待中", value: "waiting" }, { label: "进行中", value: "playing" }, { label: "已关闭", value: "closed" },
];
const lifecycleText: Record<RoomLifecycle, string> = { waiting: "等待中", playing: "进行中", closed: "已关闭" };
const rooms = computed(() => (resource.data.value?.rooms ?? []).filter((room) => !lifecycle.value || room.lifecycle === lifecycle.value));
const closableSelected = computed(() => selected.value.filter((room) => room.lifecycle === "waiting"));
const pageError = computed(() => resource.error.value ?? actions.error.value);

async function closeRooms(ids: string[]) {
  const csrf = session.identity.value?.csrf_token;
  if (!csrf || !ids.length) return;
  const success = await actions.run(() => completeAdminBatch(ids.map((id) => () => adminApi.closeRoom(id, csrf))), "房间已关闭");
  await resource.reload();
  if (success) selected.value = [];
}

function confirmClose(event: Event, room: AdminRoom) {
  actions.require(event, "关闭房间", room.name, () => closeRooms([room.id]), true);
}

function confirmSelected(event: Event) {
  const ids = closableSelected.value.map((room) => room.id);
  if (!ids.length) return;
  actions.require(event, "批量关闭", `确定关闭 ${ids.length} 个等待中的房间？`, () => closeRooms(ids), true);
}
</script>

<template>
  <PageShell title="房间管理" :error="pageError" :loading="resource.loading.value">
    <DataTable v-model:selection="selected" :value="rooms" data-key="id" paginator :rows="10" :rows-per-page-options="[10, 20, 50]" scrollable table-style="min-width: 48rem">
      <template #header><div class="management-toolbar"><div class="management-filters">
        <Select v-model="lifecycle" :options="lifecycleOptions" option-label="label" option-value="value" show-clear placeholder="全部状态" />
      </div><div class="management-actions"><Button icon="pi pi-refresh" severity="secondary" variant="text" aria-label="刷新" :loading="resource.loading.value" @click="resource.reload" />
        <Button v-if="selected.length" :label="`关闭所选（${closableSelected.length}）`" icon="pi pi-times-circle" severity="danger" variant="outlined" :disabled="!closableSelected.length" @click="confirmSelected" />
      </div></div></template>
        <Column selection-mode="multiple" header-style="width: 3rem" />
        <Column field="name" header="房间" />
        <Column header="人数" style="width: 7rem"><template #body="{ data }">{{ data.member_count }}/{{ data.seat_count }}</template></Column>
        <Column field="visibility" header="可见性" style="width: 7rem"><template #body="{ data }">{{ data.visibility === 'public' ? '公开' : '私有' }}</template></Column>
        <Column field="lifecycle" header="状态" style="width: 8rem"><template #body="{ data }"><Tag :severity="data.lifecycle === 'playing' ? 'info' : 'secondary'" :value="lifecycleText[data.lifecycle as RoomLifecycle]" /></template></Column>
        <Column header="操作" style="width: 7rem"><template #body="{ data }"><Button v-if="data.lifecycle === 'waiting'" label="关闭" severity="danger" variant="text" size="small" :loading="actions.pending.value" @click="confirmClose($event, data)" /></template></Column>
        <template #empty>暂无房间</template>
    </DataTable>
  </PageShell>
</template>
