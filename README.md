# AI 日程引擎（AI Schedule Engine）

本地 SQLite + 多厂商 LLM 的桌面日程应用（Tauri 2 + React）。API 密钥走系统钥匙串，请求由 Rust 直连。

---

## 新手：从 GitHub「一键」安装（推荐）

适合完全不想碰命令行的用户，只需要浏览器和鼠标。

### 1. 打开本项目的发行版页面

在浏览器地址栏访问（把用户名换成你的仓库位置即可）：

`https://github.com/changshenhan/ai-schedule-engine/releases`

### 2. 下载安装包

- 在 **Latest**（最新）版本里找到 **Assets（资源文件）**
- **Mac（苹果电脑）**：下载以 **`.dmg`** 结尾的文件（例如 `AI日程引擎_0.2.0_aarch64.dmg`）
- 双击 DMG，把应用拖到 **应用程序** 文件夹即可

### 3. 第一次打开（若系统提示「无法验证开发者」）

**系统设置 → 隐私与安全性** 里选择 **仍要打开**，或在应用图标上 **右键 → 打开**。

---

## 仓库与维护者

- 建议仓库名：`changshenhan/ai-schedule-engine`

### 首次把本项目推到你的 GitHub（维护者操作）

1. 在 GitHub 网页新建仓库：**New repository** → 名称填 `ai-schedule-engine` → **不要**勾选添加 README（本地已有）→ Create。
2. 在本项目目录执行（HTTPS 示例，也可换成 SSH）：

```bash
cd /Users/songlvhan/Desktop/AI-003
git remote add origin https://github.com/changshenhan/ai-schedule-engine.git
git push -u origin main
```

若 GitHub 提示登录，按浏览器或 Personal Access Token 提示完成授权。

### 发布新版本安装包（自动生成 Releases）

推送以 `v` 开头的标签后，Actions 会在云端构建 **macOS `.dmg`** 并挂到 Releases：

```bash
git tag v0.2.0
git push origin v0.2.0
```

首次发版前请确认：**Settings → Actions → General** 中 Workflow 权限允许读写仓库内容。

---

## 开发者本地运行

需要安装 [Node.js](https://nodejs.org/) 与 [Rust](https://rustup.rs/)。

```bash
npm ci
npm run tauri dev
```

打包：

```bash
npm run tauri build
```

---

## 技术栈

- 前端：Vite、React 19、Tailwind CSS 4、Zustand  
- 桌面：Tauri 2、Rust、SQLite（rusqlite）

---

## 许可证

MIT License — 见 [LICENSE](./LICENSE)。
