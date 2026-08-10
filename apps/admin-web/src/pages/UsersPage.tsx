import { ReloadOutlined } from "@ant-design/icons";
import { ProCard } from "@ant-design/pro-components/es/card";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  App,
  Button,
  Input,
  Space,
  Table,
  Tag,
  type TableColumnsType,
} from "antd";
import { useMemo, useState } from "react";
import { adminApi } from "../api";
import { Page } from "../components/Page";
import type { AccountStatus, AdminIdentity, AdminUser } from "../types";

export function UsersPage({ identity }: { identity: AdminIdentity }) {
  const { modal, message } = App.useApp();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState("");
  const query = useQuery({
    queryKey: ["admin", "users"],
    queryFn: adminApi.users,
  });
  const update = useMutation({
    mutationFn: ({
      userId,
      status,
    }: {
      userId: string;
      status: AccountStatus;
    }) => adminApi.updateUserStatus(userId, status, identity.csrf_token),
    onSuccess: async () => {
      message.success("账号状态已更新");
      await queryClient.invalidateQueries({ queryKey: ["admin"] });
    },
  });
  const users = useMemo(() => {
    const keyword = search.trim().toLocaleLowerCase();
    return (query.data?.users ?? []).filter(
      (user) =>
        !keyword ||
        user.login_name.toLocaleLowerCase().includes(keyword) ||
        user.nickname.toLocaleLowerCase().includes(keyword),
    );
  }, [query.data, search]);

  const confirm = (user: AdminUser) => {
    const next = user.status === "active" ? "suspended" : "active";
    modal.confirm({
      title: next === "suspended" ? "停用账号" : "恢复账号",
      content: user.nickname,
      okButtonProps: { danger: next === "suspended" },
      onOk: () => update.mutateAsync({ userId: user.id, status: next }),
    });
  };

  const columns: TableColumnsType<AdminUser> = [
    { title: "昵称", dataIndex: "nickname", width: 180 },
    { title: "账号", dataIndex: "login_name", width: 200 },
    {
      title: "类型",
      dataIndex: "role",
      width: 110,
      render: (value: string) =>
        value === "administrator" ? "管理员" : "玩家",
    },
    {
      title: "状态",
      dataIndex: "status",
      width: 110,
      render: (_, user) => (
        <Tag color={user.status === "active" ? "success" : "default"}>
          {user.status === "active" ? "正常" : "已停用"}
        </Tag>
      ),
    },
    {
      title: "操作",
      width: 100,
      render: (_, user) => (
        <Button
          type="link"
          size="small"
          danger={user.status === "active"}
          disabled={user.id === identity.id}
          loading={update.isPending}
          onClick={() => confirm(user)}
        >
          {user.status === "active" ? "停用" : "恢复"}
        </Button>
      ),
    },
  ];

  return (
    <Page title="用户" error={query.error}>
      <ProCard
        title={`全部用户（${users.length}）`}
        variant="outlined"
        extra={
          <Space wrap>
            <Input.Search
              allowClear
              placeholder="搜索昵称或账号"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
            <Button
              aria-label="刷新用户"
              icon={<ReloadOutlined />}
              onClick={() => query.refetch()}
            />
          </Space>
        }
      >
        <Table<AdminUser>
          rowKey="id"
          loading={query.isLoading}
          dataSource={users}
          columns={columns}
          scroll={{ x: 760 }}
          pagination={{ pageSize: 10, showSizeChanger: true }}
        />
      </ProCard>
    </Page>
  );
}
