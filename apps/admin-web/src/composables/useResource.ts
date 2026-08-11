import { onMounted, ref, shallowRef, type Ref, type ShallowRef } from "vue";

export interface Resource<T> {
  data: ShallowRef<T | undefined>;
  error: Ref<Error | undefined>;
  loading: Ref<boolean>;
  reload: () => Promise<T | undefined>;
}

export function useResource<T>(loader: () => Promise<T>, immediate = true): Resource<T> {
  const data = shallowRef<T>();
  const error = ref<Error>();
  const loading = ref(false);

  async function reload() {
    loading.value = true;
    error.value = undefined;
    try {
      const value = await loader();
      data.value = value;
      return value;
    } catch (cause) {
      error.value = cause instanceof Error ? cause : new Error("请求失败");
      return undefined;
    } finally {
      loading.value = false;
    }
  }

  if (immediate) onMounted(reload);
  return { data, error, loading, reload };
}
