<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import Accordion from "primevue/accordion";
import AccordionContent from "primevue/accordioncontent";
import AccordionHeader from "primevue/accordionheader";
import AccordionPanel from "primevue/accordionpanel";
import Button from "primevue/button";
import Card from "primevue/card";
import Column from "primevue/column";
import DataTable from "primevue/datatable";
import Message from "primevue/message";
import Tag from "primevue/tag";
import { adminApi } from "../api";
import PageShell from "../components/PageShell.vue";
import { useResource } from "../composables/useResource";

type HandView = { dealer?: number; reason?: string; winners?: number[]; events?: unknown[]; point_deltas?: number[] };
const props = defineProps<{ matchId: string }>();
const router = useRouter();
const resource = useResource(() => adminApi.matchDetail(props.matchId));
const hands = computed(() => (resource.data.value?.hands ?? []) as HandView[]);
const players = computed(() => {
  const detail = resource.data.value;
  const placements = detail?.result?.placements ?? [];
  return (detail?.players ?? []).map((player) => ({ ...player, ...placements.find((item) => item.seat === player.seat) })).sort((left, right) => (left.rank ?? 9) - (right.rank ?? 9));
});
</script>

<template>
  <PageShell title="对局详情" :error="resource.error.value" :loading="resource.loading.value">
    <template #actions><Button label="返回" icon="pi pi-arrow-left" severity="secondary" variant="outlined" @click="router.push({ name: 'matches' })" /></template>
    <Card>
      <template #content>
        <div class="grid">
          <div class="col-12 md:col-6 xl:col-3"><div class="text-color-secondary mb-2">对局编号</div><div class="font-medium break-all">{{ resource.data.value?.match_id ?? matchId }}</div></div>
          <div class="col-12 md:col-6 xl:col-3"><div class="text-color-secondary mb-2">类型</div><Tag severity="secondary" :value="resource.data.value?.friend_match ? '好友房' : '匹配'" /></div>
          <div class="col-12 md:col-6 xl:col-3"><div class="text-color-secondary mb-2">版本</div><div class="font-medium">{{ resource.data.value?.version ?? '-' }}</div></div>
          <div class="col-12 md:col-6 xl:col-3"><div class="text-color-secondary mb-2">局数</div><div class="font-medium">{{ hands.length }}</div></div>
        </div>
      </template>
    </Card>
    <Card><template #title>玩家与结果</template><template #content>
      <DataTable :value="players" data-key="seat" scrollable table-style="min-width: 48rem">
        <Column field="rank" header="名次" style="width: 6rem"><template #body="{ data }">{{ data.rank ?? '-' }}</template></Column>
        <Column field="nickname" header="玩家" />
        <Column field="user_id" header="用户编号" />
        <Column field="seat" header="座位" style="width: 6rem" />
        <Column field="points" header="素点" style="width: 7rem" />
        <Column field="score_tenths" header="得分" style="width: 7rem"><template #body="{ data }">{{ data.score_tenths == null ? '-' : (data.score_tenths / 10).toFixed(1) }}</template></Column>
        <template #empty>暂无玩家信息</template>
      </DataTable>
    </template></Card>
    <Card><template #title>局记录</template><template #content>
      <Accordion v-if="hands.length" multiple>
        <AccordionPanel v-for="(hand, index) in hands" :key="index" :value="String(index)">
          <AccordionHeader><div class="flex align-items-center justify-content-between w-full pr-3"><span>第 {{ index + 1 }} 局</span><Tag v-if="hand.reason" severity="secondary" :value="hand.reason" /></div></AccordionHeader>
          <AccordionContent><div class="grid">
            <div class="col-12 md:col-3"><div class="text-color-secondary mb-1">庄家座位</div>{{ hand.dealer ?? '-' }}</div>
            <div class="col-12 md:col-3"><div class="text-color-secondary mb-1">和牌座位</div>{{ hand.winners?.join('、') || '无' }}</div>
            <div class="col-12 md:col-3"><div class="text-color-secondary mb-1">事件数</div>{{ hand.events?.length ?? 0 }}</div>
            <div class="col-12 md:col-3"><div class="text-color-secondary mb-1">点数变化</div>{{ hand.point_deltas?.join(' / ') || '-' }}</div>
          </div></AccordionContent>
        </AccordionPanel>
      </Accordion>
      <Message v-else severity="secondary" :closable="false">暂无局记录</Message>
    </template></Card>
  </PageShell>
</template>
