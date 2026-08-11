<script setup lang="ts">
import Menu from "primevue/menu";
import type { MenuItem } from "primevue/menuitem";

defineProps<{ model: MenuItem[] }>();
const emit = defineEmits<{ navigate: [] }>();

function navigateTo(navigate: (event?: MouseEvent) => unknown, event: MouseEvent) {
  void navigate(event);
  emit("navigate");
}
</script>

<template>
  <Menu :model="model" class="admin-navigation w-full border-none">
    <template #submenulabel="{ item }"><span class="text-xs font-semibold text-color-secondary">{{ item.label }}</span></template>
    <template #item="{ item, props }">
      <RouterLink v-slot="{ href, navigate, isActive }" :to="item.route" custom>
        <a :href="href" v-bind="props.action" :class="{ 'admin-navigation-active': isActive }" @click="navigateTo(navigate, $event)">
          <span v-bind="props.icon" aria-hidden="true" />
          <span v-bind="props.label">{{ item.label }}</span>
        </a>
      </RouterLink>
    </template>
  </Menu>
</template>
