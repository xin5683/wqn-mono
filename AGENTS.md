# AGENTS.md

给 AI 编码助手的约束规则。动手前先读完本文件；前端 UI 细节另见 `frontend/.claude/CLAUDE.md`。

## 项目概览

WQN（错题本）全栈 monorepo：

- `backend/` — Rust 2024 / Axum 0.8 REST API，负责数据库、鉴权、文件存储、OpenAPI 文档，是**唯一数据源**。
- `frontend/web/` — Next.js 16（App Router）前端，通过 `/api/*` 代理调用后端。
- `postgres` — 共享 `wqn-net` Docker 网络。

根目录一键启动：

```bash
cp .env.example .env   # 填入 AUTH_JWT_SECRET（openssl rand -base64 48）
docker compose up --build -d
```

不要与 `backend/docker-compose.yml`、`frontend/web/docker-compose.yml` 同时运行——它们共用 `wqn-net` 和容器名。

## 后端规则（Rust）

**分层单向，不可破坏：** `routes/` → `services/` → `sqlx(DB)` + `dto`/`models`

- `routes/` 只做参数解析 + 调一个 service + 用 `response::*` 包裹；**不写 SQL**。
- `services/` 负责业务逻辑与查询；**不构造 HTTP/serde 外壳**。
- 所有 handler 返回 `AppResult<Json<response::ApiSuccess<T>>>`，用 `response::success` / `created` / `empty_ok`，不手拼 `{data,success,timestamp}`。
- 错误一律走 `AppError` + `?`；**不要对运行期数据 `unwrap`/`expect`/`panic`**，`sqlx::Error` 已映射为 500 且不向外泄露细节。
- 鉴权用 `AuthUser` extractor，查询一律按 `auth.id` 过滤——**多租户隔离是安全边界**。
- 新迁移：`migrations/YYYYMMDDHHMMSS_<name>.sql`，由 `sqlx::migrate!()` 编译期嵌入，**构建镜像不需要 `DATABASE_URL`、不挂载 migrations 目录**。
- 导入风格：扁平、全限定、分组 `use`，照搬邻文件写法。
- 新 handler 加 `#[utoipa::path(...)]` 并在 `openapi.rs` 注册。

```bash
# 在 backend/
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test            # 需要可连的 Postgres，设置 DATABASE_URL
```

## 前端规则（Next.js / TypeScript）

- **后端是唯一数据/鉴权源**。Supabase 已从代码中彻底移除（0 引用）；`frontend/web/README.md` 里的 Supabase 字样是**过期文档，不要重新引入**或加 `@supabase/*` 依赖。
- 所有请求走 `lib/api/` + `/api/*` 代理，**不要在浏览器直连 `localhost:8080`**。
- TypeScript `strict`，新代码不用 `any`，外部数据用 `zod` 校验。
- 页面保持 server component，交互拆成 `'use client'` 子组件。
- i18n：所有可见文案取自 `messages/<locale>.json`；新增 key 要**同时**更新 `en.json` 和 `zh-CN.json`。
- UI/设计系统遵循 `frontend/.claude/CLAUDE.md`（暖色调、大圆角、必有 `dark:` 配对、复用 `globals.css` 既有 class、lucide 图标）。
- 格式化：Prettier，单引号、2 空格、LF、80 列。

```bash
# 在 frontend/web/
npm run dev          # 开发
npm run check-all    # type-check + lint + format:check
npm run test         # vitest
npm run prepush      # 提交前全套检查
```

## 收尾前

两端的门都要过，**不要在红的状态下提交**：

- 后端：`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
- 前端：`npm run prepush`（最少 `npm run check-all && npm run test`）

## 红线

- **不提交密钥**。`.env`、`.env*.local`、`.claude/settings.local.json` 已在 `.gitignore`；示例文件里 `AUTH_JWT_SECRET` 只能放占位符。
- **不绕过鉴权**、不为了"先跑通"放宽按用户过滤。
- **不改 API 契约却不补齐**：改响应结构/状态码时，`dto/`、`#[utoipa::path]`、前端 `lib/types.ts` 同步更新。
- **不无故加依赖**，尤其不加 `@supabase/*`；后端依赖以 `Cargo.toml` 现有为先。
- **照搬邻文件风格**（注释密度、命名、import 分组），**改动范围限定在任务内**，不顺手重构无关代码。
