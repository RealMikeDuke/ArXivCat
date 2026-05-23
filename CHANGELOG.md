# ArxivCat v0.5.0 - 论文历史列表 & 无限缓存

## 更新内容

- 新增左侧论文历史列表，显示所有已下载论文，点击即可快速加载
- 移除 50MB 缓存上限，改为无限缓存
- 三栏可拖拽布局（论文列表 / 预览区 / chat）
- 鼠标悬浮显示论文完整标题
- 加载失败时在预览区显示错误提示，并预填 arXiv ID 方便重新下载
- 按 arXiv ID 去重，自动选择可用的缓存文件夹

---

# ArxivCat v0.4.0 - Web 版本

## 更新内容

- 新增 Web 版本，支持跨平台使用（Windows、macOS、Linux、Android、iOS）
- PWA 支持，可安装到主屏幕
- 响应式设计，自动适配手机和电脑
- 零前端依赖，纯 HTML + CSS + JavaScript

## 快速开始

```bash
# 启动 Web 版本
.\run-web.ps1

# 或使用桌面版
python main.py
```

## 文件结构

```
ArxivCat/
├── web/                    # Web 版本
│   ├── app.py              # Flask 后端
│   ├── static/             # 前端资源
│   └── templates/          # HTML 模板
├── docs/                   # 开发文档
├── run-web.ps1             # Web 版启动脚本
└── requirements-web.txt    # Web 版依赖
```

## 版本历史

- v0.5.0 - 论文历史列表 & 无限缓存
- v0.4.0 - 新增 Web 版本
- v0.3.0 - Tkinter 版本
- v0.2.1 - 修复提取逻辑
- v0.2.0 - 切换到 Flet

## 文档

- `web/README.md` - Web 版本说明
- `docs/QUICKSTART_WEB.md` - 快速启动指南
- `docs/DEVELOPMENT_SUMMARY.md` - 开发总结
- `tech_memo.md` - 技术备忘录
