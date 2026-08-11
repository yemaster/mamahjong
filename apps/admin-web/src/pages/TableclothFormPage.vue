<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useRouter } from "vue-router";
import Button from "primevue/button";
import Fieldset from "primevue/fieldset";
import InputText from "primevue/inputtext";
import Message from "primevue/message";
import ToggleSwitch from "primevue/toggleswitch";
import { adminApi } from "../api";
import PageShell from "../components/PageShell.vue";
import { useAdminActions } from "../composables/useAdminActions";
import { useResource } from "../composables/useResource";
import { useAdminSession } from "../session";
import type { TableclothInput } from "../types";

const props = defineProps<{ tableclothId?: string }>();
const router = useRouter();
const session = useAdminSession();
const actions = useAdminActions();
const resource = useResource(adminApi.tablecloths);
const model = reactive<TableclothInput>({ id: "", name: "", texture_path: "", enabled: true, is_default: false });
const validationError = ref("");
const editing = computed(() => Boolean(props.tableclothId));
const pageError = computed(() => resource.error.value ?? actions.error.value);
watch(resource.data, (data) => { const current = data?.tablecloths.find((item) => item.id === props.tableclothId); if (current) Object.assign(model, structuredClone(current)); }, { immediate: true });

async function save() {
  validationError.value = [model.id, model.name, model.texture_path].every((value) => value.trim()) ? "" : "请填写所有必填项";
  const csrf = session.identity.value?.csrf_token;
  if (validationError.value || !csrf) return;
  const input = structuredClone(model);
  const success = await actions.run(() => editing.value ? adminApi.updateTablecloth(input, csrf) : adminApi.createTablecloth(input, csrf), editing.value ? "桌布已更新" : "桌布已添加");
  if (success) await router.push({ name: "tablecloths" });
}
</script>

<template>
  <PageShell :title="editing ? '编辑桌布' : '添加桌布'" :error="pageError" :loading="resource.loading.value">
    <template #actions><Button label="返回" icon="pi pi-arrow-left" severity="secondary" variant="outlined" @click="router.push({ name: 'tablecloths' })" /></template>
    <Message v-if="validationError" severity="error" :closable="false">{{ validationError }}</Message>
    <form class="flex flex-column gap-4" @submit.prevent="save"><Fieldset legend="桌布信息">
      <div class="flex flex-column gap-4">
      <div class="grid"><div class="col-12 md:col-6 flex flex-column gap-2"><label for="tablecloth-id" class="font-medium">桌布编号</label><InputText id="tablecloth-id" v-model="model.id" :disabled="editing" fluid placeholder="例如 peacock-green" /></div><div class="col-12 md:col-6 flex flex-column gap-2"><label for="tablecloth-name" class="font-medium">桌布名称</label><InputText id="tablecloth-name" v-model="model.name" fluid /></div></div>
      <div class="flex flex-column gap-2"><label for="texture-path" class="font-medium">纹理路径</label><InputText id="texture-path" v-model="model.texture_path" fluid placeholder="/game/assets/local-game-assets/..." /></div>
      <div class="flex gap-5 flex-wrap"><label class="flex align-items-center gap-2"><ToggleSwitch v-model="model.enabled" /><span>启用</span></label><label class="flex align-items-center gap-2"><ToggleSwitch v-model="model.is_default" /><span>设为初始桌布</span></label></div>
      </div>
      </Fieldset>
      <div class="flex justify-content-end gap-2"><Button label="取消" severity="secondary" variant="text" type="button" @click="router.push({ name: 'tablecloths' })" /><Button label="保存" icon="pi pi-check" type="submit" :loading="actions.pending.value" /></div>
    </form>
  </PageShell>
</template>
