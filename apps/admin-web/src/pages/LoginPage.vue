<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import Button from "primevue/button";
import Card from "primevue/card";
import InputText from "primevue/inputtext";
import Message from "primevue/message";
import Password from "primevue/password";
import ProgressSpinner from "primevue/progressspinner";
import { adminApi } from "../api";
import { useAdminSession } from "../session";
import type { SessionBootstrap } from "../types";

const router = useRouter();
const session = useAdminSession();
const bootstrap = ref<SessionBootstrap>();
const loading = ref(true);
const submitting = ref(false);
const error = ref<Error>();
const loginName = ref("");
const password = ref("");
const loginNameError = ref("");
const passwordError = ref("");

function validate() {
  loginNameError.value = loginName.value.trim() ? "" : "请输入账号";
  passwordError.value = password.value ? "" : "请输入密码";
  return !loginNameError.value && !passwordError.value;
}

async function submit() {
  if (!validate() || !bootstrap.value?.login_csrf) return;
  submitting.value = true;
  error.value = undefined;
  try {
    await session.signIn(loginName.value.trim(), password.value, bootstrap.value.login_csrf);
    await router.replace({ name: "overview" });
  } catch (cause) {
    error.value = cause instanceof Error ? cause : new Error("登录失败");
  } finally {
    submitting.value = false;
  }
}

onMounted(async () => {
  try {
    bootstrap.value = await adminApi.bootstrap();
  } catch (cause) {
    error.value = cause instanceof Error ? cause : new Error("管理端不可用");
  } finally {
    loading.value = false;
  }
});
</script>

<template>
  <main class="admin-login-page">
    <ProgressSpinner v-if="loading" aria-label="正在加载" />
    <Card v-else class="admin-login-card">
      <template #content>
        <div class="admin-login-heading">
          <h1>管理后台</h1>
        </div>

        <Message v-if="!bootstrap?.enabled && !error" severity="warn" :closable="false" class="mb-4">管理端未启用</Message>
        <Message v-if="error" severity="error" :closable="false" class="mb-4">{{ error.message }}</Message>

        <form v-if="bootstrap?.enabled" class="flex flex-column gap-4" @submit.prevent="submit">
          <div class="flex flex-column gap-2">
            <label for="loginName" class="font-medium">账号</label>
            <InputText id="loginName" v-model="loginName" name="loginName" autocomplete="username" autofocus fluid placeholder="请输入账号" :invalid="Boolean(loginNameError)" @input="loginNameError = ''" />
            <Message v-if="loginNameError" severity="error" size="small" variant="simple">{{ loginNameError }}</Message>
          </div>
          <div class="flex flex-column gap-2">
            <label for="password" class="font-medium">密码</label>
            <Password id="password" v-model="password" name="password" autocomplete="current-password" :feedback="false" toggle-mask fluid placeholder="请输入密码" :invalid="Boolean(passwordError)" @input="passwordError = ''" />
            <Message v-if="passwordError" severity="error" size="small" variant="simple">{{ passwordError }}</Message>
          </div>
          <Button type="submit" label="登录" icon="pi pi-sign-in" fluid size="large" :loading="submitting" />
        </form>
      </template>
    </Card>
  </main>
</template>
