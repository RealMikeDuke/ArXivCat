# ArXivCat 技术架构

> 内部开发文档：理解系统如何工作、代码在哪里、改代码时别踩什么坑。
> 用户手册见 [cli.md](cli.md)；决策档案见 [maintenance-decisions.md](maintenance-decisions.md)。

## 1. 系统总览

两个 crate，职责严格分离：

```
arxivcat (workspace)
├── crates/arxivcat-core   库：下载、提取、manifest、AI 生成、chat（无 CLI 逻辑）
└── crates/arxivcat-cli    二进制：命令面、进程调度、锁协议、事件流
```

- **core 是纯库**：AI 生成管线不依赖 CLI；下载管线核心不依赖 AI（`is_complete` = 有 body.tex，无 key 也能下载）。
- **cli 是壳**：解析命令 → 调 core → 管进程/锁/契约。冻结契约（退出码、JSON 信封）全部在 cli 层实现。
- 所有写入（manifest/文件）都是**原子写**（tmp+rename，0600）。

## 2. 核心数据模型：PaperManifest + 五子结构

每篇论文一个目录（文件夹名 = base id，去版本号），manifest 是单一事实源：

```
{workspace}/2501_12948/
├── paper.json           PaperManifest：arxiv_id/title/files/ready 标记/cooldown
├── body.tex             提取的正文（LaTeX 原样）
├── appendix.tex         附录（若有）
├── note.txt             用户笔记
├── brief_summary.md     round-1 摘要（语义检索/chat 用）＋ .description_ready
├── deep_summary.md      round-2 深度复述（含附录表格原样 cp）＋ .deep_ready
├── arxiv_chats/         会话 JSON
├── .deep.lock           flock 常驻锁文件（内核互斥，勿删）
├── .brief.lock          flock 常驻锁文件（同上）
└── .deep.log            deep-worker 日志
```

关键规则：
- `description.md` 是历史遗留名，写路径惰性迁移为 `brief_summary.md`；空 stub 永不遮蔽真实 brief（`brief_complete` 要求标记 + 文件非空）。
- `download-all` 的 pending = 无 `body.tex`；`--force` 跳过 24h cooldown。

## 3. 执行模型（进程架构）

三种执行形态，语义不同：

| 形态 | 入口 | 行为 |
|---|---|---|
| 前台同步 | `paper download <id>` | 下载→提取→PDF→brief→deep，全等完 |
| 进程调度 | `paper download-all` | **每个 pending 论文 spawn 一个独立 `download-worker` 进程**（从下载第一步就是独立进程），`--jobs` 并发，读管道事件聚合 |
| 分离进程 | `internal deep-worker` | detached（独立进程组、stdio→.deep.log），download-all 每篇完成下载后 spawn，主进程不等 |

```
download-all（调度器进程）
  │ spawn ×N（--jobs）
  ▼
download-worker（进程 1）──stdout 管道──▶ 调度器实时读事件
  ├─ 下载 → 提取 → PDF → brief
  ├─ spawn deep-worker（孙进程，detached）→ 后台生成 deep
  └─ {"event":"done"/"failed"} → 聚合 success/failures/skipped
```

**事件流**（worker stdout，行分隔 JSON）：`downloading / downloaded / brief_done / deep_spawned / done / failed`。调度器只消费 `done`/`failed` 聚合；`Ctrl-C` 杀 download-worker，deep-worker 存活（独立进程组）。

## 4. 锁协议（kernel flock——勿改回内容型）

deep/brief 生成用 **flock(LOCK_EX|LOCK_NB)**，锁文件**常驻**（永不 unlink，unlink 破互斥）：

- 持有者是**生成者自身**（worker spawn 后自持锁；并发 spawn 的 worker acquire 失败直接 exit 0）。
- 进程退出/崩溃/被杀 → 内核自动释放（无 stale 回收、无 PID 判活、无幽灵锁）。
- 所有入口 **acquire → 锁下复检 ready 标记**（`.deep_ready` / `.description_ready`）。
- **门控不变量**：`generate_deep` 内部的 brief 重建是无锁的——所以任何入口在 brief 不完整时**绝不**进入 `generate_deep`。用 `BriefStatus`（Ready/Locked/Failed）区分：
  - Locked（他人正在生成）→ 跳过（对方完成自己的 deep）
  - Failed（我们持锁但生成失败）→ batch 仍 spawn worker 重试；单条跳过待下次
  - `--no-describe` = 绝不生成 brief，deep 仅在 brief 已存在时运行

**为什么是 flock 而不是内容型锁**：5 轮内容型协议（O_EXCL+PID 判活+stale 回收+回读校验）有真实天花板——µs 回收竞态、初始化窗口幽灵锁、PID 复用。3:0 jury 裁决换内核锁，见 maintenance-decisions.md。

## 5. AI 生成管线（两轮对话）

