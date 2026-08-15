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
import type { CharacterInput } from "../types";
import AssetField from "../components/AssetField.vue";
import { formSnapshot } from "../formSnapshot";

const props = defineProps<{ characterId?: string }>();
const router = useRouter();
const session = useAdminSession();
const actions = useAdminActions();
const resource = useResource(adminApi.characters);
const validationError = ref("");
const empty = (): CharacterInput => ({ id: "", name: "", illustration_path: "", emotes: [], voices: [], outfits: [{ id: "default", name: "初始装扮", illustration_path: "" }], enabled: true, is_default: false });
const model = reactive<CharacterInput>(empty());
const editing = computed(() => Boolean(props.characterId));
const pageError = computed(() => resource.error.value ?? actions.error.value);

watch(resource.data, (data) => {
  if (!props.characterId) return;
  const current = data?.characters.find((item) => item.id === props.characterId);
  if (current) Object.assign(model, formSnapshot(current));
}, { immediate: true });

watch(() => model.is_default, (isDefault) => {
  if (isDefault) model.enabled = true;
});

function validate() {
  const required = [model.id, model.name, model.illustration_path,
    ...model.outfits.flatMap((item) => [item.id, item.name, item.illustration_path]),
    ...model.emotes.flatMap((item) => [item.name, item.path]),
    ...model.voices.flatMap((item) => [item.name, item.path])];
  validationError.value = required.every((value) => value.trim()) ? "" : "请填写所有必填项";
  return !validationError.value;
}

async function save() {
  if (!validate()) return;
  const csrf = session.identity.value?.csrf_token;
  if (!csrf) return;
  const input = formSnapshot(model);
  input.id = input.id.trim();
  input.name = input.name.trim();
  input.illustration_path = input.illustration_path.trim();
  input.outfits = input.outfits.map((item) => ({ id: item.id.trim(), name: item.name.trim(), illustration_path: item.illustration_path.trim() }));
  input.emotes = input.emotes.map((item) => ({ name: item.name.trim(), path: item.path.trim() }));
  input.voices = input.voices.map((item) => ({ name: item.name.trim(), path: item.path.trim() }));
  const success = await actions.run(
    () => editing.value ? adminApi.updateCharacter(input, csrf) : adminApi.createCharacter(input, csrf),
    editing.value ? "角色已更新" : "角色已添加",
  );
  if (success) await router.push({ name: "characters" });
}
</script>

<template>
  <PageShell :title="editing ? '编辑角色' : '添加角色'" :error="pageError" :loading="resource.loading.value">
    <Message v-if="validationError" severity="error" :closable="false">{{ validationError }}</Message>
    <form class="flex flex-column gap-4" @submit.prevent="save">
      <Fieldset legend="基础信息">
        <div class="grid">
          <div class="col-12 md:col-6 flex flex-column gap-2"><label for="character-id" class="font-medium">角色编号</label><InputText id="character-id" v-model="model.id" :disabled="editing" fluid /></div>
          <div class="col-12 md:col-6 flex flex-column gap-2"><label for="character-name" class="font-medium">角色名称</label><InputText id="character-name" v-model="model.name" fluid /></div>
          <div class="col-12 flex flex-column gap-2"><label for="illustration-path" class="font-medium">主立绘路径</label><AssetField id="illustration-path" v-model="model.illustration_path" accept="image" upload-folder="characters" /></div>
          <div class="col-12 flex gap-5 flex-wrap"><label class="flex align-items-center gap-2"><ToggleSwitch v-model="model.enabled" :disabled="model.is_default" /><span>启用</span></label><label class="flex align-items-center gap-2"><ToggleSwitch v-model="model.is_default" /><span>设为初始角色</span></label></div>
        </div>
      </Fieldset>

      <Fieldset><template #legend><div class="flex align-items-center gap-3"><span>角色装扮</span><Button label="添加" icon="pi pi-plus" size="small" variant="text" type="button" @click="model.outfits.push({ id: '', name: '', illustration_path: '' })" /></div></template>
        <div class="flex flex-column gap-3"><div v-for="(item, index) in model.outfits" :key="index" class="grid align-items-end">
          <div class="col-12 md:col-3 flex flex-column gap-2"><label :for="`outfit-id-${index}`" class="font-medium">编号</label><InputText :id="`outfit-id-${index}`" v-model="item.id" fluid /></div>
          <div class="col-12 md:col-3 flex flex-column gap-2"><label :for="`outfit-name-${index}`" class="font-medium">名称</label><InputText :id="`outfit-name-${index}`" v-model="item.name" fluid /></div>
          <div class="col-12 md:col-5 flex flex-column gap-2"><label :for="`outfit-path-${index}`" class="font-medium">立绘路径</label><AssetField :id="`outfit-path-${index}`" v-model="item.illustration_path" accept="image" upload-folder="characters/outfits" /></div>
          <div class="col-12 md:col-1"><Button icon="pi pi-trash" severity="danger" variant="text" type="button" aria-label="删除装扮" @click="model.outfits.splice(index, 1)" /></div>
        </div></div>
      </Fieldset>

      <Fieldset><template #legend><div class="flex align-items-center gap-3"><span>表情</span><Button label="添加" icon="pi pi-plus" size="small" variant="text" type="button" @click="model.emotes.push({ name: '', path: '' })" /></div></template>
        <div class="flex flex-column gap-3"><div v-for="(item, index) in model.emotes" :key="index" class="grid align-items-end"><div class="col-12 md:col-4 flex flex-column gap-2"><label :for="`emote-name-${index}`" class="font-medium">名称</label><InputText :id="`emote-name-${index}`" v-model="item.name" fluid /></div><div class="col-12 md:col-7 flex flex-column gap-2"><label :for="`emote-path-${index}`" class="font-medium">资源路径</label><AssetField :id="`emote-path-${index}`" v-model="item.path" accept="image" upload-folder="characters/emotes" /></div><div class="col-12 md:col-1"><Button icon="pi pi-trash" severity="danger" variant="text" type="button" aria-label="删除表情" @click="model.emotes.splice(index, 1)" /></div></div></div>
      </Fieldset>

      <Fieldset><template #legend><div class="flex align-items-center gap-3"><span>语音</span><Button label="添加" icon="pi pi-plus" size="small" variant="text" type="button" @click="model.voices.push({ name: '', path: '' })" /></div></template>
        <div class="flex flex-column gap-3"><div v-for="(item, index) in model.voices" :key="index" class="grid align-items-end"><div class="col-12 md:col-4 flex flex-column gap-2"><label :for="`voice-name-${index}`" class="font-medium">名称</label><InputText :id="`voice-name-${index}`" v-model="item.name" fluid /></div><div class="col-12 md:col-7 flex flex-column gap-2"><label :for="`voice-path-${index}`" class="font-medium">资源路径</label><AssetField :id="`voice-path-${index}`" v-model="item.path" accept="audio" upload-folder="characters/voices" /></div><div class="col-12 md:col-1"><Button icon="pi pi-trash" severity="danger" variant="text" type="button" aria-label="删除语音" @click="model.voices.splice(index, 1)" /></div></div></div>
      </Fieldset>

      <div class="flex justify-content-end gap-2"><Button label="取消" severity="secondary" variant="text" type="button" @click="router.push({ name: 'characters' })" /><Button label="保存" icon="pi pi-check" type="submit" :loading="actions.pending.value" /></div>
    </form>
  </PageShell>
</template>
