<script setup lang="ts">
import Message from "primevue/message";
import Skeleton from "primevue/skeleton";
import Toolbar from "primevue/toolbar";

defineProps<{
  title: string;
  error?: Error;
  loading?: boolean;
}>();
</script>

<template>
  <section class="page-content">
    <Toolbar class="page-heading border-none p-0 bg-transparent">
      <template #start>
        <h1 class="m-0">{{ title }}</h1>
      </template>
      <template v-if="$slots.actions" #end><div class="flex align-items-center gap-2 flex-wrap"><slot name="actions" /></div></template>
    </Toolbar>
    <Message v-if="error" severity="error" :closable="false" class="mt-4">{{ error.message }}</Message>
    <div v-if="loading" class="page-loading-panel mt-4" aria-label="正在加载">
      <Skeleton width="35%" height="1.25rem" />
      <Skeleton width="70%" height="1rem" />
      <Skeleton width="100%" height="16rem" border-radius="0.75rem" />
    </div>
    <div v-else class="page-body flex flex-column gap-4 mt-4"><slot /></div>
  </section>
</template>
