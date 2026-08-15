<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import Button from "primevue/button";
import Column from "primevue/column";
import DataTable from "primevue/datatable";
import FileUpload from "primevue/fileupload";
import type { FileUploadUploaderEvent } from "primevue/fileupload";
import IconField from "primevue/iconfield";
import Image from "primevue/image";
import InputIcon from "primevue/inputicon";
import InputText from "primevue/inputtext";
import Tag from "primevue/tag";
import { adminApi } from "../api";
import { characterInput, downloadAssetBundle, isCharacterInput, readAssetBundle, upsertAssetItems } from "../assetTransfer";
import PageShell from "../components/PageShell.vue";
import { useAdminActions } from "../composables/useAdminActions";
import { useResource } from "../composables/useResource";
import { useAdminSession } from "../session";
import type { AdminCharacter } from "../types";
import { completeAdminBatch } from "../batchActions";

const router = useRouter();
const session = useAdminSession();
const actions = useAdminActions();
const resource = useResource(adminApi.characters);
const search = ref("");
const selected = ref<AdminCharacter[]>([]);
const rows = computed(() => {
  const keyword = search.value.trim().toLocaleLowerCase();
  return (resource.data.value?.characters ?? []).filter((item) => !keyword || `${item.name} ${item.id}`.toLocaleLowerCase().includes(keyword));
});
const pageError = computed(() => resource.error.value ?? actions.error.value);

async function remove(ids: string[]) {
  const csrf = session.identity.value?.csrf_token;
  if (!csrf || !ids.length) return;
  const success = await actions.run(() => completeAdminBatch(ids.map((id) => () => adminApi.deleteCharacter(id, csrf))), "角色已删除");
  await resource.reload();
  if (success) selected.value = [];
}

function confirmRemove(event: Event, items: AdminCharacter[]) {
  const ids = items.filter((item) => !item.is_default).map((item) => item.id);
  if (!ids.length) return;
  actions.require(event, ids.length > 1 ? "批量删除角色" : "删除角色", `确定删除 ${ids.length} 个角色？`, () => remove(ids), true);
}

function exportSelected() {
  downloadAssetBundle("characters", selected.value.map(characterInput));
}

async function importCharacters(event: FileUploadUploaderEvent) {
  const file = Array.isArray(event.files) ? event.files[0] : event.files;
  const csrf = session.identity.value?.csrf_token;
  if (!file || !csrf) return;
  const success = await actions.run(async () => {
    const items = await readAssetBundle(file, "characters", isCharacterInput);
    await upsertAssetItems(items, (resource.data.value?.characters ?? []).map((item) => item.id), (item) => adminApi.createCharacter(item, csrf), (item) => adminApi.updateCharacter(item, csrf));
  }, "角色导入完成");
  await resource.reload();
  if (success) selected.value = [];
}
</script>

<template>
  <PageShell title="角色管理" :error="pageError" :loading="resource.loading.value">
    <DataTable v-model:selection="selected" :value="rows" data-key="id" paginator :rows="10" :rows-per-page-options="[10, 20, 50]" scrollable table-style="min-width: 64rem">
        <template #header><div class="management-toolbar"><IconField><InputIcon class="pi pi-search" /><InputText v-model="search" placeholder="搜索角色" /></IconField><div class="management-actions"><Button icon="pi pi-refresh" severity="secondary" variant="text" aria-label="刷新" :loading="resource.loading.value" @click="resource.reload" /><Button label="添加" icon="pi pi-plus" @click="router.push({ name: 'character-new' })" /><FileUpload mode="basic" accept=".json,application/json" :max-file-size="5242880" auto custom-upload choose-label="导入" choose-icon="pi pi-upload" :disabled="actions.pending.value" @uploader="importCharacters" /><Button label="导出" icon="pi pi-download" severity="secondary" variant="text" :disabled="!selected.length" @click="exportSelected" /><Button v-if="selected.length" :label="`删除（${selected.filter((item) => !item.is_default).length}）`" icon="pi pi-trash" severity="danger" variant="text" :disabled="!selected.some((item) => !item.is_default)" @click="confirmRemove($event, selected)" /></div></div></template>
        <Column selection-mode="multiple" header-style="width: 3rem" />
        <Column header="角色" style="width: 14rem"><template #body="{ data }"><div class="flex align-items-center gap-3"><Image :src="data.illustration_path" :alt="data.name" width="40" /><span class="font-medium">{{ data.name }}</span></div></template></Column>
        <Column field="id" header="编号" style="width: 13rem" />
        <Column header="装扮" style="width: 6rem"><template #body="{ data }">{{ data.outfits.length }}</template></Column>
        <Column header="表情" style="width: 6rem"><template #body="{ data }">{{ data.emotes.length }}</template></Column>
        <Column header="语音" style="width: 6rem"><template #body="{ data }">{{ data.voices.length }}</template></Column>
        <Column header="状态" style="width: 10rem"><template #body="{ data }"><div class="flex gap-2"><Tag v-if="data.is_default" severity="success" value="初始" /><Tag severity="secondary" :value="data.enabled ? '启用' : '停用'" /></div></template></Column>
        <Column header="操作" style="width: 12rem"><template #body="{ data }"><div class="flex gap-1"><Button label="编辑" icon="pi pi-pencil" variant="text" size="small" @click="router.push({ name: 'character-edit', params: { characterId: data.id } })" /><Button label="删除" icon="pi pi-trash" severity="danger" variant="text" size="small" :disabled="data.is_default" @click="confirmRemove($event, [data])" /></div></template></Column>
        <template #empty>暂无角色</template>
    </DataTable>
  </PageShell>
</template>
