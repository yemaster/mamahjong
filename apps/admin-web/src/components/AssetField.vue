<script setup lang="ts">
import { ref } from "vue";
import Button from "primevue/button";
import InputText from "primevue/inputtext";
import { useToast } from "primevue/usetoast";
import { adminApi } from "../api";
import { publicAssetUrl } from "../assetPaths";
import { useAdminSession } from "../session";
import AssetPickerDialog from "./AssetPickerDialog.vue";
import { acceptsAssetFile, useFileDrop } from "../composables/useFileDrop";

const props = withDefaults(defineProps<{
  modelValue: string;
  id?: string;
  placeholder?: string;
  accept?: "image" | "audio" | "all";
  uploadFolder?: string;
}>(), { placeholder: "/user-assets/...", accept: "all", uploadFolder: "" });
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const session = useAdminSession();
const toast = useToast();
const pickerVisible = ref(false);
const uploading = ref(false);
const input = ref<HTMLInputElement>();
const fileDrop = useFileDrop((files) => uploadFile(files[0]));

async function uploadFile(file?: File) {
  const csrf = session.identity.value?.csrf_token;
  if (!file || !csrf) return;
  if (!acceptsAssetFile(file, props.accept)) {
    toast.add({ severity: "warn", summary: "文件类型不匹配", detail: props.accept === "image" ? "请拖入图片文件" : "请拖入音频文件", life: 3000 });
    return;
  }
  uploading.value = true;
  try {
    const asset = await adminApi.uploadAsset(props.uploadFolder, file, csrf);
    emit("update:modelValue", publicAssetUrl(asset.path));
    toast.add({ severity: "success", summary: "资源已上传并设置", detail: asset.name, life: 2200 });
  } catch (cause) {
    toast.add({ severity: "error", summary: "上传失败", detail: cause instanceof Error ? cause.message : "请求失败", life: 3500 });
  } finally {
    uploading.value = false;
  }
}

function quickUpload(event: Event) {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  target.value = "";
  void uploadFile(file);
}
</script>

<template>
  <div class="asset-field" :class="{ 'asset-field-drop-active': fileDrop.active.value }" @dragenter.prevent="fileDrop.enter" @dragover.prevent="fileDrop.over" @dragleave.prevent="fileDrop.leave" @drop.prevent="fileDrop.drop">
    <span v-if="fileDrop.active.value" class="asset-field-drop-label"><i class="pi pi-upload" /> 释放以上传</span>
    <InputText :id="id" :model-value="modelValue" :placeholder="placeholder" fluid class="asset-field-input" @update:model-value="emit('update:modelValue', String($event ?? ''))" />
    <Button label="资源库" icon="pi pi-folder-open" severity="secondary" variant="outlined" type="button" @click="pickerVisible = true" />
    <Button label="上传" icon="pi pi-upload" severity="secondary" variant="outlined" type="button" :loading="uploading" @click="input?.click()" />
    <input ref="input" class="asset-hidden-input" type="file" :accept="accept === 'image' ? 'image/*' : accept === 'audio' ? 'audio/*' : undefined" @change="quickUpload" />
  </div>
  <AssetPickerDialog v-model:visible="pickerVisible" :accept="accept" @select="emit('update:modelValue', $event)" />
</template>
