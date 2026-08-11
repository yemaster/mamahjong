<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useRouter } from "vue-router";
import Button from "primevue/button";
import Fieldset from "primevue/fieldset";
import InputNumber from "primevue/inputnumber";
import InputText from "primevue/inputtext";
import Message from "primevue/message";
import Select from "primevue/select";
import ToggleSwitch from "primevue/toggleswitch";
import { adminApi } from "../api";
import PageShell from "../components/PageShell.vue";
import { useAdminActions } from "../composables/useAdminActions";
import { useResource } from "../composables/useResource";
import { useAdminSession } from "../session";
import type { MusicInput } from "../types";

const props = defineProps<{ musicId?: string }>();
const router = useRouter();
const session = useAdminSession();
const actions = useAdminActions();
const resource = useResource(adminApi.music);
const model = reactive<MusicInput>({ id: "", name: "", scene: "lobby", audio_path: "", duration_ms: 0, enabled: true, is_default: false });
const validationError = ref("");
const editing = computed(() => Boolean(props.musicId));
const pageError = computed(() => resource.error.value ?? actions.error.value);
const scenes = [{ label: "大厅", value: "lobby" }, { label: "对局", value: "match" }, { label: "立直", value: "riichi" }];
watch(resource.data, (data) => { const current = data?.music_tracks.find((item) => item.id === props.musicId); if (current) Object.assign(model, structuredClone(current)); }, { immediate: true });

async function save() {
  validationError.value = [model.id, model.name, model.audio_path].every((value) => value.trim()) && model.duration_ms > 0 ? "" : "请填写所有必填项，时长必须大于 0";
  const csrf = session.identity.value?.csrf_token;
  if (validationError.value || !csrf) return;
  const input = structuredClone(model);
  const success = await actions.run(() => editing.value ? adminApi.updateMusic(input, csrf) : adminApi.createMusic(input, csrf), editing.value ? "音乐已更新" : "音乐已添加");
  if (success) await router.push({ name: "music" });
}
</script>

<template>
  <PageShell :title="editing ? '编辑音乐' : '添加音乐'" :error="pageError" :loading="resource.loading.value">
    <template #actions><Button label="返回" icon="pi pi-arrow-left" severity="secondary" variant="outlined" @click="router.push({ name: 'music' })" /></template>
    <Message v-if="validationError" severity="error" :closable="false">{{ validationError }}</Message>
    <form class="flex flex-column gap-4" @submit.prevent="save"><Fieldset legend="音乐信息">
      <div class="flex flex-column gap-4">
      <div class="grid"><div class="col-12 md:col-6 flex flex-column gap-2"><label for="music-id" class="font-medium">音乐编号</label><InputText id="music-id" v-model="model.id" :disabled="editing" fluid placeholder="例如 lobby-default" /></div><div class="col-12 md:col-6 flex flex-column gap-2"><label for="music-name" class="font-medium">音乐名称</label><InputText id="music-name" v-model="model.name" fluid /></div></div>
      <div class="grid"><div class="col-12 md:col-6 flex flex-column gap-2"><label for="music-scene" class="font-medium">使用场景</label><Select id="music-scene" v-model="model.scene" :options="scenes" option-label="label" option-value="value" fluid /></div><div class="col-12 md:col-6 flex flex-column gap-2"><label for="music-duration" class="font-medium">时长（毫秒）</label><InputNumber id="music-duration" v-model="model.duration_ms" :min="1" :use-grouping="false" fluid /></div></div>
      <div class="flex flex-column gap-2"><label for="audio-path" class="font-medium">音频路径</label><InputText id="audio-path" v-model="model.audio_path" fluid placeholder="/game/assets/local-game-assets/music/..." /></div>
      <div class="flex gap-5 flex-wrap"><label class="flex align-items-center gap-2"><ToggleSwitch v-model="model.enabled" /><span>启用</span></label><label class="flex align-items-center gap-2"><ToggleSwitch v-model="model.is_default" /><span>设为默认音乐</span></label></div>
      </div>
      </Fieldset>
      <div class="flex justify-content-end gap-2"><Button label="取消" severity="secondary" variant="text" type="button" @click="router.push({ name: 'music' })" /><Button label="保存" icon="pi pi-check" type="submit" :loading="actions.pending.value" /></div>
    </form>
  </PageShell>
</template>
