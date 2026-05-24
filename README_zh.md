# ArXivCat

[English README](README.md)

ArXivCat 是一个处理 arXiv LaTeX 源码包的小型桌面工具。
它会下载源码、展开 LaTeX 里的 `\input` / `\include`，并导出相对干净的 `body.tex` 和 `appendix.tex`。

这个项目面向一个很直接的工作流：粘贴 arXiv 链接或 ID，查看提取后的文本，做一点轻量编辑，并在需要时使用右侧内置的 DeepSeek chat，对当前论文内容做快速问答。

![ArXivCat 截图](assets/screenshot.png)

## 功能

- **Workspace 模式**：以文件夹为工作区（类似 Obsidian），每个子文件夹对应一篇论文
- 启动时自动恢复上次打开的 workspace；可随时用 "Open Folder" 切换
- **Scan PDFs**：一键扫描 workspace 中的 PDF，自动识别 arXiv ID（支持版本号如 `2604.12630v1`）
- **Download All**：并发批量下载，支持完整性补全、进度显示和中断按钮
- 每篇论文自动下载 PDF 到子文件夹
- **Open PDF**：在浏览器中打开 arXiv PDF
- 支持从 arXiv 页面链接、PDF 链接或纯 arXiv ID 下载源码包
- 自动解压并缓存 arXiv 源码（无限缓存）
- 递归展开 LaTeX `\input` 和 `\include`
- 自动寻找主 TeX 文件
- 导出 `body.tex` 和 `appendix.tex`
- 自动为每篇论文生成 `description.md`（使用独立的 DeepSeek Flash 描述流程）
- 每篇论文子文件夹现在可包含 `body.tex`、可选 `appendix.tex`、`note.txt`、`description.md`、PDF 和描述完成标记
- 三栏可拖拽布局（论文列表 / 预览区 / chat）
- Tkinter GUI 可预览 `body`、`appendix`、`note`、`description`
- 右侧 DeepSeek side chat，支持流式输出
- 面向整个 workspace 描述集合的 **Global Chat**

## 项目边界

ArXivCat 的目标比较收敛。

- 它不是完整的 LaTeX 编译器。
- 它不保证对所有论文源码结构都能完美解析。
- 右侧 chat 主要用于轻量阅读辅助，不是针对超长论文的完整检索系统。

## 版本选择

ArXivCat 提供两个版本：

### 桌面版（Tkinter）

传统桌面应用，适合 Windows 用户。

**安装依赖：**

```bash
pip install -r requirements.txt
```

**运行：**

```bash
# GUI
python main.py

# CLI
python cli.py --url 2601.11514
python cli.py --url https://arxiv.org/abs/2601.11514
```

### Web 版（推荐）⭐

基于浏览器的版本，支持 Windows、macOS、Linux、Android、iOS。

**特性：**

- 🌐 跨平台（包括手机）
- 📱 可安装到主屏幕（PWA）
- 🎨 响应式设计
- 💬 集成 AI 助手

**快速开始：**

```bash
# 安装依赖
pip install -r requirements-web.txt

# 启动服务器
.\run-web.ps1
```

访问 <http://localhost:5000>

**手机使用：**

1. 手机和电脑连接同一 WiFi
2. 手机浏览器访问 `http://你的电脑IP:5000`
3. 点击"添加到主屏幕"
4. 像原生 app 一样使用

详见 [web/README.md](web/README.md)

---

## 配置

如果要使用 chat，需要在环境变量里设置 `DEEPSEEK_API_KEY`：

```bash
# Windows PowerShell
$env:DEEPSEEK_API_KEY="your-api-key"

# Linux/macOS
export DEEPSEEK_API_KEY="your-api-key"
```

## GUI 使用流程（桌面版）

1. 首次启动时选择一个 workspace 文件夹
2. 粘贴 arXiv 链接或 ID → 点击 `Run` → 论文下载并提取到 workspace
3. 或者：把 PDF 放到 workspace 文件夹 → 点击 `Scan PDFs` → 再点 `Download All`
4. 点击左侧任意论文即可加载
5. 使用操作按钮：`Copy`、`Open Folder`、`Open PDF`、`Strip Comments`
6. 切到 `description` 视图查看自动生成的论文简介
7. 用右侧 side chat 对当前已加载论文做快速总结或解释
8. 用左侧的 `Global Chat` 对整个 workspace 的论文描述做问答、筛选和比较

## Chat 面板

桌面版现在有两个相关 chat 面板：

- **Side chat**：基于当前预览区文本
- **Global Chat**：基于当前 workspace 中所有 `description.md`

两者共享同一套面板结构，都支持 `Flash` / `Pro` 模型切换和可选的 Deep Thinking。

功能特性：

- 流式输出实时反馈
- 深度思考模式开关（可选）
- 中止按钮可取消长响应
- 性能指标显示（TTFT、tokens/sec、token 使用量）
- Side chat 会把当前预览区文本作为上下文发送
- Global Chat 会把当前 workspace 中所有带编号的论文 description 作为上下文发送
- 会保留一个短期的多轮内存历史
- 点击 `Reset` 会清空 chat 记忆
- 最适合在 description 已经为 workspace 论文生成完成后使用

## 每篇论文子文件夹内容

每个论文子文件夹现在通常包含：

- `body.tex`
- `appendix.tex`（可选）
- `note.txt`
- `description.md`
- 下载的 PDF
- `.description_ready`

## 输出目录

- workspace：用户选择的文件夹（每篇论文是一个子文件夹，包含提取结果、便签、description、完成标记和 PDF）
- 下载缓存：`%APPDATA%/ArxivCat/downloads/`
- 配置文件：`%APPDATA%/ArxivCat/config.json`

如果缓存目录不可读，ArXivCat 可能会自动重下，或者写到 `*_freshN` 目录。

## 打包

Windows 打包目前使用 `build.ps1`，底层是 PyInstaller，默认依赖 `arxivcat` 这个 conda 环境。

## 给维护者

如果你准备继续维护或扩展这个项目，建议先读 `tech_memo.md`。
