<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import Button from "primevue/button";
import Card from "primevue/card";
import Dialog from "primevue/dialog";
import InputText from "primevue/inputtext";
import ProgressSpinner from "primevue/progressspinner";
import { adminApi } from "../api";
import { assetIcon, formatAssetSize, publicAssetUrl } from "../assetPaths";
import PageShell from "../components/PageShell.vue";
import { useAdminActions } from "../composables/useAdminActions";
import { useFileDrop } from "../composables/useFileDrop";
import { useAdminSession } from "../session";
import type { AssetList, ManagedAsset } from "../types";

const session = useAdminSession();
const actions = useAdminActions();
const data = ref<AssetList>();
const loading = ref(true);
const error = ref<Error>();
const search = ref("");
const folderDialog = ref(false);
const folderName = ref("");
const fileInput = ref<HTMLInputElement>();
const currentPath = computed(() => data.value?.path ?? "");
const entries = computed(() => {
  const keyword = search.value.trim().toLocaleLowerCase();
  return (data.value?.entries ?? []).filter((asset) => !keyword || asset.name.toLocaleLowerCase().includes(keyword));
});
const crumbs = computed(() => {
  const parts = currentPath.value.split("/").filter(Boolean);
  return [{ label: "全部资源", path: "" }, ...parts.map((label, index) => ({ label, path: parts.slice(0, index + 1).join("/") }))];
});
const summary = computed(() => ({
  folders: (data.value?.entries ?? []).filter((item) => item.kind === "folder").length,
  files: (data.value?.entries ?? []).filter((item) => item.kind === "file").length,
  size: (data.value?.entries ?? []).reduce((total, item) => total + item.size, 0),
}));

const fileDrop = useFileDrop(uploadFiles);

async function load(path = currentPath.value) {
  loading.value = true;
  error.value = undefined;
  try {
    data.value = await adminApi.assets(path);
  } catch (cause) {
    error.value = cause instanceof Error ? cause : new Error("资源加载失败");
  } finally {
    loading.value = false;
  }
}

async function uploadFiles(files: File[]) {
  const csrf = session.identity.value?.csrf_token;
  if (!files.length || !csrf) return;
  const success = await actions.run(
    async () => {
      for (const file of files) await adminApi.uploadAsset(currentPath.value, file, csrf);
    },
    files.length > 1 ? `${files.length} 个文件已上传` : "文件已上传",
  );
  await load();
  if (!success) fileDrop.reset();
}

function upload(event: Event) {
  const input = event.target as HTMLInputElement;
  const files = Array.from(input.files ?? []);
  input.value = "";
  void uploadFiles(files);
}

async function createFolder() {
  const name = folderName.value.trim();
  const csrf = session.identity.value?.csrf_token;
  if (!name || !csrf) return;
  const success = await actions.run(() => adminApi.createAssetFolder(currentPath.value, name, csrf), "文件夹已创建");
  if (success) {
    folderDialog.value = false;
    folderName.value = "";
    await load();
  }
}

function confirmDelete(event: Event, asset: ManagedAsset) {
  actions.require(event, `删除${asset.kind === 'folder' ? '文件夹' : '文件'}`, `确定删除“${asset.name}”？此操作不可恢复。`, async () => {
    const csrf = session.identity.value?.csrf_token;
    if (!csrf) return;
    const success = await actions.run(() => adminApi.deleteAsset(asset.path, csrf), "资源已删除");
    if (success) await load();
  }, true);
}

function open(asset: ManagedAsset) {
  if (asset.kind === "folder") void load(asset.path);
  else window.open(publicAssetUrl(asset.path), "_blank", "noopener,noreferrer");
}

onMounted(() => void load(""));
</script>

