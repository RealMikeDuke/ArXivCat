# ArXivCat 项目导航（meta）

> **一切的一切从这里开始。** 不知道看哪个文档？先读这份。
> 本文件是文档体系的根节点，只做导航与速查，不展开细节。

## 这是什么

ArXivCat 是一个 **arXiv 论文工作区 CLI**（Rust，双 crate）：下载 LaTeX 源码 → 提取 body/appendix → 生成双粒度 AI 摘要（brief/deep）→ 支持语义检索、笔记、与论文对话。核心用户是 **AI Agent**（冻结的退出码 + JSON 契约），其次是研究者个人。

- 仓库：`github.com/RealMikeDuke/ArXivCat`（本地 `~/ArXivCat`）
- 形态：纯 CLI，core 无 AI 依赖的下载管线承诺；GUI 是远期（legacy-gui 分支）
- 现状：**未发版**（crates.io 待用户 token + 发布意愿），版本候选 0.11.x

## 文档地图（docs/）

| 文件 | 作用 | 谁读 |
|---|---|---|
| **meta.md**（本文件） | 导航总入口 | 所有人 |
| **architecture.md** | 技术架构：进程模型/flock 锁/AI 管线/代码地图/陷阱 | 内部开发者（快速上手） |
| **cli.md** | 用户手册：命令/JSON/退出码契约/工作流 | 使用者、Agent 集成 |
| **maintenance-decisions.md** | 维护者决策档案（错误分类、发布顺序、flock 裁决、known-issues） | 维护者 |
| **conventions.md** | 代码/文档约定 | 贡献者 |
| **adjudication-p2-p3.md** | jury-decide 裁决档案（P2/P3 源码级核实） | 维护者 |
| **final-plan-v2.md** | 最终方案（纯 CLI 转型蓝图） | 历史背景 |
| **guidebook.md** | 旧 GUI 时代指南（历史，仅背景） | 历史 |
| **gui-revival.md** | GUI 复活手册（远期） | 未来 GUI 工作 |

**推荐阅读顺序**（新开发者）：`meta.md` → `architecture.md` → `cli.md`（契约部分）→ 挑一个模块读代码 → 改完跑第 7 节门禁。

## 按角色进入

| 你想做什么 | 入口 |
|---|---|
| 快速理解系统怎么工作 | `architecture.md` 全文（15 分钟） |
| 上手写第一个功能 | `architecture.md` 第 6 节代码地图 + 第 7 节开发工作流 + 第 8 节陷阱 |
| 用这个工具 | `cli.md`（Quick Start + 命令表） |
| 给 Agent 集成 | `cli.md` 的 JSON 信封 + 退出码契约表（冻结） |
| 改设计/做技术决策 | `maintenance-decisions.md`（先查是否已有裁决，避免重打 12 轮 review） |
| 发布/版本号 | `maintenance-decisions.md` 发布顺序 + `architecture.md` 版本哲学 |
| 理解锁/并发为什么这么写 | `architecture.md` 第 4 节 + `maintenance-decisions.md` flock 裁决 |

## 关键概念速查

| 概念 | 一句话 | 详见 |
|---|---|---|
| 五子结构 | 每篇论文 = body/appendix/note/brief_summary/deep_summary | architecture §2 |
| PaperManifest | 论文目录的单一事实源（paper.json，原子写） | architecture §2 |
| 进程调度 | download-all 每篇一个独立 download-worker 进程 + 管道事件流 | architecture §3 |
| detached worker | deep-worker 独立进程组后台生成，Ctrl-C 杀不掉 | architecture §3 |
| flock 锁 | `.deep.lock`/`.brief.lock` 内核锁，常驻文件，进程死自动释放 | architecture §4 |
| BriefStatus 门控 | Ready/Locked/Failed 三态，任何入口进 generate_deep 前必须过 | architecture §4 |
| 两轮生成 | brief→deep 延续同一对话（prefix-cache 命中） | architecture §5 |
| 表格 cp | 数字表格 LaTeX 原样复制，不经 LLM | architecture §5 |
| 冻结契约 | 退出码 0/1/2/3/4/5/6/7/8/130 + JSON 信封形状 | cli.md |

## 项目状态速查

- **测试**：126 全绿（cli 单元 11 + cli_contract 9 + exit_codes 9 + core 60 + http_retry 8 + integration 27 + summary_rounds 2）
- **门禁**：fmt + clippy `-D warnings`（最新 stable）+ test + release build
- **CI**：GitHub Actions（ubuntu-latest 单矩阵；bash -e 语义已在 smoke 处理）
- **版本线**：0.10.0 → 0.11.0 … → 0.11.11（tagged）；v0.11.12 tag 已撤，方案 3（摘要管线+进程化+flock）在 main 未发版
- **发布**：待用户 token + 意愿；core 先发、cli 后发（cli 依赖 core crates.io）
- **遗留 known-issues**：Windows/macOS best-effort 未 CI；下载锁 30s 后仍 24h 冷却（--force 兜底）；非 unix 锁 fallback 需手动清理
