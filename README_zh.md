# ArXivCat

[English README](README.md)

ArXivCat 是一个处理 arXiv LaTeX 源码包的小型桌面工具。
它会下载源码、展开 LaTeX 里的 `\input` / `\include`，并导出相对干净的 `body.tex` 和 `appendix.tex`。

这个项目面向一个很直接的工作流：粘贴 arXiv 链接或 ID，查看提取后的文本，做一点轻量编辑，并在需要时使用右侧内置的 Gemini chat，对当前论文内容做快速问答。

![ArXivCat 截图](assets/screenshot.png)

## 功能

- 支持从 arXiv 页面链接、PDF 链接或纯 arXiv ID 下载源码包
- 自动解压并缓存 arXiv 源码
- 递归展开 LaTeX `\input` 和 `\include`
- 自动寻找主 TeX 文件
- 导出：
  - `body.tex`
  - 如果存在则导出 `appendix.tex`
- 提供 Tkinter GUI，用于预览和轻量编辑提取结果
- 右侧带一个轻量 Gemini chat 面板

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

访问 http://localhost:5000

**手机使用：**
1. 手机和电脑连接同一 WiFi
2. 手机浏览器访问 `http://你的电脑IP:5000`
3. 点击"添加到主屏幕"
4. 像原生 app 一样使用

详见 [web/README.md](web/README.md)

---

## 配置

如果要使用 chat，需要在环境变量里设置 `GEMINI_API_KEY`：

```bash
# Windows PowerShell
$env:GEMINI_API_KEY="your-api-key"

# Linux/macOS
export GEMINI_API_KEY="your-api-key"
```

## GUI 使用流程（桌面版）

1. 粘贴 arXiv 链接或 ID
2. 点击 `Run`
3. 查看提取后的 `body` 或 `appendix`
4. 可选使用：
   - `Copy`
   - `Overwrite`
   - `Open Folder`
   - `Strip Comments`
5. 用右侧 chat 做快速总结或解释

## Chat 面板

当前 chat 使用 `gemini-2.0-flash-lite`。

目前的行为：

- 会把当前预览区文本作为上下文发送
- 会保留一个短期的多轮内存历史
- 点击 `Reset` 会清空 chat 记忆
- 最适合在左侧已经加载论文内容之后使用

## 输出目录

- 缓存：`%APPDATA%/ArxivCat/downloads/`
- 结果：`%APPDATA%/ArxivCat/outputs/`

如果缓存目录不可读，ArXivCat 可能会自动重下，或者写到 `*_freshN` 目录。

## 打包

Windows 打包目前使用 `build.ps1`，底层是 PyInstaller，默认依赖 `arxivcat` 这个 conda 环境。

## 给维护者

如果你准备继续维护或扩展这个项目，建议先读 `tech_memo.md`。
