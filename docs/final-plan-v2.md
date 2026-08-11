# ArXivCat CLI 独立项目化 — 最终定稿方案 v2（8 轮专家收敛版）

> 状态：**FINAL**。经 8 轮专家团迭代收敛（4 轮方案收敛 → 元评审 → 修订 → 定稿投票），全部决策已冻结。
> 生效日期：2026-08-11。实施时以此文档为唯一权威，与 docs/plan-v2-2026-08-11.md（第六轮中间产物）如有冲突以此为准。

---

## 一、最终决策总表

| 决策点 | 定稿 | 收敛方式 |
|---|---|---|
| 仓库策略 | 就地裁剪：先切 `legacy-gui` 归档分支，main 删 GUI，workspace 只剩 core+cli；回迁 = `git merge legacy-gui` | 3:0 |
| GUI 遗留代码 | 删 ErrorLevel/level()、compute_selection_delta、ChatSession 三字段 + fixture 锁旧 JSON 兼容；**description_ready 字段保留为信息展示**（is_complete 不依赖） | 3:0 / 2:1 |
| AI 解耦 | is_complete=has_body；3 处 build_description 全删（workspace.rs:202/231 + paper.rs:115）；`paper describe` 唯一入口 | 3:0 |
| paper.json manifest | P1 必做，唯一事实源；from_folder 优先读、legacy 只读回退、懒迁移（写操作前 ensure_manifest）、temp+rename 原子写 | 3:0 |
| 文件夹命名 | ID-only（vN strip，`2501_12948`）；标题进 manifest；title 失败降级 ID-only 不用 unknown | 3:0 |
| title 抓取 | export API（`export.arxiv.org/api/query?id_list=` 批量 Atom） | 3:0 |
| 并发 | download-all `--jobs N` 默认 4（绑定 429 退避为前置，退避未过 mock 测试前默认退回 2）；失败隔离、末尾结构化汇总、真 Ctrl-C | 3:0 |
| freshN | 删除（两处触发点）→ 可操作错误 + 有界重试 | 3:0 |
| repair_permissions | 保留收窄：公共函数 force_uniform_permissions（解压后 + validate 失败路径两处复用），symlink 跳过，P2 评估退役 | 3:0 |
| 退出码 | 0/1/2/3/4/5/6/7/8/130（见下表） | 多数决融合 |
| 信封 | `{"error":{code,kind,message,retryable}}`；--json 时信封走 stdout（stdout 恒单一 JSON 文档） | 2:1 |
| 冷却窗口 | 统一 24h（manifest 记 failed_at + failure_kind；--force 绕过；单篇 download 不受限） | 3:0 |
| CI | 从零新建一套 main-only（test + clippy -D warnings + release build） | 3:0 |
| README | 40-60 行精简版（第 0 步放，P2 再重写）；guidebook.md 删除 | 2:1 / 3:0 |
| 版本 | P0→0.10.0、P1→0.11.0、P2→0.12.0 | 3:0 |

## 二、最终 exit code 全表（P0 门禁后冻结）

| exit | 类别 | 覆盖（ArxivError 9 变体逐一映射） | retryable |
|---|---|---|---|
| 0 | 成功 | —；含 --help/--version | — |
| 1 | 其他/未分类 | `Other`；download-all 全败且类别混合 | 依 kind |
| 2 | 用法 | clap 解析错误；**ID 提取失败**；无 JSON 契约命令传 `--json` | false |
| 3 | 网络 | `Http`：连接失败/超时/5xx/429 退避耗尽 | true |
| 4 | 配置 | `Config`：缺 key/配置损坏/workspace 未配置 | false |
| 5 | 数据 | `Parse`/`Extraction`/`NotFound`/`Json`；find_paper_by_id 歧义 | false |
| 6 | IO/系统 | `Io`：磁盘满/权限错误 | false |
| 7 | 上游业务 | `Chat`：DeepSeek 401/403/配额/模型错误（重试无意义） | 视 kind：401/403 false，超时 true |
| 8 | download-all 部分成功 | ≥1 成功 且 ≥1 失败（幂等补跑） | true |
| 130 | SIGINT | Ctrl-C（128+2） | — |

信封 schema（--json 时 stdout 恒为单一 JSON 文档；人类错误恒走 stderr）：
```json
{"error": {"code": 7, "kind": "chat", "message": "...", "retryable": false}}
```
kind ∈ {io, http, parse, extraction, chat, config, not_found, json, other, usage}。

download-all 聚合：0=全成（含全 skipped/total=0）、8=部分（0<failed<attempted）、1=全败混合 / 类别码=全败单一。
payload（stdout 唯一文档）：
```json
{"status": "ok|partial|failed", "total": 10, "success": 7, "failed": 2, "skipped": 1,
 "failures": [{"arxiv_id": "2501.12948", "code": 3, "kind": "http", "message": "...", "retryable": true}]}
```