<template>
  <PageShell title="资源库" :error="error">
    <input ref="fileInput" class="asset-hidden-input" type="file" multiple @change="upload" />

    <div class="asset-summary-grid">
      <Card><template #content><div class="asset-summary-card"><strong>{{ summary.folders }}</strong><span>文件夹</span></div></template></Card>
      <Card><template #content><div class="asset-summary-card"><strong>{{ summary.files }}</strong><span>文件</span></div></template></Card>
      <Card><template #content><div class="asset-summary-card"><strong>{{ formatAssetSize(summary.size) }}</strong><span>大小</span></div></template></Card>
    </div>

    <Card class="asset-manager-card" :class="{ 'asset-drop-active': fileDrop.active.value }" @dragenter.prevent="fileDrop.enter" @dragover.prevent="fileDrop.over" @dragleave.prevent="fileDrop.leave" @drop.prevent="fileDrop.drop">
      <template #content>
        <div v-if="fileDrop.active.value" class="asset-drop-overlay" aria-live="polite">
          <i class="pi pi-upload" aria-hidden="true" />
          <strong>释放以上传</strong>
        </div>
        <div class="asset-manager-toolbar">
          <nav class="asset-breadcrumb" aria-label="资源路径">
            <template v-for="(crumb, index) in crumbs" :key="crumb.path">
              <i v-if="index" class="pi pi-angle-right text-color-secondary" aria-hidden="true" />
              <Button :label="crumb.label" :icon="index === 0 ? 'pi pi-database' : undefined" severity="secondary" variant="text" @click="load(crumb.path)" />
            </template>
          </nav>
          <div class="management-actions">
            <InputText v-model="search" placeholder="搜索文件" />
            <Button icon="pi pi-refresh" severity="secondary" variant="text" aria-label="刷新" :loading="loading" @click="load()" />
            <Button label="新建文件夹" icon="pi pi-folder-plus" severity="secondary" variant="text" @click="folderDialog = true" />
            <Button label="上传" icon="pi pi-upload" :loading="actions.pending.value" @click="fileInput?.click()" />
          </div>
        </div>

        <div v-if="loading" class="asset-manager-loading"><ProgressSpinner aria-label="正在加载资源" /></div>
        <div v-else-if="entries.length" class="asset-manager-grid">
          <article v-for="asset in entries" :key="asset.path" class="asset-manager-item">
            <button type="button" class="asset-manager-open" @click="open(asset)">
              <div class="asset-manager-preview">
                <img v-if="asset.media_type === 'image'" :src="publicAssetUrl(asset.path)" :alt="asset.name" loading="lazy" />
                <i v-else :class="assetIcon(asset)" aria-hidden="true" />
              </div>
              <span class="font-medium overflow-hidden text-overflow-ellipsis white-space-nowrap">{{ asset.name }}</span>
              <small>{{ asset.kind === 'folder' ? '文件夹' : formatAssetSize(asset.size) }}</small>
            </button>
            <Button icon="pi pi-trash" severity="danger" variant="text" rounded aria-label="删除资源" class="asset-delete-button" @click="confirmDelete($event, asset)" />
          </article>
        </div>
        <div v-else class="asset-empty-state">
          <strong>暂无文件</strong>
          <Button label="上传" icon="pi pi-upload" @click="fileInput?.click()" />
        </div>
      </template>
    </Card>
  </PageShell>

  <Dialog v-model:visible="folderDialog" modal header="新建文件夹" :style="{ width: 'min(28rem, 92vw)' }" :draggable="false">
    <div class="flex flex-column gap-2">
      <label for="asset-folder-name" class="font-medium">文件夹名称</label>
      <InputText id="asset-folder-name" v-model="folderName" fluid autofocus placeholder="文件夹名称" @keyup.enter="createFolder" />
    </div>
    <template #footer><Button label="取消" severity="secondary" variant="text" @click="folderDialog = false" /><Button label="创建" icon="pi pi-check" :disabled="!folderName.trim()" :loading="actions.pending.value" @click="createFolder" /></template>
  </Dialog>
</template>
