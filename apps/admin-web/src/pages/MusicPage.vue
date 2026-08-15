<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import Button from "primevue/button";
import Column from "primevue/column";
import DataTable from "primevue/datatable";
import FileUpload from "primevue/fileupload";
import type { FileUploadUploaderEvent } from "primevue/fileupload";
import IconField from "primevue/iconfield";
import InputIcon from "primevue/inputicon";
import InputText from "primevue/inputtext";
import Select from "primevue/select";
import Tag from "primevue/tag";
import { adminApi } from "../api";
import { downloadAssetBundle, isMusicInput, musicInput, readAssetBundle, upsertAssetItems } from "../assetTransfer";
import PageShell from "../components/PageShell.vue";
import { useAdminActions } from "../composables/useAdminActions";
import { useResource } from "../composables/useResource";
import { useAdminSession } from "../session";
import type { AdminMusic, MusicScene } from "../types";
import { completeAdminBatch } from "../batchActions";

const router = useRouter();
const session = useAdminSession();
const actions = useAdminActions();
const resource = useResource(adminApi.music);
const search = ref("");
const scene = ref<MusicScene>();
const selected = ref<AdminMusic[]>([]);
const sceneLabel: Record<MusicScene, string> = { lobby: "大厅", match: "对局", riichi: "立直" };
const sceneOptions = Object.entries(sceneLabel).map(([value, label]) => ({ value, label }));
const rows = computed(() => { const keyword = search.value.trim().toLocaleLowerCase(); return (resource.data.value?.music_tracks ?? []).filter((item) => (!scene.value || item.scene === scene.value) && (!keyword || `${item.name} ${item.id}`.toLocaleLowerCase().includes(keyword))); });
const pageError = computed(() => resource.error.value ?? actions.error.value);

function duration(value: number) { return `${Math.floor(value / 60000)}:${String(Math.floor(value / 1000) % 60).padStart(2, "0")}`; }
async function remove(ids: string[]) {
  const csrf = session.identity.value?.csrf_token;
  if (!csrf || !ids.length) return;
  const success = await actions.run(() => completeAdminBatch(ids.map((id) => () => adminApi.deleteMusic(id, csrf))), "音乐已删除");
  await resource.reload();
  if (success) selected.value = [];
}
function confirmRemove(event: Event, items: AdminMusic[]) {
  const ids = items.filter((item) => !item.is_default).map((item) => item.id);
  if (!ids.length) return;
  actions.require(event, ids.length > 1 ? "批量删除音乐" : "删除音乐", `确定删除 ${ids.length} 首音乐？`, () => remove(ids), true);
}

function exportSelected() {
  downloadAssetBundle("music", selected.value.map(musicInput));
}

async function importMusic(event: FileUploadUploaderEvent) {
  const file = Array.isArray(event.files) ? event.files[0] : event.files;
  const csrf = session.identity.value?.csrf_token;
  if (!file || !csrf) return;
  const success = await actions.run(async () => {
    const items = await readAssetBundle(file, "music", isMusicInput);
    await upsertAssetItems(items, (resource.data.value?.music_tracks ?? []).map((item) => item.id), (item) => adminApi.createMusic(item, csrf), (item) => adminApi.updateMusic(item, csrf));
  }, "音乐导入完成");
  await resource.reload();
  if (success) selected.value = [];
}
</script>

<template>
  <PageShell title="音乐管理" :error="pageError" :loading="resource.loading.value">
    <DataTable v-model:selection="selected" :value="rows" data-key="id" paginator :rows="10" :rows-per-page-options="[10, 20, 50]" scrollable table-style="min-width: 66rem">
        <template #header><div class="management-toolbar"><div class="management-filters"><IconField><InputIcon class="pi pi-search" /><InputText v-model="search" placeholder="搜索音乐" /></IconField><Select v-model="scene" :options="sceneOptions" option-label="label" option-value="value" show-clear placeholder="全部场景" /></div><div class="management-actions"><Button icon="pi pi-refresh" severity="secondary" variant="text" aria-label="刷新" :loading="resource.loading.value" @click="resource.reload" /><Button label="添加" icon="pi pi-plus" @click="router.push({ name: 'music-new' })" /><FileUpload mode="basic" accept=".json,application/json" :max-file-size="5242880" auto custom-upload choose-label="导入" choose-icon="pi pi-upload" :disabled="actions.pending.value" @uploader="importMusic" /><Button label="导出" icon="pi pi-download" severity="secondary" variant="text" :disabled="!selected.length" @click="exportSelected" /><Button v-if="selected.length" :label="`删除（${selected.filter((item) => !item.is_default).length}）`" icon="pi pi-trash" severity="danger" variant="text" :disabled="!selected.some((item) => !item.is_default)" @click="confirmRemove($event, selected)" /></div></div></template>
        <Column selection-mode="multiple" header-style="width: 3rem" />
        <Column field="name" header="名称" style="width: 12rem" />
        <Column field="id" header="编号" style="width: 13rem" />
        <Column field="scene" header="场景" style="width: 7rem"><template #body="{ data }">{{ sceneLabel[data.scene as MusicScene] }}</template></Column>
        <Column field="duration_ms" header="时长" style="width: 7rem"><template #body="{ data }">{{ duration(data.duration_ms) }}</template></Column>
        <Column field="audio_path" header="音频路径" />
        <Column header="状态" style="width: 10rem"><template #body="{ data }"><div class="flex gap-2"><Tag v-if="data.is_default" severity="success" value="默认" /><Tag severity="secondary" :value="data.enabled ? '启用' : '停用'" /></div></template></Column>
        <Column header="操作" style="width: 12rem"><template #body="{ data }"><div class="flex gap-1"><Button label="编辑" icon="pi pi-pencil" variant="text" size="small" @click="router.push({ name: 'music-edit', params: { musicId: data.id } })" /><Button label="删除" icon="pi pi-trash" severity="danger" variant="text" size="small" :disabled="data.is_default" @click="confirmRemove($event, [data])" /></div></template></Column>
        <template #empty>暂无音乐</template>
    </DataTable>
  </PageShell>
</template>
