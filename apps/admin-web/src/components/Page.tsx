import { PageContainer } from "@ant-design/pro-components/es/layout";
import { Alert, Flex } from "antd";
import type { ReactNode } from "react";

interface Props {
  title: string;
  error?: Error | null;
  extra?: ReactNode;
  children: ReactNode;
}

export function Page({ title, error, extra, children }: Props) {
  return (
    <PageContainer title={title} extra={extra}>
      <Flex vertical gap="large">
        {error ? <Alert type="error" showIcon message={error.message} /> : null}
        {children}
      </Flex>
    </PageContainer>
  );
}
