# ArxivCat Web 版本 - 快速启动指南

## 第一次使用

### 1. 安装依赖

```bash
# 使用 conda web 环境（推荐）
conda activate web
pip install -r requirements-web.txt

# 或使用其他 Python 环境
pip install -r requirements-web.txt
```

### 2. 启动服务器

```bash
# Windows
.\run-web.ps1

# 或手动启动
cd web
python app.py
```

### 3. 访问应用

打开浏览器访问：http://localhost:5000

## 手机使用

### Android / iOS

1. **确保手机和电脑在同一 WiFi**

2. **获取电脑 IP 地址**
   - Windows: 运行 `ipconfig`，查看 IPv4 地址
   - 例如: `192.168.1.100`

3. **手机浏览器访问**
   - 打开 Chrome/Safari
   - 访问 `http://192.168.1.100:5000`

4. **安装到主屏幕**
   
   **Android (Chrome/Edge):**
   - 点击浏览器菜单（⋮）
   - 选择"添加到主屏幕"或"安装应用"
   - 完成！

   **iOS (Safari):**
   - 点击分享按钮（□↑）
   - 选择"添加到主屏幕"
   - 完成！

5. **使用**
   - 从主屏幕点击 ArxivCat 图标
   - 像原生 app 一样使用
   - 没有浏览器地址栏和菜单

## Windows 桌面使用

1. 使用 Chrome 或 Edge 访问 http://localhost:5000
2. 地址栏右侧会出现"安装"图标（⊕）
3. 点击安装
4. 应用会出现在开始菜单

## 功能说明

### 提取论文

1. 输入 arXiv ID（如 `2301.12345`）或完整 URL
2. 点击"提取"按钮
3. 等待处理完成
4. 查看提取的正文和附录

### 操作按钮

- **正文/附录**: 切换视图
- **📋 复制**: 复制到剪贴板
- **🧹 去注释**: 去除 LaTeX 注释
- **📊 日志**: 查看处理日志

### AI 助手

1. 提取论文后，右侧聊天面板激活
2. 输入问题，AI 基于论文内容回答
3. 点击"重置"清空对话历史

**注意**: 需要设置 `GEMINI_API_KEY` 环境变量

```bash
# Windows
$env:GEMINI_API_KEY="your-api-key"

# Linux/macOS
export GEMINI_API_KEY="your-api-key"
```

## 常见问题

### 手机无法访问

1. 确认手机和电脑在同一 WiFi
2. 检查防火墙是否阻止 5000 端口
3. Windows 防火墙可能需要允许 Python

### 端口被占用

修改 `web/app.py` 最后一行：

```python
app.run(host='0.0.0.0', port=5001, debug=True)  # 改成其他端口
```

### PWA 无法安装

1. 确保使用 HTTPS 或 localhost
2. Chrome/Edge 需要访问至少 30 秒
3. 检查浏览器控制台是否有错误

## 技术架构

```
后端: Flask (Python)
  ├─ 复用 arxivcat/core.py
  ├─ REST API
  └─ Gemini API 集成

前端: HTML + CSS + JavaScript
  ├─ 响应式布局
  ├─ Fetch API
  └─ PWA 支持
```

## 与桌面版对比

| 特性 | 桌面版 | Web 版 |
|------|--------|--------|
| 跨平台 | Windows | 全平台 |
| 手机支持 | ❌ | ✅ |
| 安装 | 需要打包 | 浏览器直接用 |
| UI | Tkinter | HTML/CSS |
| 部署 | 本地 | 本地或云端 |

## 下一步

- 详细文档: [web/README.md](web/README.md)
- 技术细节: [tech_memo.md](tech_memo.md)
- 主项目: [README_zh.md](README_zh.md)
