<script setup lang="ts">
import { computed, ref } from "vue";
import Button from "primevue/button";
import Column from "primevue/column";
import DataTable from "primevue/datatable";
import Dialog from "primevue/dialog";
import IconField from "primevue/iconfield";
import InputIcon from "primevue/inputicon";
import InputText from "primevue/inputtext";
import Message from "primevue/message";
import Tag from "primevue/tag";
import { adminApi } from "../api";
import PageShell from "../components/PageShell.vue";
import { useAdminActions } from "../composables/useAdminActions";
import { useResource } from "../composables/useResource";
import { useAdminSession } from "../session";
import type { AccountStatus, AdminUser } from "../types";
import { completeAdminBatch } from "../batchActions";

const session = useAdminSession();
const actions = useAdminActions();
const resource = useResource(adminApi.users);
const search = ref("");
const selected = ref<AdminUser[]>([]);
const editing = ref<AdminUser>();
const nickname = ref("");
const nicknameError = ref("");
const pageError = computed(() => resource.error.value ?? actions.error.value);
const users = computed(() => {
  const keyword = search.value.trim().toLocaleLowerCase();
  return (resource.data.value?.users ?? []).filter((user) => !keyword || `${user.login_name} ${user.nickname}`.toLocaleLowerCase().includes(keyword));
});

function selectedStatusIds(status: AccountStatus) {
  return selected.value
    .filter((user) => user.id !== session.identity.value?.id && user.status !== status)
    .map((user) => user.id);
}

function openEdit(user: AdminUser) {
  editing.value = user;
  nickname.value = user.nickname;
  nicknameError.value = "";
}

async function saveUser() {
  const value = nickname.value.trim();
  if (!value) { nicknameError.value = "请输入昵称"; return; }
  const user = editing.value;
  const csrf = session.identity.value?.csrf_token;
  if (!user || !csrf) return;
  const success = await actions.run(() => adminApi.updateUser(user.id, value, csrf), "用户资料已更新");
  if (success) { editing.value = undefined; await resource.reload(); }
}

async function updateUsers(ids: string[], status: AccountStatus) {
  const csrf = session.identity.value?.csrf_token;
  if (!csrf || !ids.length) return;
  const success = await actions.run(() => completeAdminBatch(ids.map((id) => () => adminApi.updateUserStatus(id, status, csrf))), "账号状态已更新");
  await resource.reload();
  if (success) selected.value = [];
}

function confirmUser(event: Event, user: AdminUser) {
  const status: AccountStatus = user.status === "active" ? "suspended" : "active";
  actions.require(event, status === "suspended" ? "停用账号" : "恢复账号", user.nickname, () => updateUsers([user.id], status), status === "suspended");
}

function confirmSelected(event: Event, status: AccountStatus) {
  const ids = selectedStatusIds(status);
  if (!ids.length) return;
  actions.require(event, status === "suspended" ? "批量停用" : "批量恢复", `确定${status === "suspended" ? "停用" : "恢复"} ${ids.length} 个账号？`, () => updateUsers(ids, status), status === "suspended");
}
</script>

<template>
  <PageShell title="用户管理" :error="pageError" :loading="resource.loading.value">
    <DataTable v-model:selection="selected" :value="users" data-key="id" paginator :rows="10" :rows-per-page-options="[10, 20, 50]" scrollable table-style="min-width: 50rem">
      <template #header><div class="management-toolbar"><div class="management-filters">
        <IconField><InputIcon class="pi pi-search" /><InputText v-model="search" placeholder="搜索用户" /></IconField>
      </div><div class="management-actions"><Button icon="pi pi-refresh" severity="secondary" variant="text" aria-label="刷新" :loading="resource.loading.value" @click="resource.reload" />
        <Button v-if="selected.length" :label="`恢复所选（${selectedStatusIds('active').length}）`" icon="pi pi-check-circle" severity="secondary" variant="outlined" :disabled="!selectedStatusIds('active').length" @click="confirmSelected($event, 'active')" />
        <Button v-if="selected.length" :label="`停用所选（${selectedStatusIds('suspended').length}）`" icon="pi pi-ban" severity="danger" variant="outlined" :disabled="!selectedStatusIds('suspended').length" @click="confirmSelected($event, 'suspended')" />
      </div></div></template>
        <Column selection-mode="multiple" header-style="width: 3rem" />
        <Column field="nickname" header="昵称" style="width: 12rem" />
        <Column field="login_name" header="账号" />
        <Column field="role" header="类型" style="width: 8rem"><template #body="{ data }">{{ data.role === 'administrator' ? '管理员' : '玩家' }}</template></Column>
        <Column field="status" header="状态" style="width: 8rem"><template #body="{ data }"><Tag :severity="data.status === 'active' ? 'success' : 'secondary'" :value="data.status === 'active' ? '正常' : '已停用'" /></template></Column>
        <Column header="操作" style="width: 12rem"><template #body="{ data }"><div class="flex gap-1">
          <Button label="编辑" icon="pi pi-pencil" variant="text" size="small" @click="openEdit(data)" />
          <Button :label="data.status === 'active' ? '停用' : '恢复'" :severity="data.status === 'active' ? 'danger' : 'secondary'" variant="text" size="small" :disabled="data.id === session.identity.value?.id" @click="confirmUser($event, data)" />
        </div></template></Column>
        <template #empty>暂无用户</template>
    </DataTable>

    <Dialog :visible="Boolean(editing)" modal header="编辑用户" :style="{ width: 'min(28rem, calc(100vw - 2rem))' }" @update:visible="(visible) => { if (!visible) editing = undefined; }">
      <div class="flex flex-column gap-2"><label for="nickname" class="font-medium">昵称</label><InputText id="nickname" v-model="nickname" fluid autofocus /><Message v-if="nicknameError" severity="error" size="small" variant="simple">{{ nicknameError }}</Message></div>
      <template #footer><Button label="取消" severity="secondary" variant="text" @click="editing = undefined" /><Button label="保存" icon="pi pi-check" :loading="actions.pending.value" @click="saveUser" /></template>
    </Dialog>
  </PageShell>
</template>
