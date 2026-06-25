# Android 打包（TWA 套壳）

把已部署的 WQN 站点用 **Trusted Web Activity (TWA)** 套成 Android App。外壳由 [Bubblewrap](https://github.com/GoogleChromeLabs/bubblewrap) 生成，套的是线上 Next.js 站点本身——前端代码零改动，所有数据/鉴权仍走远程 Rust + Postgres 后端。

> 前置已完成：`frontend/web/public/manifest.webmanifest` + `public/icons/{icon-192,icon-512,maskable-512}.png` + `app/layout.tsx` 里挂了 `manifest` / `appleWebApp`。Bubblewrap 靠这份 manifest 生成工程。

## 0. 前置条件

1. **站点已部署到 HTTPS 域名**：前端（Next.js）+ 后端（Rust Axum）+ Postgres 都跑在线上（`docker-compose` 或 Vercel）。TWA 只能套线上 HTTPS 站点，不能套 `localhost`。
2. **JDK 17**（本机已装 Temurin 17 ✓）。
3. **Android SDK**（本机在 `~/Android/Sdk` ✓，但环境变量没设，必须导出）：
   ```bash
   export ANDROID_HOME="$HOME/Android/Sdk"
   export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"
   ```
   写进 `~/.bashrc` 持久化。Bubblewrap 首次运行会按需补装 `platforms;android-34`、`build-tools` 等。
4. Node 24（本机 ✓，Bubblewrap 用 npx 跑）。

## 1. 生成 Android 工程

```bash
cd twa
npx @bubblewrap/cli init --manifest https://YOUR_DOMAIN/manifest.webmanifest
```

交互问的几项（参考值）：
- Application name: `Wrong Question Notebook`
- Short name: `错题本`
- Application ID: `com.wqn.app`（你的包名，Play 上架后不可改）
- Theme color: `#FF5300`（与 manifest `theme_color` 一致）
- Nav color / background color: `#FFFFFF`
- Start URL: 默认 `/`（**见下方「start_url 注意」**）
- Icon maskable: yes（用 `maskable-512.png`）
- Signing key: 选 “Create a new digital signing key”，设个 keystore 密码

生成后 `twa/` 下会出现完整 Gradle 工程（`build.gradle.kts`、`app/`、`twa-manifest.json`）。

## 2. 构建 APK / AAB

```bash
npx @bubblewrap/cli build
```

- 产物：`app/build/outputs/apk/release/app-release.apk`（直接装机测）和 `.aab`（上架 Play 用）。
- 首次构建会在 `android.keystore` 生成签名密钥——**务必备份这个文件和密码**，丢了 Play 上的 App 就再也无法更新。

## 3. 去掉地址栏：部署 Digital Asset Links

TWA 默认顶部带 URL 条；要让 Chrome 认可这个 App “拥有”该域名，得在站点根目录放 `assetlinks.json`：

1. 构建后 Bubblewrap 会打印出对应的 `assetlinks.json` 内容（含你签名 key 的 SHA256），也可以用：
   ```bash
   npx @bubblewrap/cli assetlinks
   ```
2. 把内容存为 `frontend/web/public/.well-known/assetlinks.json`，随前端一起部署——它会被 Next 当静态文件返回到 `https://YOUR_DOMAIN/.well-known/assetlinks.json`。
   - 本仓库 `twa/assetlinks.template.json` 是结构参考，真实指纹由上面命令生成，**别手填**。
3. 等 CDN/浏览器缓存刷新后重装 App，URL 条消失、全屏体验即生效。
   - 校验：浏览器开 `https://YOUR_DOMAIN/.well-known/assetlinks.json` 能直接拿到 JSON（不能被鉴权重定向到登录页——本项目目前没有活跃 middleware，静态文件直出，满足要求）。

## 4. start_url 注意

manifest 里 `start_url: "/"`。本项目用 `[locale]` 路由（URL 是 `/en`、`/zh-CN`），`/`→`/en` 的跳转靠 next-intl middleware。**当前 `frontend/web/proxy.ts` 没有作为 middleware 接入**（无 `middleware.ts` 引用它），所以 `/` 可能不跳转。两种处理：

- 若线上 `/` 能正确落到某个 locale → 保持 `/`。
- 若 `/` 404 → 把 `public/manifest.webmanifest` 的 `start_url` 改成 `/en`，或在 `frontend/web/` 加一个标准 `middleware.ts` 把 `proxy.ts` 接上（属于路由修复，不在套壳任务内）。

## 5. 更新与迭代

- 纯前端改动（文案、页面、样式）：**不用动 Android 工程**，部署站点即可，App 重启就生效。
- 改 manifest（名字/图标/包名/主题色）才需要重跑 `npx @bubblewrap/cli update` + `build`，再用同一 keystore 签名上新版本（Play 要求 versionCode 递增）。

## 6. 上架 Google Play

1. 用 `.aab`（不是 apk）上传到 Play Console。
2. Play 会用自己的 app signing key 重签；首次上传后到 Console 「App signing」拿到 Play 的 SHA256，**用它再生成一份 `assetlinks.json` 部署到站点**（否则从 Play 安装的版本会重新出现 URL 条）。开发自签 keystore 的 assetlinks 用于内测包。
3. 截图、隐私政策（项目已有 `/privacy` 页）按 Console 要求提交。

## ⚠️ 打包前置阻塞项（路由修复，独立 task）

实际跑 `bubblewrap init`/`build` 前，必须先解决以下两点，否则 TWA 首屏/校验会出问题。**属路由修复，不在移动端适配任务内**：

1. **`start_url:"/"` 不跳 locale**：`frontend/web/proxy.ts` 导出的是 `proxy`（非 `middleware` default），仓库无 `middleware.ts` 引用，next-intl middleware 实际未运行。`/` 会渲染根 `app/page.tsx`（landing），**不 404**，但不会自动跳 `/en`。TWA 启动首屏是 landing 而非 locale app。推荐：新建 `frontend/web/middleware.ts` 接上 `proxy.ts`（`export { proxy as default, config } from './proxy'`）。
2. **`assetlinks.json` 静态直出**：当前 proxy 死代码状态下，`public/.well-known/assetlinks.json` 被当静态文件直出，TWA 校验畅通。**但接上 middleware 后**，`proxy.ts` 的 matcher 未排除 `.json`/`/.well-known/*`，且 `lib/proxy-paths.ts` 的 `isPublicContentPath` 白名单不含 `/.well-known`，未登录访问会被 302 到登录页，**破坏 TWA 校验**。接 middleware 时必须把 `/.well-known/*` 加入 matcher 排除或 `publicPathPrefixes` 白名单。

另外，`assetlinks.json` 本身**必须由 `npx @bubblewrap/cli assetlinks`（`build` 之后）生成**，内含签名 key 的 SHA256，**不可手填**——`twa/assetlinks.template.json` 仅作结构参考。

## 工具链现状（本机已具备）

| 依赖 | 状态 |
| ---- | ---- |
| JDK 17 (Temurin) | ✅ |
| Android SDK (`~/Android/Sdk`) | ✅（需 `export ANDROID_HOME`） |
| Node 24 / npm 11 | ✅ |
| inkscape + ImageMagick | ✅（已用来生成图标） |
| Bubblewrap CLI | 用 `npx @bubblewrap/cli` 按需跑 |
