# ArxivCat 使用指南

## 版本选择

### 桌面版（Tkinter）
```bash
python main.py
```
适合：Windows 本地使用

### Web 版（推荐）⭐
```bash
.\run-web.ps1
```
适合：跨平台使用，包括手机

## Web 版快速开始

### 1. 启动服务器

```bash
# Windows PowerShell
.\run-web.ps1

# 或直接运行
.\run-web.bat
```

### 2. 访问应用

- **电脑**: http://localhost:5000
- **手机**: http://你的电脑IP:5000

### 3. 手机安装（可选）

1. 手机浏览器访问后
2. 点击"添加到主屏幕"
3. 像原生 app 一样使用

## 配置 AI 助手

```bash
# Windows
$env:GEMINI_API_KEY="your-api-key"

# Linux/macOS
export GEMINI_API_KEY="your-api-key"
```

## 功能说明

### 提取论文
1. 输入 arXiv ID（如 `2301.12345`）
2. 点击"提取"
3. 查看正文和附录

### 操作
- **复制**: 复制到剪贴板
- **去注释**: 去除 LaTeX 注释
- **日志**: 查看处理日志

### AI 助手
- 提取论文后可以提问
- 点击"重置"清空对话

## 故障排除

### 端口被占用
修改 `web/app.py` 中的端口号

### 手机无法访问
1. 确认同一 WiFi
2. 检查防火墙
3. 获取正确的 IP 地址（运行 `ipconfig`）

## 文档

- `web/README.md` - Web 版详细说明
- `docs/QUICKSTART_WEB.md` - 快速启动指南
- `tech_memo.md` - 技术备忘录
- `CHANGELOG.md` - 版本历史

## 版本

当前版本: **v0.4.0**

- v0.4.0 - Web 版本
- v0.3.0 - Tkinter 版本
- v0.2.1 - 修复提取
- v0.2.0 - Flet 版本
