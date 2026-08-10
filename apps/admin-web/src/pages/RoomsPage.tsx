import { ReloadOutlined } from "@ant-design/icons";
import { ProCard } from "@ant-design/pro-components/es/card";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  App,
  Button,
  Select,
  Space,
  Table,
  Tag,
  type TableColumnsType,
} from "antd";
import { useMemo, useState } from "react";
import { adminApi } from "../api";
import { Page } from "../components/Page";
import type { AdminIdentity, AdminRoom, RoomLifecycle } from "../types";

const lifecycleText: Record<RoomLifecycle, string> = {
  waiting: "等待中",
  playing: "进行中",
  closed: "已关闭",
};

export function RoomsPage({ identity }: { identity: AdminIdentity }) {
  const { modal, message } = App.useApp();
  const queryClient = useQueryClient();
  const [lifecycle, setLifecycle] = useState<RoomLifecycle | undefined>();
  const query = useQuery({
    queryKey: ["admin", "rooms"],
    queryFn: adminApi.rooms,
  });
  const close = useMutation({
    mutationFn: (roomId: string) =>
      adminApi.closeRoom(roomId, identity.csrf_token),
    onSuccess: async () => {
      message.success("房间已关闭");
      await queryClient.invalidateQueries({ queryKey: ["admin"] });
    },
  });
  const rooms = useMemo(
    () =>
      (query.data?.rooms ?? []).filter(
        (room) => !lifecycle || room.lifecycle === lifecycle,
      ),
    [lifecycle, query.data],
  );
  const confirmClose = (room: AdminRoom) => {
    modal.confirm({
      title: "关闭房间",
      content: room.name,
      okButtonProps: { danger: true },
      onOk: () => close.mutateAsync(room.id),
    });
  };

  const columns: TableColumnsType<AdminRoom> = [
    { title: "房间", dataIndex: "name", ellipsis: true },
    {
      title: "人数",
      width: 100,
      render: (_, room) => `${room.member_count}/${room.seat_count}`,
    },
    {
      title: "可见性",
      dataIndex: "visibility",
      width: 100,
      render: (value: string) => (value === "public" ? "公开" : "私有"),
    },
    {
      title: "状态",
      dataIndex: "lifecycle",
      width: 110,
      render: (_, room) => (
        <Tag color={room.lifecycle === "playing" ? "processing" : "default"}>
          {lifecycleText[room.lifecycle]}
        </Tag>
      ),
    },
    {
      title: "操作",
      width: 100,
      render: (_, room) =>
        room.lifecycle === "waiting" ? (
          <Button
            type="link"
            size="small"
            danger
            loading={close.isPending}
            onClick={() => confirmClose(room)}
          >
            关闭
          </Button>
        ) : null,
    },
  ];

  return (
    <Page title="房间" error={query.error}>
      <ProCard
        title={`全部房间（${rooms.length}）`}
        variant="outlined"
        extra={
          <Space wrap>
            <Select
              allowClear
              placeholder="全部状态"
              value={lifecycle}
              onChange={setLifecycle}
              options={Object.entries(lifecycleText).map(([value, label]) => ({
                value,
                label,
              }))}
            />
            <Button
              aria-label="刷新房间"
              icon={<ReloadOutlined />}
              onClick={() => query.refetch()}
            />
          </Space>
        }
      >
        <Table<AdminRoom>
          rowKey="id"
          loading={query.isLoading}
          dataSource={rooms}
          columns={columns}
          scroll={{ x: 720 }}
          pagination={{ pageSize: 10, showSizeChanger: true }}
        />
      </ProCard>
    </Page>
  );
}
