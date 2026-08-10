# 麻麻的将管理端

## 开发

先在项目根目录启动服务端并配置管理员密码：

```bash
MAMAHJONG_ADMIN_PASSWORD=change-this-password \
  cargo run -p mamahjong-server --bin mamahjong-server
```

再启动前端：

```bash
npm install
npm run dev
```

访问 `http://127.0.0.1:5173/admin/`。Vite 会把 `/api` 代理到
`127.0.0.1:8080`。

## 检查

```bash
npm run typecheck
npm test
npm run build
```

生产环境由 Docker 构建前端，服务端在 `/admin/` 提供构建结果。
