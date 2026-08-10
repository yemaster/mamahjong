import {
  DeleteOutlined,
  EditOutlined,
  PlusOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  App,
  Button,
  Card,
  Empty,
  Form,
  Image,
  Input,
  Modal,
  Popconfirm,
  Space,
  Switch,
  Tag,
} from "antd";
import { useState } from "react";
import { adminApi } from "../api";
import { Page } from "../components/Page";
import type {
  AdminIdentity,
  AdminTablecloth,
  TableclothInput,
} from "../types";

const emptyTablecloth: TableclothInput = {
  id: "",
  name: "",
  texture_path: "",
  enabled: true,
  is_default: false,
};

export function TableclothsPage({ identity }: { identity: AdminIdentity }) {
  const { message } = App.useApp();
  const queryClient = useQueryClient();
  const [form] = Form.useForm<TableclothInput>();
  const [editing, setEditing] = useState<AdminTablecloth | null>();
  const query = useQuery({
    queryKey: ["admin", "tablecloths"],
    queryFn: adminApi.tablecloths,
  });
  const save = useMutation({
    mutationFn: (tablecloth: TableclothInput) =>
      editing
        ? adminApi.updateTablecloth(tablecloth, identity.csrf_token)
        : adminApi.createTablecloth(tablecloth, identity.csrf_token),
    onSuccess: async () => {
      message.success(editing ? "桌布资料已更新" : "桌布已添加");
      setEditing(undefined);
      await queryClient.invalidateQueries({ queryKey: ["admin"] });
    },
  });
  const remove = useMutation({
    mutationFn: (id: string) =>
      adminApi.deleteTablecloth(id, identity.csrf_token),
    onSuccess: async () => {
      message.success("桌布已删除");
      await queryClient.invalidateQueries({ queryKey: ["admin"] });
    },
  });

  const openCreate = () => {
    setEditing(null);
    form.setFieldsValue(emptyTablecloth);
  };
  const openEdit = (tablecloth: AdminTablecloth) => {
    setEditing(tablecloth);
    form.setFieldsValue(tablecloth);
  };
  const submit = async () => {
    await save.mutateAsync(await form.validateFields());
  };

  return (
    <Page
      title="桌布"
      error={query.error ?? save.error ?? remove.error}
      extra={
        <Space>
          <Button
            aria-label="刷新桌布"
            icon={<ReloadOutlined />}
            onClick={() => query.refetch()}
          />
          <Button type="primary" icon={<PlusOutlined />} onClick={openCreate}>
            添加桌布
          </Button>
        </Space>
      }
    >
      {query.data?.tablecloths.length ? (
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fill, minmax(250px, 1fr))",
            gap: 16,
          }}
        >
          {query.data.tablecloths.map((tablecloth) => (
            <Card
              key={tablecloth.id}
              cover={
                <Image
                  src={tablecloth.texture_path}
                  alt={tablecloth.name}
                  preview={false}
                  style={{ width: "100%", aspectRatio: 1, objectFit: "cover" }}
                />
              }
              actions={[
                <Button
                  key="edit"
                  type="text"
                  icon={<EditOutlined />}
                  onClick={() => openEdit(tablecloth)}
                >
                  编辑
                </Button>,
                <Popconfirm
                  key="delete"
                  title="删除此桌布？"
                  disabled={tablecloth.is_default}
                  onConfirm={() => remove.mutate(tablecloth.id)}
                >
                  <Button
                    type="text"
                    danger
                    disabled={tablecloth.is_default}
                    icon={<DeleteOutlined />}
                  >
                    删除
                  </Button>
                </Popconfirm>,
              ]}
            >
              <Card.Meta
                title={
                  <Space>
                    {tablecloth.name}
                    {tablecloth.is_default ? <Tag color="green">初始</Tag> : null}
                    {!tablecloth.enabled ? <Tag>停用</Tag> : null}
                  </Space>
                }
                description={tablecloth.id}
              />
            </Card>
          ))}
        </div>
      ) : (
        <Empty description="暂无桌布" />
      )}

      <Modal
        title={editing ? `编辑${editing.name}` : "添加桌布"}
        open={editing !== undefined}
        confirmLoading={save.isPending}
        onCancel={() => !save.isPending && setEditing(undefined)}
        onOk={() => void submit()}
        okText="保存"
        cancelText="取消"
        destroyOnHidden
      >
        <Form form={form} layout="vertical" preserve={false}>
          <Form.Item
            name="id"
            label="桌布编号"
            rules={[{ required: true, message: "请输入桌布编号" }]}
          >
            <Input disabled={Boolean(editing)} placeholder="例如 peacock-green" />
          </Form.Item>
          <Form.Item
            name="name"
            label="桌布名称"
            rules={[{ required: true, message: "请输入桌布名称" }]}
          >
            <Input />
          </Form.Item>
          <Form.Item
            name="texture_path"
            label="本地纹理路径"
            rules={[{ required: true, message: "请输入本地纹理路径" }]}
          >
            <Input placeholder="/game/assets/local-game-assets/..." />
          </Form.Item>
          <Space size="large">
            <Form.Item name="enabled" label="启用" valuePropName="checked">
              <Switch />
            </Form.Item>
            <Form.Item name="is_default" label="设为初始桌布" valuePropName="checked">
              <Switch />
            </Form.Item>
          </Space>
        </Form>
      </Modal>
    </Page>
  );
}
