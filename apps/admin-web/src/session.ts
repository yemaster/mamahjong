import { readonly, ref } from "vue";
import { adminApi, ApiError } from "./api";
import type { AdminIdentity } from "./types";

const identity = ref<AdminIdentity>();
const loading = ref(true);
const error = ref<Error>();
let initialized = false;

async function initialize(force = false) {
  if (initialized && !force) return;
  initialized = true;
  loading.value = true;
  error.value = undefined;
  try {
    identity.value = await adminApi.identity();
  } catch (cause) {
    identity.value = undefined;
    if (!(cause instanceof ApiError && cause.status === 401)) {
      error.value = cause instanceof Error ? cause : new Error("管理端不可用");
    }
  } finally {
    loading.value = false;
  }
}

async function signIn(loginName: string, password: string, loginCsrf: string) {
  identity.value = await adminApi.login(loginName, password, loginCsrf);
  error.value = undefined;
}

async function signOut() {
  const current = identity.value;
  try {
    if (current) await adminApi.logout(current.csrf_token);
  } finally {
    identity.value = undefined;
  }
}

function expire() {
  identity.value = undefined;
}

export function useAdminSession() {
  return {
    identity: readonly(identity),
    loading: readonly(loading),
    error: readonly(error),
    initialize,
    signIn,
    signOut,
    expire,
  };
}