- **round 1** brief：`SUMMARY_SYSTEM` + `build_user1`（body/appendix 截断 120k）→ `brief_summary.md`
- **round 2** deep：**延续同一对话**（system+user1+assistant(brief 原文)+user2(deep 指令)）→ DeepSeek prefix-cache 命中（共享前缀 0.1 倍计费）
- 关键：`SUMMARY_SYSTEM` 与 `build_user1` 必须**逐字节一致**（worker 与前台从同一 manifest 读 id/title、同一文件读 body）——改其中一个就丢 cache。
- **表格数字不经 LLM**：`extract_tabular` 从 body/appendix 原样 cp LaTeX 表格到 deep_summary.md 附录（零转录）。
- 语义：best-effort **at-least-once**（失败请求可能已计费，batch 重试罕见重复 round-1；文档已明示）。

## 6. 代码地图

**core（`crates/arxivcat-core/src/`）**
| 模块 | 职责 | 关键函数 |
|---|---|---|
| `extract/source.rs` | 下载 tar.gz/PDF，DownloadLock（30s 等待+stale 回收+cooldown） | `download_source` |
| `extract/tex.rs` | 提取 body/appendix、tabular 原样 cp | `extract_body_from_dir`, `extract_tabular` |
| `extract/arxiv.rs` | 标题回填（key 归一化 strip_version——写/读两侧对称） | `fetch_titles_batch` |
| `manifest.rs` | PaperManifest、惰性迁移、cooldown、`strip_version` | `refresh_manifest`, `lazy_migrate_brief` |
| `workspace.rs` | Paper 扫描、`process_pending_paper`（on_event 回调） | `scan_workspace` |
| `chat/summary.rs` | 两轮生成（prefix-cache 关键） | `generate_brief`, `generate_deep` |
| `chat/description.rs` | 转发到 summary（兼容旧 API） | `build_description` |
| `chat/mod.rs` | side/global chat、`read_brief`（brief 优先+description 回退） | `build_side_chat_context` |
| `net.rs` | HTTP、env override（`ARXIVCAT_ARXIV_BASE_URL`/`ARXIVCAT_DEEPSEEK_BASE_URL`） | `HttpConfig` |

**cli（`crates/arxivcat-cli/src/`）**
| 位置 | 职责 |
|---|---|
| `main.rs` | clap 命令面（含隐藏 `internal deep-worker/download-worker`） |
| `commands/paper.rs` | 全部命令；**DeepLock（flock guard）、BriefStatus 门控、spawn_deep_worker、进程调度器**（本文件是并发正确性核心，改前必读第 4 节） |
| `commands/mod.rs` | 退出码/错误分类（冻结表） |

## 7. 开发工作流

```bash
# 门禁（CI 同款，必须全绿）
cargo +stable fmt --all -- --check
cargo +stable clippy --workspace --all-targets -- -D warnings
cargo +stable test --workspace
cargo +stable build --release --workspace

# 测试基建
# - wiremock mock arXiv（ARXIVCAT_ARXIV_BASE_URL）+ DeepSeek（ARXIVCAT_DEEPSEEK_BASE_URL）
# - exit_codes.rs 跑真实二进制（CARGO_BIN_EXE_arxivcat）
# - 回归测试钉死语义：busy 拒绝、--force 顺序、--no-describe 零 API（expect(0)）
```

- **版本哲学**：0.x minor = 破坏信号、patch = 零破坏；core 先发、cli 后发（cli 依赖 core crates.io）。
- **契约冻结**：退出码 0/1/2/3/4/5/6/7/8/130、JSON 信封形状——改前先想清楚（有契约测试矩阵）。
- **发布纪律**：版本 bump/行为变更同步更新 CHANGELOG、docs、README（用户会追查）。

## 8. 陷阱区（12 轮 jury-review 的血泪）

1. **锁协议别改回内容型**——flock 是 3:0 裁决的终态；任何"优化"回 exists()/PID 判活都是开历史倒车。
2. **门控别绕过**——任何新入口调 `generate_deep` 前必须过 brief 门控（`brief_complete` + BriefStatus）；`generate_deep` 内部重建是无锁的。
3. **锁下必须复检**——acquire 成功后要再查 ready 标记（双检锁），否则窄窗口双计费。
4. **`--force` 顺序**——先 acquire+门控，后清理产物；顺序反了会毁掉拒绝时本该保留的旧产物。
5. **flag 语义分裂**——同一 flag 单条/批量路径行为必须一致（`--no-describe` 曾分裂，被 MAJOR 抓回）。
6. **事件只聚合 done/failed**——`brief_done/deep_spawned` 是信息性的，调度器不消费。
7. **prefix-cache 前提**——改 `SUMMARY_SYSTEM`/`build_user1` 即丢缓存命中（不破坏正确性，只多花钱）。
8. **`std::process::exit` 不跑 Drop**——靠 guard 清理资源时记得 exit 路径显式处理（flock 后已无此问题，但模式还在）。
9. **测试互踩**——共享真实下载目录/APPDATA 的测试要串行化（全局 Mutex）。
10. **CI 语义**——GH Actions 默认 `bash -e`：预期非零退出码必须 `set +e` 收集后断言；门禁用最新 stable（新 lint 本地旧版看不见）。
