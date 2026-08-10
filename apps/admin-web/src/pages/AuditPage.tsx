import { ReloadOutlined } from "@ant-design/icons";
import { ProCard } from "@ant-design/pro-components/es/card";
import { useQuery } from "@tanstack/react-query";
import {
  Button,
  Input,
  Select,
  Space,
  Table,
  Tag,
  type TableColumnsType,
} from "antd";
import { useMemo, useState } from "react";
import { adminApi } from "../api";
import { Page } from "../components/Page";
import {
  actionLabel,
  categoryLabel,
  formatDateTime,
} from "../format";
import type { AuditEvent } from "../types";

export function AuditPage() {
  const [category, setCategory] = useState<string>();
  const [search, setSearch] = useState("");
  const query = useQuery({
    queryKey: ["admin", "audit"],
    queryFn: adminApi.audit,
  });
  const events = useMemo(() => {
    const keyword = search.trim().toLocaleLowerCase();
    return (query.data?.events ?? []).filter(
      (event) =>
        (!category || event.category === category) &&
        (!keyword ||
          event.action.toLocaleLowerCase().includes(keyword) ||
          actionLabel(event.action).toLocaleLowerCase().includes(keyword) ||
          event.detail.toLocaleLowerCase().includes(keyword) ||
          event.target_id?.toLocaleLowerCase().includes(keyword)),
    );
  }, [category, query.data, search]);

  const columns: TableColumnsType<AuditEvent> = [
    {
      title: "序号",
      dataIndex: "sequence",
      width: 90,
    },
    {
      title: "时间",
      dataIndex: "occurred_at",
      width: 180,
      render: (value: string) => formatDateTime(value),
      defaultSortOrder: "descend",
      sorter: (left, right) => left.sequence - right.sequence,
    },
    {
      title: "类别",
      dataIndex: "category",
      width: 100,
      render: (value: string) => categoryLabel(value),
    },
    {
      title: "操作",
      dataIndex: "action",
      width: 160,
      render: (value: string) => actionLabel(value),
    },
    {
      title: "目标",
      dataIndex: "target_id",
      width: 220,
      ellipsis: true,
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
      render: (_, event) => (
        <Tag color={event.outcome === "success" ? "success" : "error"}>
          {event.outcome === "success" ? "成功" : "失败"}
        </Tag>
      ),
    },
  ];

  return (
    <Page title="审计日志" error={query.error}>
      <ProCard
        title={`审计记录（${events.length}）`}
        variant="outlined"
        extra={
          <Space wrap>
            <Select
              allowClear
              placeholder="全部类别"
              value={category}
              onChange={setCategory}
              options={[
                { value: "auth", label: "认证" },
                { value: "room", label: "房间" },
                { value: "matchmaking", label: "匹配" },
                { value: "game", label: "对局" },
                { value: "admin", label: "管理" },
              ]}
            />
            <Input.Search
              allowClear
              placeholder="搜索操作、说明或目标 ID"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
            <Button
              aria-label="刷新审计日志"
              icon={<ReloadOutlined />}
              onClick={() => query.refetch()}
            />
          </Space>
        }
      >
        <Table<AuditEvent>
          rowKey="sequence"
          loading={query.isLoading}
          dataSource={events}
          columns={columns}
          scroll={{ x: 1080 }}
          pagination={{ pageSize: 20, showSizeChanger: true }}
        />
      </ProCard>
    </Page>
  );
}