## 三、硬约束（5 条）

1. **先切 legacy-gui 分支，后删代码**，顺序不可颠倒。
2. **--jobs 4 与 429 退避绑定交付**（P0.5/P0.6 与 P1.4 同 PR；退避未实现并通过 mock 测试前默认值退回 2）。
3. **P0 与 P1 分两个版本发布**（P0 同格式裁剪可立即发版；P1 是 manifest + ID-only 破坏性格式变更）。
4. **P1 先落 manifest 再动命名/迁移**。
5. **退出码/JSON 契约只在 P0 改一次，P0 门禁后冻结**。

## 四、第 0 步（仓库操作，一天内一气呵成）

0.1 提交脏 Cargo.lock（0.9.1 bump）→ `git status` 干净。
0.2 切 `legacy-gui` 分支；提交信息含 revival 清单（core API 移除清单：ErrorLevel/level、3 处 build_description、compute_selection_delta、_freshN、repair_permissions；context_snapshot/view_name 回迁需补 #[serde(default)]；is_complete 语义变化）；断言归档树 `cargo build --release` 通过。
0.3 main 全量删除：`src/`、`src-tauri/`、`android-app/`（含 .gradle）、`python-legacy/`、`performance_profiling/`、`package.json`、`package-lock.json`、`index.html`、`tsconfig.json`、`vite.config.ts`、`assets/`、`README_zh.md`、`docs/archive/`、`docs/conventions.md`、`docs/guidebook.md`（含 docs/cli.md 对它的链接行）、根 `CHANGELOG.md`、`opencode.json`；`node_modules/`、`dist/` 仅工作树 rm（gitignored 不进 commit）。
0.4 workspace 只剩 core+cli；README 替换为 40-60 行精简版；LICENSE(MIT)。
0.5 门禁：`cargo test` + `cargo clippy -D warnings` + `cargo build --release` 三连绿；`git ls-files | grep -E 'src-tauri|^src/|android-app|python-legacy|package-lock'` 为空。

## 五、P0（v0.10.0，发布阻断：无 key 可用 + Agent 契约）

| # | 任务 | 验收断言（1-3 条） |
|---|---|---|
| P0.1 | AI 解耦：is_complete=has_body；3 处 build_description 全删；`paper describe <id> [--model]` 唯一入口；description_ready 保留为信息字段；list 两态 [C]/[.] + description: present/absent | 无 key 下载 exit 0、零 AI 调用；list --json 无 "pending"；describe 无 key exit 4/kind=config |
| P0.2 | sanitize_filename 字符边界截断 | 多字节 >80B 标题不 panic；79B 处 CJK 边界 fixture |
| P0.3 | find_main_tex 唯一实现 + 递归 + documentclass 校验（main.tex 候选也校验） | source/main.tex 命中；main.tex 无 documentclass + paper.tex 有 → 选后者；grep 单实现 |
| P0.4 | 错误契约：exit code 全表 + 信封 + 三通道 stdout 纯净（6 处污染点：paper.rs:73/147/154-158/171-176/196-202、token.rs:99）+ token status --json 落地 + chat --json → exit 2 | **golden 测试**：每个 --json 命令 stdout 恰一个 JSON 文档可解析；exit 2/3/4/5/6/7/8 各注入失败断言；download-all 三态 0/8/1；stderr 非 TTY 进度零字节 |
| P0.5 | HttpConfig{client, arxiv_base, deepseek_base} + from_env()（ARXIVCAT_ARXIV_BASE_URL/ARXIVCAT_DEEPSEEK_BASE_URL）+ 网络函数签名注入 &HttpConfig + wiremock 引入 | grep 零 Client::new()（仅 HttpConfig 内部）；env 指向 mock 时请求打到 mock |
| P0.6 | 重试退避：3 次、500ms×2^n、429/503 尊重 Retry-After（上限 30s） | 429×2 → 第 3 次成功（请求数=3）；429×3 → exit 3 + kind=http；间隔≥退避时长 |
| P0.7 | title 解耦 + 缓存优先（cache hit 零请求）+ ID-only 降级 + extract_arxiv_id 收紧（拒绝 DOI） | 断网下载成功、文件夹 `2501_12948` 形态无 "unknown"；DOI 链接不误配 |
| P0.8 | lossy 读取 + \subfile/\import/空格 \input 存在性检测警告 + [unexpanded] 标记（不硬失败） | latin-1 body.tex 提取成功；\subfile 论文提取成功 + stderr 警告 + 标记 |
| P0.9 | Workspace::open 去写副作用 | chmod 555 只读 workspace list exit 0 |
| P0.10 | config 原子写（temp+rename）+ API key 0600 | umask 0777 下 token mode==0600；无 .tmp 残留 |
| P0.11 | 依赖清理：anyhow/uuid/base64 删；tokio 生产只留 time；core dev-deps tokio(macros,rt-multi-thread)+wiremock | cargo machete 零 unused；build+test 绿 |
| P0.12 | GUI 遗留删除 + fixture | 含 GUI 字段旧 session JSON 可反序列化；五符号零引用 |
| P0.13 | 删 _freshN（→可操作错误+有界重试）+ force_uniform_permissions（解压后+validate 失败路径，symlink 跳过）+ repair_permissions 收窄保留 | 删除失败路径含路径+原因；恶意 tar（symlink→外部）零落地；0400→0644 |
| P0.14 | find_paper_by_id 歧义报错（前缀过渡期）；description 模型复用 load_model_preference + CHAT_MODELS | 前缀多候选 → exit 5 歧义错误列候选 |
| P0.15 | LICENSE(MIT) + docs/cli.md 同步（exit code 表、信封、三通道、describe 命令） | 文档与实现逐条一致 |

