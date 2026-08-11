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
import { downloadAssetBundle, isTableclothInput, readAssetBundle, tableclothInput, upsertAssetItems } from "../assetTransfer";
import PageShell from "../components/PageShell.vue";
import { useAdminActions } from "../composables/useAdminActions";
import { useResource } from "../composables/useResource";
import { useAdminSession } from "../session";
import type { AdminTablecloth } from "../types";

const router = useRouter();
const session = useAdminSession();
const actions = useAdminActions();
const resource = useResource(adminApi.tablecloths);
const search = ref("");
const selected = ref<AdminTablecloth[]>([]);
const rows = computed(() => { const keyword = search.value.trim().toLocaleLowerCase(); return (resource.data.value?.tablecloths ?? []).filter((item) => !keyword || `${item.name} ${item.id}`.toLocaleLowerCase().includes(keyword)); });
const pageError = computed(() => resource.error.value ?? actions.error.value);

async function remove(ids: string[]) {
  const csrf = session.identity.value?.csrf_token;
  if (!csrf || !ids.length) return;
  const success = await actions.run(() => Promise.all(ids.map((id) => adminApi.deleteTablecloth(id, csrf))), "桌布已删除");
  if (success) { selected.value = []; await resource.reload(); }
}

function confirmRemove(event: Event, items: AdminTablecloth[]) {
  const ids = items.filter((item) => !item.is_default).map((item) => item.id);
  actions.require(event, ids.length > 1 ? "批量删除桌布" : "删除桌布", `确定删除 ${ids.length} 个桌布？`, () => remove(ids), true);
}

function exportSelected() {
  downloadAssetBundle("tablecloths", selected.value.map(tableclothInput));
}

async function importTablecloths(event: FileUploadUploaderEvent) {
  const file = Array.isArray(event.files) ? event.files[0] : event.files;
  const csrf = session.identity.value?.csrf_token;
  if (!file || !csrf) return;
  const success = await actions.run(async () => {
    const items = await readAssetBundle(file, "tablecloths", isTableclothInput);
    await upsertAssetItems(items, (resource.data.value?.tablecloths ?? []).map((item) => item.id), (item) => adminApi.createTablecloth(item, csrf), (item) => adminApi.updateTablecloth(item, csrf));
  }, "桌布导入完成");
  if (success) {
    selected.value = [];
    await resource.reload();
  }
}
</script>

<template>
  <PageShell title="桌布" :error="pageError" :loading="resource.loading.value">
    <template #actions><Button icon="pi pi-refresh" severity="secondary" variant="outlined" aria-label="刷新" :loading="resource.loading.value" @click="resource.reload" /><Button label="添加桌布" icon="pi pi-plus" @click="router.push({ name: 'tablecloth-new' })" /></template>
    <DataTable v-model:selection="selected" :value="rows" data-key="id" paginator :rows="10" :rows-per-page-options="[10, 20, 50]" scrollable table-style="min-width: 58rem">
        <template #header><div class="flex align-items-center justify-content-between gap-3 flex-wrap"><span>全部桌布（{{ rows.length }}）</span><div class="flex gap-2 flex-wrap"><IconField><InputIcon class="pi pi-search" /><InputText v-model="search" placeholder="搜索名称或编号" /></IconField><FileUpload mode="basic" accept=".json,application/json" :max-file-size="5242880" auto custom-upload choose-label="导入" choose-icon="pi pi-upload" :disabled="actions.pending.value" @uploader="importTablecloths" /><Button label="导出所选" icon="pi pi-download" severity="secondary" variant="outlined" :disabled="!selected.length" @click="exportSelected" /><Button v-if="selected.length" :label="`删除所选（${selected.filter((item) => !item.is_default).length}）`" icon="pi pi-trash" severity="danger" variant="outlined" @click="confirmRemove($event, selected)" /></div></div></template>
        <Column selection-mode="multiple" header-style="width: 3rem" />
        <Column header="桌布" style="width: 14rem"><template #body="{ data }"><div class="flex align-items-center gap-3"><Image :src="data.texture_path" :alt="data.name" width="54" /><span class="font-medium">{{ data.name }}</span></div></template></Column>
        <Column field="id" header="编号" style="width: 13rem" />
        <Column field="texture_path" header="纹理路径" />
        <Column header="状态" style="width: 10rem"><template #body="{ data }"><div class="flex gap-2"><Tag v-if="data.is_default" severity="success" value="初始" /><Tag severity="secondary" :value="data.enabled ? '启用' : '停用'" /></div></template></Column>
        <Column header="操作" style="width: 12rem"><template #body="{ data }"><div class="flex gap-1"><Button label="编辑" icon="pi pi-pencil" variant="text" size="small" @click="router.push({ name: 'tablecloth-edit', params: { tableclothId: data.id } })" /><Button label="删除" icon="pi pi-trash" severity="danger" variant="text" size="small" :disabled="data.is_default" @click="confirmRemove($event, [data])" /></div></template></Column>
        <template #empty>暂无桌布</template>
    </DataTable>
  </PageShell>
</template>
