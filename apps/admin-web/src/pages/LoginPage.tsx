import { LoginFormPage } from "@ant-design/pro-components/es/form/layouts/LoginFormPage";
import ProFormText from "@ant-design/pro-components/es/form/components/Text";
import type { UseQueryResult } from "@tanstack/react-query";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Alert, Result } from "antd";
import { adminApi } from "../api";
import type { AdminIdentity } from "../types";

interface LoginValues {
  loginName: string;
  password: string;
}

interface Props {
  identityQuery: UseQueryResult<AdminIdentity, Error>;
}

export function LoginPage({ identityQuery }: Props) {
  const queryClient = useQueryClient();
  const bootstrap = useQuery({
    queryKey: ["admin", "bootstrap"],
    queryFn: adminApi.bootstrap,
    retry: false,
  });
  const login = useMutation({
    mutationFn: ({ loginName, password }: LoginValues) =>
      adminApi.login(
        loginName,
        password,
        bootstrap.data?.login_csrf ?? "",
      ),
    onSuccess: async (identity) => {
      queryClient.setQueryData(["admin", "identity"], identity);
      await identityQuery.refetch();
    },
  });

  if (bootstrap.isLoading) {
    return null;
  }
  if (bootstrap.isError) {
    return <Result status="error" title="管理端不可用" />;
  }
  if (!bootstrap.data?.enabled) {
    return <Result status="warning" title="管理端未启用" />;
  }
  return (
    <LoginFormPage<LoginValues>
      title="管理员登录"
      subTitle={false}
      activityConfig={{
        title: "麻麻的将",
        subTitle: "管理后台",
      }}
      style={{ minHeight: "100dvh" }}
      message={
        login.error ? (
          <Alert type="error" showIcon message={login.error.message} />
        ) : false
      }
      submitter={{
        searchConfig: { submitText: "登录" },
        submitButtonProps: { loading: login.isPending, block: true },
      }}
      onFinish={async (values) => {
        await login.mutateAsync(values);
        return true;
      }}
    >
      <ProFormText
        name="loginName"
        label="账号"
        placeholder="请输入账号"
        fieldProps={{ autoComplete: "username", autoFocus: true }}
        rules={[{ required: true, message: "请输入账号" }]}
      />
      <ProFormText.Password
        name="password"
        label="密码"
        placeholder="请输入密码"
        fieldProps={{ autoComplete: "current-password" }}
        rules={[{ required: true, message: "请输入密码" }]}
      />
    </LoginFormPage>
  );
}
