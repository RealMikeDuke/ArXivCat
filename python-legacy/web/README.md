# ArxivCat Web 版本

基于浏览器的跨平台版本，支持 Windows、macOS、Linux、Android、iOS。

## 特性

- 🌐 跨平台（包括手机）
- 📱 PWA 支持（可安装到主屏幕）
- 🎨 响应式设计
- 💬 AI 助手集成
- 📦 零前端依赖

## 快速开始

```bash
# 安装依赖
pip install -r requirements-web.txt

# 启动服务器
.\run-web.ps1
```

访问 http://localhost:5000

## 手机使用

1. 手机和电脑连接同一 WiFi
2. 手机浏览器访问 `http://你的电脑IP:5000`
3. 点击"添加到主屏幕"
4. 像原生 app 一样使用

## 配置

使用 AI 助手需要设置环境变量：

```bash
# Windows
$env:GEMINI_API_KEY="your-api-key"

# Linux/macOS
export GEMINI_API_KEY="your-api-key"
```

## 技术栈

- 后端: Flask (复用 arxivcat/core.py)
- 前端: HTML + CSS + JavaScript (零依赖)
- PWA: Manifest + Service Worker

## 目录结构

```
web/
├── app.py              # Flask 后端
├── static/
│   ├── css/            # 样式
│   ├── js/             # 前端逻辑
│   ├── icons/          # PWA 图标
│   ├── manifest.json   # PWA 配置
│   └── sw.js           # Service Worker
└── templates/
    └── index.html      # 主页面
```

## API 接口

- `POST /api/extract` - 提取论文
- `POST /api/strip-comments` - 去除注释
- `POST /api/chat` - AI 聊天

## 部署

### 本地运行

```bash
cd web
python app.py
```

### 云端部署

可部署到 Vercel、Railway、Render 等平台。

## 故障排除

### 端口被占用

修改 `app.py` 最后一行的端口号。

### 手机无法访问

1. 确认同一 WiFi
2. 检查防火墙设置
3. Windows 可能需要允许 Python 通过防火墙

### PWA 无法安装

1. 使用 HTTPS 或 localhost
2. Chrome/Edge 需要访问至少 30 秒
3. 检查浏览器控制台错误

## 开发文档

详细文档见 `docs/` 目录。
