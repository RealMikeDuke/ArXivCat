# MinerU 集成调研（PDF 输入路径）

> 状态：**调研完成、概念验证通过、未实施（TODO）**。本文档固化 API 实测结论与集成设计，实施时以此为准。
> 属于 0.12.0 大改的 PDF 输入路径前置调研，详见 [0.12-content-pdf.md](0.12-content-pdf.md)。

## 一句话

MinerU（opendatalab，77.7k stars）把 PDF/Office 解析成 LLM-ready 的 Markdown/JSON。概念验证全链路跑通：上传 → 解析 → 下载 → 质量实测通过（表格 6/6、数字 8/8 与 LaTeX 源一致）。**实施已列为 TODO，暂不开发。**

## API 全貌（2026-08-16 实测）

两种 API：

| | 🎯 精准解析（需 token） | ⚡ Agent 轻量（免 token） |
|---|---|---|
| 接口 | `POST /api/v4/extract/task`（单，URL）<br>`POST /api/v4/file-urls/batch`（批量，**本地上传**） | `POST /api/v1/agent/parse/file`<br>`POST /api/v1/agent/parse/url` |
| 鉴权 | `Authorization: Bearer <token>` | IP 限频 |
| 限额 | ≤200MB / ≤200 页 / 批量 ≤50 个 / 每天 1000 页高优先级 | ≤10MB / ≤20 页 |
| 输出 | zip：`full.md` + `content_list.json` + `layout.json` + `_model.json` + `origin.pdf` + `images/`；可加 `extra_formats:["latex","docx","html"]` | 仅 Markdown（CDN 链接） |
| 模型 | `pipeline`（默认）/ `vlm`（推荐）/ `MinerU-HTML` | 固定轻量 |

### 关键事实

1. **`extract/task` 不支持文件上传（只收 URL）**——但 `file-urls/batch` 支持**本地文件上传**（预签名链接）。文档表述误导，"不支持上传"仅指单任务接口。
2. **本地上传流程（实测跑通）**：
   ```
   POST /api/v4/file-urls/batch        # Bearer token
     {"files":[{"name":"x.pdf","data_id":"myid"}], "model_version":"vlm",
      "enable_table":true, "enable_formula":true}
     → data.batch_id + data.file_urls（预签名 OSS 链接，有效期 24h）
   PUT <file_url>                       # curl -X PUT -T，无 Content-Type
     → 上传完系统自动提交解析任务，无需再调接口
   GET /api/v4/extract-results/batch/{batch_id}
     → extract_result: [{data_id, file_name, state: done/running/failed,
         full_zip_url, err_msg}]
   ```
3. **轮询**：`GET /api/v4/extract/task/{task_id}`（单任务）或 `extract-results/batch/{batch_id}`（批量）。state: `pending/running/done/failed/converting`。
4. **zip 内容**：`full.md`（Markdown 主结果）、`content_list.json`（**结构化内容列表，含表格 HTML 块 + img_path**）、`layout.json`、`origin.pdf`、`images/`。
5. **表格输出是 HTML**（`<table><tr><td>`，含 rowspan/colspan）不是 Markdown 管道表，也不是 LaTeX tabular。content_list 的 `table_body` 是 HTML，`table_caption` 是标题。
6. **表格有 img_path**（表格以图像识别/转录，数字经 vlm 模型转录——有误差风险，非零转录）。

### 实测结果（2602.22441，vlm 模型）

- 全链路：申请链接 → PUT 上传 → 自动解析 → 一次轮询 done → 下载 zip（~900KB）
- 质量：标题/作者/章节结构完整 ✅；公式 LaTeX 化（`$h_t = F_θ(...)$`）✅；表格 **6/6 全部提取** ✅
- **数字准确性实测**：Table 6 的 8 个数字（97.80/97.40/93.60/93.60/97.80/94.80/93.80/4.40）与 LaTeX 源**逐格一致** ✅
- 缺陷观察：full.md 中个别正文有拼写粘连（"hain-of-thought"、"comprehen sive"）——低概率噪音，可接受

### token 信息（安全注意事项）

- token 格式：`sk-...`（OpenXLab 格式）或 JWT 均可；JWT 有 exp 过期（实测旧 token 401 `A0202 user authenticate failed`，需重新创建）
- 对话中提供过两个 token（一个 JWT 已过期、一个 sk- 有效），**均已只存在会话环境变量 `MINERU_TOKEN`，未落盘、未进 git**
- 实施时：token 存 config（对齐 `deepseek_api_key` 模式），支持 env（如 `MINERU_TOKEN`）；401 → 错误映射 config/4 + "重新创建 token"提示

## 集成设计（实施时用）

```
arxivcat paper ingest-pdf <file.pdf> [--id <自定义ID>] [--title <自定义标题>]
  → 计算内容 hash（身份，见 0.12-content-pdf.md 身份体系）
  → 尝试提取 arXiv ID（PDF metadata / arXiv: 前缀，尽力而为）
  → POST file-urls/batch → PUT 上传 → 轮询 extract-results/batch → 下载 zip
  → content_list.json 按 Appendix 标题切分 body/appendix（如实施分块）
  → full.md 落 content.md，源 PDF 存论文目录（files.pdf）
  → manifest：source:"pdf"、arxiv_id（可空）、files.content、files.pdf
  → 默认自动 brief/deep（与 download 流程完全一致，仅来源不同）
  → 失败映射：MinerU task failed → data/5；轮询超时 → net/3；zip 破损 → data/5
```

命令面未定项（实施时决策）：`--json` 信封形状（对齐 download 信封，arxiv_id 空时输出 `pdf_<hash12>`）、失败残留目录处理、并发 ingest 去重（复用 DownloadLock 或 per-dir flock）。

## TODO（实施清单）

- [ ] MinerU API client（file-urls/batch 申请 → PUT → 轮询 → zip 下载解包）
- [ ] `ingest-pdf` 命令（信封、自动摘要、失败映射、--id/--title）
- [ ] token 配置（config 字段 + env + token 命令面扩展）
- [ ] content_list.json 保留入库（结构化表格/溯源，比从 full.md 现榨 HTML 可靠）
- [ ] 身份体系接入（hash 计算 + slug + 提取 ID）
- [ ] 实测更复杂的论文（长表格、多栏、扫描件）验证 vlm 转录质量边界
- [ ] 试 `extra_formats:["latex"]`——若 latex 导出把表格 tabular 化，"零转录"哲学可在 PDF 源部分恢复
- [ ] 隐私合规：非 arXiv 私有 PDF 全文上传第三方——文档注明
