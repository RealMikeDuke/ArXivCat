# ArxivCat CHANGELOG

## v0.7.1 - Chat Persistence & UI Polish

### v0.7.1 更新内容

- Global Chat 新增 `Select Context`，支持按论文切换 `body` / `appendix` / `description` / `note`
- side chat 与 Global Chat 新增历史会话的自动保存、打开与重命名，并尽量复用已保存的上下文快照
- workspace 根目录新增 `arxivcat_global_chats`，论文目录新增 `arxiv_chats`，并排除内部目录被误识别为论文
- 多处 header / toolbar 与 `Send` / `Stop` / `Reset` 按钮行改为自动换行布局
- 重命名弹窗改为统一主题样式，修复若干 Global Chat 交互与显示问题

---

## v0.7.0 - Description & Global Chat

### 更新内容

- 新增标准论文文件 `description.md`，在单篇下载和 `Download All` 中自动生成
- 新增 `.description_ready` 完成标记，用于识别中途中断导致的不完整 description
- 预览下拉框新增 `description` 视图，可直接查看 `description.md`
- 新增 **Global Chat**，基于当前 workspace 全部论文的 `description.md` 做多轮问答
- Global Chat 与右侧 side chat 共享同一套 chat panel 抽象与交互结构
- Global Chat 现已支持 `Flash` / `Pro` 模型切换与 `Deep Thinking`
- **Download All** 改为并发处理，并支持中断按钮
- `Download All` 现在会补全缺失 description 的论文，不再只检查 `body.tex`
- 更新 README、README_zh、tech memo 与打包版本号到 `v0.7.0`

---

## v0.6.0 - Workspace 模式 & PDF 批量扫描

### v0.6.0 更新内容

- **Workspace 模式**：以文件夹为工作区（类似 Obsidian），每个子文件夹对应一篇论文
- 启动时自动恢复上次打开的 workspace，首次运行弹出文件夹选择器
- 左侧面板新增 "Open Folder" 按钮，随时切换 workspace
- **Scan PDFs**：一键扫描 workspace 中的 PDF，自动识别 arXiv ID（含版本号 v1/v2），创建论文文件夹
- **Download All**：一键批量下载所有待处理论文，显示进度 `3/10`，支持断点续传
- 每篇论文同时下载 PDF 到子文件夹，带下载进度显示
- 新增 "Open PDF" 按钮，在浏览器中打开 arXiv PDF
- arXiv ID 支持版本号后缀（`2604.12630v1`）
- 新增依赖 `pymupdf`（用于 PDF 识别）

---

## v0.5.0 - 论文历史列表 & 无限缓存

### v0.5.0 更新内容

- 新增左侧论文历史列表，显示所有已下载论文，点击即可快速加载
- 移除 50MB 缓存上限，改为无限缓存
- 三栏可拖拽布局（论文列表 / 预览区 / chat）
- 鼠标悬浮显示论文完整标题
- 加载失败时在预览区显示错误提示，并预填 arXiv ID 方便重新下载
- 按 arXiv ID 去重，自动选择可用的缓存文件夹

---

## v0.4.0 - Web 版本

### v0.4.0 更新内容

- 新增 Web 版本，支持跨平台使用（Windows、macOS、Linux、Android、iOS）
- PWA 支持，可安装到主屏幕
- 响应式设计，自动适配手机和电脑
- 零前端依赖，纯 HTML + CSS + JavaScript
