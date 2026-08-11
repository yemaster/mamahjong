import { ref } from "vue";
import { useConfirm } from "primevue/useconfirm";
import { useToast } from "primevue/usetoast";

export function useAdminActions() {
  const confirm = useConfirm();
  const toast = useToast();
  const pending = ref(false);
  const error = ref<Error>();

  async function run(task: () => Promise<unknown>, success: string) {
    pending.value = true;
    error.value = undefined;
    try {
      await task();
      toast.add({ severity: "success", summary: success, life: 2500 });
      return true;
    } catch (cause) {
      error.value = cause instanceof Error ? cause : new Error("请求失败");
      toast.add({ severity: "error", summary: "操作失败", detail: error.value.message, life: 3500 });
      return false;
    } finally {
      pending.value = false;
    }
  }

  function require(event: Event, header: string, message: string, task: () => Promise<void>, danger = false) {
    confirm.require({
      target: event.currentTarget as HTMLElement,
      header,
      message,
      icon: danger ? "pi pi-exclamation-triangle" : "pi pi-question-circle",
      acceptLabel: "确定",
      rejectLabel: "取消",
      acceptProps: danger ? { severity: "danger" } : undefined,
      accept: task,
    });
  }

  return { pending, error, run, require };
}
