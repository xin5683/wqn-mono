# wqn

(WQN) 的后端服务：Rust + Axum 实现的 REST API，提供用户认证、错题/学科/标签管理、本地文件存储与 OpenAPI 文档。

## 技术栈

- Rust 2024 edition / Axum 0.8（需 rustc ≥ 1.85）
- SQLx 0.8（PostgreSQL + rustls）—— 迁移在**编译期嵌入**二进制，启动时自动执行
- JWT 会话认证（httpOnly cookie）+ Argon2 密码哈希
- utoipa / utoipa-swagger-ui（OpenAPI 文档）

## 目录结构

```
src/                 # Axum 路由 / 服务 / 中间件
migrations/          # SQLx 迁移（编译期嵌入，无需单独挂载）
scripts/             # 烟雾测试、存储备份/清理脚本
docs/                # local-storage-operations.md
Dockerfile           # musl 静态构建 → Alpine 运行镜像
docker-compose.yml   # Postgres + backend，创建共享网络 wqn-net
.env.example         # 环境变量模板
```

## 前置条件

- Docker + Docker Compose（推荐）
- 或本地：Rust ≥ 1.85、PostgreSQL 16+

## 用 Docker Compose 构建运行（推荐）

```bash
cp .env.example .env
# 生成 JWT 密钥并填入 .env 的 AUTH_JWT_SECRET
openssl rand -base64 48
docker compose up --build -d
```

启动后：

| 地址 | 说明 |
| --- | --- |
| `http://localhost:${BACKEND_PORT:-8080}/api` | API 根路径 |
| `http://localhost:${BACKEND_PORT:-8080}/swagger-ui` | Swagger UI |
| `http://localhost:${BACKEND_PORT:-8080}/api-docs/openapi.json` | OpenAPI JSON |
| `http://localhost:${BACKEND_PORT:-8080}/healthz` | 存活探针 |

数据库迁移由 `sqlx::migrate!()` 嵌入二进制，启动时自动执行 —— **构建镜像不需要 `DATABASE_URL`，也不需要挂载 migrations 目录**。

## 手动构建镜像

```bash
docker build -t wqn-backend .
docker run --rm -p 8080:8080 --env-file .env wqn-backend
```

## 本地调试（清理重建）

`scripts/docker-dev.sh` 在一条命令里完成「停止并删除旧的后端容器 → 删除旧镜像 → 重新构建 → 运行」，
适合迭代调试 Rust 代码。配置与 `docker-compose.yml` / 当前运行的 `wqn-backend` 容器一致（网络 `wqn-net`、
存储卷、`DATABASE_URL` 指向 `postgres`、对外端口 `${BACKEND_PORT}`）。Postgres 及其数据卷在多次 `up`
之间会被保留。

```bash
scripts/docker-dev.sh          # 默认 = up：清理重建并运行
scripts/docker-dev.sh logs     # 跟随后端日志
scripts/docker-dev.sh status  # 查看 postgres + backend + 镜像状态
scripts/docker-dev.sh stop     # 停止并删除后端容器（保留镜像与 postgres）
scripts/docker-dev.sh clean    # 停止并删除后端容器与镜像（clean -v 连同 postgres 与卷）
```

> 与 `docker compose up --build` 的区别：本脚本会删除旧的后端镜像，避免调试时悬空镜像堆积。

## 本地直接运行（无 Docker）

需要先运行一个 PostgreSQL，并设置 `DATABASE_URL`（程序启动时通过 `dotenvy` 自动加载 `.env`）：

```bash
export DATABASE_URL="postgres://wqn:wqn_password@127.0.0.1:5432/wqn"
cargo run --release
```

## 超管账号（CLI）

无需直连数据库，用 `admin` 子命令创建/重置超级管理员（二进制运行名 `wqn-backend`）：

```bash
# 创建超管（交互式输入两次密码，不回显）
podman exec -it wqn-backend wqn-backend admin create --email hstar@live.cn
# 重置忘记密码的超管（令所有现有会话立即失效）
podman exec -it wqn-backend wqn-backend admin reset-password --email admin@example.com
```

本地运行：`cargo run -- admin create --email admin@example.com`。

- `create` 幂等：邮箱不存在则新建超管；已存在普通用户则提权；已是超管则跳过。
- 密码仅在新建账号时询问；自动化用 `--password-stdin` 从 stdin 读密码（跳过确认）。
- 需与服务相同的 `.env`（`DATABASE_URL` 等），迁移自动执行。

## 环境变量

完整列表见 `.env.example`，关键项：

| 变量 | 默认 | 说明 |
| --- | --- | --- |
| `AUTH_JWT_SECRET` | （必填） | JWT 签名密钥，用 `openssl rand -base64 48` 生成 |
| `DATABASE_URL` | — | Postgres 连接串（compose 中自动拼接，本地运行需手动设置） |
| `BACKEND_PORT` | 8080 | 对外暴露的 API 端口 |
| `POSTGRES_PORT` | 5432 | 对外暴露的 Postgres 端口 |
| `CORS_ALLOWED_ORIGINS` | `http://localhost:3000,http://127.0.0.1:3000` | 允许的浏览器来源 |
| `AUTH_COOKIE_SECURE` | false | 前端走 HTTPS 时设为 true |
| `AUTH_SESSION_TTL_SECONDS` | 2592000 | 会话有效期（秒，默认 30 天） |
| `DATABASE_MAX_CONNECTIONS` | 10 | 连接池大小 |

## 共享网络

`docker-compose.yml` 创建名为 `wqn-net` 的 bridge 网络。前端栈（`../Wrong-Question-Notebook/web`）以外部网络方式加入，通过 `backend:8080` 访问本服务。

## 国内网络加速

Dockerfile 已配置国内镜像源以加速构建：

- Alpine `apk` → TUNA 镜像（构建参数 `APK_MIRROR`，置空可禁用）
- crates.io → TUNA sparse 镜像（`.cargo/config.toml`）
- Swagger UI 资源 → 通过 `gh-proxy.com` 代理下载
- `ring`/rustls 需要 perl + C 工具链（构建镜像已安装）
