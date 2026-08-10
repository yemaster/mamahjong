import { ProCard, StatisticCard } from "@ant-design/pro-components/es/card";
import { useQuery } from "@tanstack/react-query";
import { Flex, Table, Tag, type TableColumnsType } from "antd";
import { adminApi } from "../api";
import { Page } from "../components/Page";
import { actionLabel, formatDateTime } from "../format";
import type { AuditEvent } from "../types";

const auditColumns: TableColumnsType<AuditEvent> = [
  {
    title: "时间",
    dataIndex: "occurred_at",
    width: 180,
    render: (value: string) => formatDateTime(value),
  },
  {
    title: "操作",
    dataIndex: "action",
    width: 160,
    render: (value: string) => actionLabel(value),
  },
  {
    title: "说明",
    dataIndex: "detail",
    ellipsis: true,
  },
  {
    title: "结果",
    dataIndex: "outcome",
    width: 90,
    render: (value: string) => (
      <Tag color={value === "success" ? "success" : "error"}>
        {value === "success" ? "成功" : "失败"}
      </Tag>
    ),
  },
];

export function OverviewPage() {
  const query = useQuery({
    queryKey: ["admin", "overview"],
    queryFn: adminApi.overview,
  });

  return (
    <Page title="概览" error={query.error}>
      <Flex vertical gap="large">
        <ProCard ghost wrap gutter={[24, 24]}>
          <StatisticCard
            colSpan={{ xs: 24, sm: 8 }}
            loading={query.isLoading}
            statistic={{
              title: "用户",
              value: query.data?.user_count ?? 0,
              suffix: "人",
            }}
          />
          <StatisticCard
            colSpan={{ xs: 24, sm: 8 }}
            loading={query.isLoading}
            statistic={{
              title: "等待中的房间",
              value: query.data?.waiting_room_count ?? 0,
              suffix: "间",
            }}
          />
          <StatisticCard
            colSpan={{ xs: 24, sm: 8 }}
            loading={query.isLoading}
            statistic={{
              title: "进行中的房间",
              value: query.data?.playing_room_count ?? 0,
              suffix: "间",
            }}
          />
        </ProCard>
        <ProCard title="最近审计" variant="outlined">
          <Table<AuditEvent>
            rowKey="sequence"
            loading={query.isLoading}
            dataSource={query.data?.recent_audit}
            columns={auditColumns}
            pagination={false}
            scroll={{ x: 720 }}
          />
        </ProCard>
      </Flex>
    </Page>
  );
}
