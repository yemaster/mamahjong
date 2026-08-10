import {
  AppstoreOutlined,
  AuditOutlined,
  DashboardOutlined,
  HomeOutlined,
  LogoutOutlined,
  ProductOutlined,
  TeamOutlined,
  UserOutlined,
  IdcardOutlined,
  PictureOutlined,
} from "@ant-design/icons";
import { ProLayout } from "@ant-design/pro-components/es/layout";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Button, Tooltip } from "antd";
import type { ReactNode } from "react";
import { adminApi } from "../api";
import type { AdminRoute } from "../routing";
import type { AdminIdentity } from "../types";

interface Props {
  identity: AdminIdentity;
  route: AdminRoute;
  navigate: (route: AdminRoute) => void;
  children: ReactNode;
}

const adminRoutes = {
  path: "/admin",
  routes: [
    {
      path: "/admin/",
      name: "概览",
      icon: <DashboardOutlined />,
    },
    {
      path: "/admin/operations",
      name: "运营管理",
      icon: <AppstoreOutlined />,
      routes: [
        {
          path: "/admin/users",
          name: "用户",
          icon: <TeamOutlined />,
        },
        {
          path: "/admin/rooms",
          name: "房间",
          icon: <HomeOutlined />,
        },
        {
          path: "/admin/characters",
          name: "角色",
          icon: <IdcardOutlined />,
        },
        {
          path: "/admin/tablecloths",
          name: "桌布",
          icon: <PictureOutlined />,
        },
      ],
    },
    {
      path: "/admin/system",
      name: "系统",
      icon: <AuditOutlined />,
      routes: [
        {
          path: "/admin/audit",
          name: "审计日志",
          icon: <AuditOutlined />,
        },
      ],
    },
  ],
};

function toAdminRoute(path?: string): AdminRoute | undefined {
  const route = path?.replace(/^\/admin/, "") || "/";
  return ["/", "/users", "/rooms", "/characters", "/tablecloths", "/audit"].includes(route)
    ? (route as AdminRoute)
    : undefined;
}

export function AdminLayout({ identity, route, navigate, children }: Props) {
  const queryClient = useQueryClient();
  const logout = useMutation({
    mutationFn: () => adminApi.logout(identity.csrf_token),
    onSettled: async () => {
      await queryClient.resetQueries({ queryKey: ["admin"] });
      navigate("/login");
    },
  });

  const pathname = `/admin${route === "/" ? "/" : route}`;

  return (
    <ProLayout
      title="麻麻的将"
      logo={<ProductOutlined />}
      route={adminRoutes}
      location={{ pathname }}
      layout="side"
      navTheme="light"
      contentWidth="Fluid"
      fixedHeader
      fixSiderbar
      siderWidth={232}
      siderMenuType="group"
      locale="zh-CN"
      footerRender={false}
      onMenuHeaderClick={() => navigate("/")}
      menuItemRender={(item, defaultDom) => {
        const target = toAdminRoute(item.path);
        if (!target) {
          return defaultDom;
        }
        return (
          <a
            href={item.path}
            onClick={(event) => {
              event.preventDefault();
              navigate(target);
            }}
          >
            {defaultDom}
          </a>
        );
      }}
      avatarProps={{
        icon: <UserOutlined />,
        title: identity.nickname,
        size: "small",
      }}
      actionsRender={() => [
        <Tooltip title="退出登录" key="logout">
          <Button
            type="text"
            aria-label="退出登录"
            icon={<LogoutOutlined />}
            loading={logout.isPending}
            onClick={() => logout.mutate()}
          />
        </Tooltip>,
      ]}
    >
      {children}
    </ProLayout>
  );
}
