# 麻麻的将管理端

管理端使用 Vue 3、TypeScript、PrimeVue 4.5.5、Aura 主题和 PrimeFlex。

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

访问 `http://127.0.0.1:5173/admin/`。Vite 会把 `/api` 和
`/user-assets` 代理到 `127.0.0.1:8080`。

## 检查

```bash
npm run typecheck
npm test
npm run build
```

## Docker

管理端为可选独立镜像，通过同源代理访问服务端：

```bash
docker build --target admin-web -t mamahjong-admin-web:local .
docker compose --profile admin up --detach admin-web
```

Compose 环境访问 `http://127.0.0.1:8080/admin/`，由主 Web 镜像转发到
不暴露宿主机端口的管理端容器。通过
`MAMAHJONG_ADMIN_SERVER_URL` 设置服务端地址，`MAMAHJONG_ADMIN_GAME_WEB_URL`
设置素材来源；数据库地址仍由服务端的
`MAMAHJONG_DATABASE_URL` 管理，浏览器不直接连接数据库。

资源库中的文件由服务端管理，保存在 `user-assets` 命名卷中。server 以读写方式挂载，
web 和 admin-web 以只读方式挂载；单文件上传上限为 50 MB。