P0 门禁：test + clippy -D warnings + release 三连；契约测试入库；此后冻结退出码/JSON 契约。

## 六、P1（v0.11.0，结构层：先 manifest 后命名）

| # | 任务 | 验收断言 |
|---|---|---|
| P1.1 | paper.json manifest 唯一事实源：schema v1（arxiv_id/title/version/fetched_at/files/last_error/last_attempt_at）；写操作前 ensure_manifest；读双轨只读回退不迁移；temp+rename 原子写；24h 冷却持久化 | **旧格式 fixture 迁移测试**：真实旧目录（含 arxiv_chats/.description_ready/长标题）→ list 双轨 → 写操作后 manifest 正确；崩溃恢复 .tmp 残留；双进程并发只写一次 |
| P1.2 | ID-only 命名 + vN strip + manifest 精确匹配（删前缀模糊）+ 歧义报错 | v2→`2501_12948`+version="v2"；v3 覆盖同文件夹；两 ID 查询同篇；歧义列候选 |
| P1.3 | export API 批量 title + scan_workspace_pdfs 迁移（ID-only + 写 manifest） | 5 篇恰 1 次 export 请求；失败降级 ID-only 不阻塞；二次 scan 幂等 |
| P1.4 | download-all --jobs N（默认 4）+ 真 Ctrl-C（exit 130）+ 失败隔离 + 聚合 0/8/1 + 24h 冷却 + --force | wiremock 8 篇 429 前 2 请求全成；SIGINT→exit 130+汇总无孤儿；冷却论文计入 skipped |
| P1.5 | paper remove / paper redownload（源码层原子替换、保留 note/description/arxiv_chats） | meta 逐字节不变；坏 tar 旧内容原样 |
| P1.6 | wiremock 全量套件（429 退避、断连重试、export 解析、JSON 契约、跨进程并发） | ≥6 测试断网全绿 |
| P1.7 | 跨进程锁：`.locks/{base_id}.lock`；tar 临时文件唯一化（tempfile Builder） | 双进程同 ID 单缓存目录零残留；并发无 truncate |
| P1.8 | 旧式 ID（hep-th/9901001）文档化不支持 + 正则拒绝 | 旧式 ID 输入 → 明确错误信息 |

P1 门禁：同前 + 真实 fixture 端到端迁移演练（list→info→download-all→redownload→remove）。

## 七、P2（v0.12.0，打磨 + 回迁准备）

- P2.1 CI 从零新建：`.github/workflows/ci.yml`，branches:[main]，paths:[crates/**, Cargo.*]，job = fmt --check + clippy -D warnings + test（含 wiremock）+ release build。
- P2.2 GUI 回迁手册 + **实跑演练**（scratch clone → git merge legacy-gui → 适配 core API → cargo check 通过，附命令转录）。
- P2.3 per-crate CHANGELOG（0.10.0 起）+ README 重写 + docs 术语审计（grep "src-tauri|python-legacy" = 0）。
- P2.4 repair_permissions 退役评估（一个发布周期内零触发、零 issue 才退役）。
- P2.5 严重度/退出码文档化（9 码表入 docs/cli.md）。
- P2.6 crates.io 发布准备：core 先发 cli 后发；path→version 依赖；`cargo publish --dry-run` 通过。

## 八、三条最重要实施建议

1. **P0.4 契约（exit code + 信封 + stdout 纯净 + golden 测试）作第一个合并批次**——0.10.0 后再改码位 = 破坏性变更。
2. **第 0 步一天内一气呵成**：提交 lock → 切分支（revival 清单）→ 全量删除 → git ls-files 核对 → scratch clone 验证，才动 P0；删除以 grep 零残留为硬门禁。
3. **网络层只在 P0.5 一次成型**（HttpConfig + 退避 + base URL 缝），"新增 Client::new()/硬编码 URL"列为 code review 一票否决——P1.6 全依赖此缝。
