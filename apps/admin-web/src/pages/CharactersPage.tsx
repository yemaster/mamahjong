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
  Collapse,
  Empty,
  Flex,
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
  AdminCharacter,
  AdminIdentity,
  CharacterInput,
} from "../types";

const emptyCharacter: CharacterInput = {
  id: "",
  name: "",
  illustration_path: "",
  emotes: [],
  voices: [],
  outfits: [
    {
      id: "default",
      name: "初始装扮",
      illustration_path: "",
    },
  ],
  enabled: true,
  is_default: false,
};

export function CharactersPage({ identity }: { identity: AdminIdentity }) {
  const { message } = App.useApp();
  const queryClient = useQueryClient();
  const [form] = Form.useForm<CharacterInput>();
  const [editing, setEditing] = useState<AdminCharacter | null>();
  const query = useQuery({
    queryKey: ["admin", "characters"],
    queryFn: adminApi.characters,
  });
  const save = useMutation({
    mutationFn: (character: CharacterInput) =>
      editing
        ? adminApi.updateCharacter(character, identity.csrf_token)
        : adminApi.createCharacter(character, identity.csrf_token),
    onSuccess: async () => {
      message.success(editing ? "角色资料已更新" : "角色已添加");
      setEditing(undefined);
      await queryClient.invalidateQueries({ queryKey: ["admin"] });
    },
  });
  const remove = useMutation({
    mutationFn: (characterId: string) =>
      adminApi.deleteCharacter(characterId, identity.csrf_token),
    onSuccess: async () => {
      message.success("角色已删除");
      await queryClient.invalidateQueries({ queryKey: ["admin"] });
    },
  });

  const openCreate = () => {
    setEditing(null);
    form.setFieldsValue(emptyCharacter);
  };
  const openEdit = (character: AdminCharacter) => {
    setEditing(character);
    form.setFieldsValue(character);
  };
  const closeEditor = () => {
    if (!save.isPending) setEditing(undefined);
  };
  const submit = async () => {
    const values = await form.validateFields();
    await save.mutateAsync(values);
  };

  return (
    <Page
      title="角色"
      error={query.error ?? save.error ?? remove.error}
      extra={
        <Space>
          <Button
            aria-label="刷新角色"
            icon={<ReloadOutlined />}
            onClick={() => query.refetch()}
          />
          <Button type="primary" icon={<PlusOutlined />} onClick={openCreate}>
            添加角色
          </Button>
        </Space>
      }
    >
      {query.data?.characters.length ? (
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))",
            gap: 16,
          }}
        >
          {query.data.characters.map((character) => (
            <Card
              key={character.id}
              cover={
                <div
                  style={{
                    height: 320,
                    display: "grid",
                    placeItems: "end center",
                    overflow: "hidden",
                    background: "#f5f5f5",
                  }}
                >
                  <Image
                    src={character.illustration_path}
                    alt={character.name}
                    preview={false}
                    style={{ maxHeight: 310, objectFit: "contain" }}
                  />
                </div>
              }
              actions={[
                <Button
                  key="edit"
                  type="text"
                  icon={<EditOutlined />}
                  onClick={() => openEdit(character)}
                >
                  编辑
                </Button>,
                <Popconfirm
                  key="delete"
                  title="删除此角色？"
                  description={
                    character.is_default ? "初始角色不能删除" : character.name
                  }
                  disabled={character.is_default}
                  onConfirm={() => remove.mutate(character.id)}
                >
                  <Button
                    type="text"
                    danger
                    disabled={character.is_default}
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
                    {character.name}
                    {character.is_default ? <Tag color="green">初始</Tag> : null}
                    {!character.enabled ? <Tag>停用</Tag> : null}
                  </Space>
                }
                description={`${character.outfits.length} 套装扮 · ${character.emotes.length} 个表情 · ${character.voices.length} 条语音`}
              />
            </Card>
          ))}
        </div>
      ) : (
        <Empty description="暂无角色" />
      )}

      <Modal
        title={editing ? `编辑${editing.name}` : "添加角色"}
        open={editing !== undefined}
        width={760}
        confirmLoading={save.isPending}
        onCancel={closeEditor}
        onOk={() => void submit()}
        okText="保存"
        cancelText="取消"
        destroyOnHidden
      >
        <Form
          form={form}
          layout="vertical"
          initialValues={emptyCharacter}
          preserve={false}
        >
          <Flex gap="middle">
            <Form.Item
              name="id"
              label="角色编号"
              rules={[{ required: true, message: "请输入角色编号" }]}
              style={{ flex: 1 }}
            >
              <Input disabled={Boolean(editing)} placeholder="例如 ichihime" />
            </Form.Item>
            <Form.Item
              name="name"
              label="角色名"
              rules={[{ required: true, message: "请输入角色名" }]}
              style={{ flex: 1 }}
            >
              <Input />
            </Form.Item>
          </Flex>
          <Form.Item
            name="illustration_path"
            label="当前立绘图片"
            rules={[{ required: true, message: "请输入本地图片路径" }]}
          >
            <Input placeholder="/game/assets/characters/..." />
          </Form.Item>
          <Space size="large">
            <Form.Item name="enabled" label="启用" valuePropName="checked">
              <Switch />
            </Form.Item>
            <Form.Item name="is_default" label="设为初始角色" valuePropName="checked">
              <Switch />
            </Form.Item>
          </Space>

          <Collapse
            items={[
              {
                key: "outfits",
                label: "角色换装",
                children: (
                  <AssetList
                    name="outfits"
                    fields={[
                      { name: "id", placeholder: "装扮编号" },
                      { name: "name", placeholder: "装扮名称" },
                      {
                        name: "illustration_path",
                        placeholder: "立绘图片路径",
                        wide: true,
                      },
                    ]}
                  />
                ),
              },
              {
                key: "emotes",
                label: "角色表情",
                children: (
                  <AssetList
                    name="emotes"
                    fields={[
                      { name: "name", placeholder: "表情名称" },
                      { name: "path", placeholder: "图片路径", wide: true },
                    ]}
                  />
                ),
              },
              {
                key: "voices",
                label: "角色语音",
                children: (
                  <AssetList
                    name="voices"
                    fields={[
                      { name: "name", placeholder: "语音名称" },
                      { name: "path", placeholder: "音频路径", wide: true },
                    ]}
                  />
                ),
              },
            ]}
          />
        </Form>
      </Modal>
    </Page>
  );
}

function AssetList({
  name,
  fields,
}: {
  name: "outfits" | "emotes" | "voices";
  fields: { name: string; placeholder: string; wide?: boolean }[];
}) {
  return (
    <Form.List name={name}>
      {(formFields, { add, remove }) => (
        <Flex vertical gap="small">
          {formFields.map((field) => (
            <Flex key={field.key} gap="small" align="start">
              {fields.map((item) => (
                <Form.Item
                  key={item.name}
                  name={[field.name, item.name]}
                  rules={[{ required: true, message: "请填写" }]}
                  style={{ flex: item.wide ? 2 : 1, marginBottom: 0 }}
                >
                  <Input placeholder={item.placeholder} />
                </Form.Item>
              ))}
              <Button
                danger
                type="text"
                icon={<DeleteOutlined />}
                aria-label="删除"
                onClick={() => remove(field.name)}
              />
            </Flex>
          ))}
          <Button type="dashed" icon={<PlusOutlined />} onClick={() => add()}>
            添加
          </Button>
        </Flex>
      )}
    </Form.List>
  );
}
