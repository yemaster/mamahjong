<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Button from "primevue/button";
import Dialog from "primevue/dialog";
import InputText from "primevue/inputtext";
import Message from "primevue/message";
import ProgressSpinner from "primevue/progressspinner";
import { useToast } from "primevue/usetoast";
import { adminApi } from "../api";
import { assetIcon, formatAssetSize, parentAssetPath, publicAssetUrl } from "../assetPaths";
import { useAdminSession } from "../session";
import { acceptsAssetFile, useFileDrop } from "../composables/useFileDrop";
import type { AssetList, ManagedAsset } from "../types";

const props = withDefaults(defineProps<{
  visible: boolean;
  accept?: "image" | "audio" | "all";
}>(), { accept: "all" });
const emit = defineEmits<{
  "update:visible": [value: boolean];
  select: [url: string];
}>();

const session = useAdminSession();
const toast = useToast();
const data = ref<AssetList>();
const loading = ref(false);
const uploading = ref(false);
const error = ref<Error>();
const search = ref("");
const fileInput = ref<HTMLInputElement>();
const currentPath = computed(() => data.value?.path ?? "");
const fileAccept = computed(() => props.accept === "image" ? "image/*" : props.accept === "audio" ? "audio/*" : undefined);
const entries = computed(() => {
  const keyword = search.value.trim().toLocaleLowerCase();
  return (data.value?.entries ?? []).filter((asset) => {
    const accepted = asset.kind === "folder" || props.accept === "all" || asset.media_type === props.accept;
    return accepted && (!keyword || asset.name.toLocaleLowerCase().includes(keyword));
  });
});
const crumbs = computed(() => {
  const parts = currentPath.value.split("/").filter(Boolean);
  return [{ label: "资源库", path: "" }, ...parts.map((label, index) => ({ label, path: parts.slice(0, index + 1).join("/") }))];
});
const fileDrop = useFileDrop((files) => uploadFile(files[0]));

async function load(path = "") {
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

function activate(asset: ManagedAsset) {
  if (asset.kind === "folder") {
    void load(asset.path);
    return;
  }
  emit("select", publicAssetUrl(asset.path));
  emit("update:visible", false);
}

async function uploadFile(file?: File) {
  const csrf = session.identity.value?.csrf_token;
  if (!file || !csrf) return;
  if (!acceptsAssetFile(file, props.accept)) {
    toast.add({ severity: "warn", summary: "文件类型不匹配", detail: props.accept === "image" ? "请选择图片文件" : "请选择音频文件", life: 3000 });
    return;
  }
  uploading.value = true;
  try {
    const asset = await adminApi.uploadAsset(currentPath.value, file, csrf);
    await load(currentPath.value);
    emit("select", publicAssetUrl(asset.path));
    emit("update:visible", false);
    toast.add({ severity: "success", summary: "上传并选择成功", life: 2200 });
  } catch (cause) {
    toast.add({ severity: "error", summary: "上传失败", detail: cause instanceof Error ? cause.message : "请求失败", life: 3500 });
  } finally {
    uploading.value = false;
  }
}

function upload(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  void uploadFile(file);
}

watch(() => props.visible, (visible) => {
  if (visible) {
    fileDrop.reset();
    search.value = "";
    void load("");
  }
});
</script>

<template>
  <Dialog :visible="visible" modal header="选择资源" :style="{ width: 'min(58rem, 94vw)' }" :draggable="false" @update:visible="emit('update:visible', $event)">
    <div class="asset-picker-dropzone" :class="{ 'asset-drop-active': fileDrop.active.value }" @dragenter.prevent="fileDrop.enter" @dragover.prevent="fileDrop.over" @dragleave.prevent="fileDrop.leave" @drop.prevent="fileDrop.drop">
      <div v-if="fileDrop.active.value" class="asset-drop-overlay" aria-live="polite">
        <i class="pi pi-upload" aria-hidden="true" />
        <strong>释放以上传</strong>
      </div>
    <div class="asset-picker-toolbar">
      <nav class="asset-breadcrumb" aria-label="资源路径">
        <template v-for="(crumb, index) in crumbs" :key="crumb.path">
          <i v-if="index" class="pi pi-angle-right text-color-secondary" aria-hidden="true" />
          <Button :label="crumb.label" :icon="index === 0 ? 'pi pi-database' : undefined" size="small" severity="secondary" variant="text" @click="load(crumb.path)" />
        </template>
      </nav>
      <div class="flex gap-2">
        <InputText v-model="search" placeholder="搜索文件" class="asset-picker-search" />
        <Button label="上传" icon="pi pi-upload" :loading="uploading" @click="fileInput?.click()" />
        <input ref="fileInput" class="asset-hidden-input" type="file" :accept="fileAccept" @change="upload" />
      </div>
    </div>

    <Message v-if="error" severity="error" :closable="false" class="mt-3">{{ error.message }}</Message>
    <div v-if="loading" class="asset-picker-loading"><ProgressSpinner aria-label="正在加载资源" /></div>
    <div v-else-if="entries.length" class="asset-picker-grid mt-3">
      <button v-for="asset in entries" :key="asset.path" type="button" class="asset-picker-item" @click="activate(asset)">
        <div class="asset-picker-preview">
          <img v-if="asset.media_type === 'image'" :src="publicAssetUrl(asset.path)" :alt="asset.name" loading="lazy" />
          <i v-else :class="assetIcon(asset)" aria-hidden="true" />
        </div>
        <div class="asset-picker-meta">
          <span class="font-medium overflow-hidden text-overflow-ellipsis white-space-nowrap">{{ asset.name }}</span>
          <span class="text-xs text-color-secondary">{{ asset.kind === 'folder' ? '文件夹' : formatAssetSize(asset.size) }}</span>
        </div>
        <i v-if="asset.kind === 'folder'" class="pi pi-angle-right text-color-secondary" aria-hidden="true" />
      </button>
    </div>
    <div v-else class="asset-empty-state">
      <strong>暂无资源</strong>
    </div>
    </div>
    <template #footer>
      <Button label="上一级" icon="pi pi-arrow-up" severity="secondary" variant="text" :disabled="!currentPath" @click="load(parentAssetPath(currentPath))" />
      <Button label="取消" severity="secondary" variant="outlined" @click="emit('update:visible', false)" />
    </template>
  </Dialog>
</template>
