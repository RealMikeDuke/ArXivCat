# P2/P3 问题独立裁决报告（发布前裁决）

- 项目：ArXivCat v0.11.6（arxivcat-core 0.11.6 + arxivcat-cli 0.11.6）
- 裁决人：独立评审专家（本会话对项目无历史上下文）
- 裁决日期：2026-08-14
- 核实方式：全部 9 项均直接阅读源码确认存在/不存在（非仅凭评审报告转述）
- 背景约束：112 测试全绿、clippy/fmt/release 干净、三轮专家验收 3:0、即将发布 crates.io（core 先发）；用户偏好不为低价值项拖延发布，但已知问题应清掉或显式记录

---

## 裁决总表

| # | 问题 | 裁决 | 理由 | 信心 |
|---|------|------|------|------|
| P2-1 | title 回填走 abs 页请求 | **现在修** | 单篇 download 是 AI agent 主路径，每篇多一次 HTML 请求（延迟+请求量+429 面），修复成本低且有现成批量 API | 高 |
| P2-2 | find_paper_by_id 静默取第一个 | **现在修（文档层）** | 现网安全，但 core 即将成为 crates.io 公共库，发布后改签名是 breaking；现在用 doc 注释钉死契约，签名改 Result 延后 | 高 |
| P2-3 | 锁竞争武装 24h 冷却 | **延期** | 真实影响仅多进程并发场景；修复需动 P0 冷却状态机，中成本高风险的发布前改动 | 高 |
| P2-4 | Windows/macOS 从未验证 | **延期**（附宣传口径修正） | CI 矩阵是大工程；但 README 三平台声明应改为诚实口径（见「人类必须决定」#1） | 高 |
| P2-5 | config.json 损坏静默覆盖 | **现在修** | 不可逆数据丢失（API key+配置），最小修复（损坏文件先备份再覆盖）约 20 行+1 测试 | 高 |
| P3-1 | fetch_titles_batch 尾部 sleep 3s | **现在修** | 与 P2-1 捆绑（单篇改批量路径后，不修则每次单篇多等 3s）；改为仅 chunk 间 sleep，1 行 | 高 |
| P3-2 | list 长标题截断 | **不修（核实不存在）** | `{:<20}` 作用于 arxiv_id（9-11 字符）而非标题；标题为裸 `{}` 无宽度限制，全库无标题截断逻辑 | 高 |
| P3-3 | --jobs 双重 clamp | **不修，记 known-issues** | 无害冗余防御代码；发布前删除零用户价值、负安全价值 | 高 |
| P3-4 | token set 明文回显 | **延期**，记 known-issues | 需新依赖（rpassword）或平台特判；AI agent 场景已有 DEEPSEEK_API_KEY env 替代路径；存储侧已 0600 | 高 |

---

## 逐项核实与裁决细节

### P2-1 title 回填走 abs 页请求 —— 现在修 ✅（信心：高）

**核实**：`crates/arxivcat-cli/src/commands/paper.rs:142-147`，单篇 `paper download` 在下载+提取后调用 `fetch_title_from_arxiv`（GET `/abs/{id}` HTML 页，解析 og:title）回填 manifest 标题。download-all 路径（`workspace::process_pending_paper`，workspace.rs:234-280）不用 abs 页——直接用已有 manifest 的 `paper.title`，无此问题。

**影响**：单篇下载每篇多一次 HTML 页面请求（几百 ms~1s 延迟 + 双倍请求量 + 429 触发面）。对 AI agent 高频逐篇下载场景是实打实的开销，且是 429 限流的已知触发面。

**修复**：将 paper.rs:142 改为调用 `fetch_titles_batch(&http, &[arxiv_id])`（export API Atom，1 次请求）。`fetch_title_from_arxiv` 保留为公共 API（http_retry.rs:63 测试依赖），CLI 不再调用即可。exit_codes.rs:135 的 `/abs/` mock 变为冗余但无害。

**风险**：低。不涉及契约/退出码；改动局限在 title 获取路径。

### P2-2 find_paper_by_id 静默取第一个 —— 现在修（文档层）✅（信心：高）

**核实**：`crates/arxivcat-core/src/workspace.rs:166-168`：`find_paper_by_id` = `find_papers_by_id(..).into_iter().next()`，多匹配静默取第一个。CLI 全部 10 处调用走 `find_paper_or_die`（mod.rs:132-171，含歧义报错+精确 base-ID 优先），现网行为安全。

**影响**：core 即将作为公共库发布 crates.io。`find_paper_by_id` 是公共 API——发布后若改返回类型（`Result`）即为 breaking change。当前无文档说明其"多匹配取第一个"行为。

**修复**：①现在——加 doc 注释明确「多匹配时返回第一个，顺序不保证；如需歧义感知请用 `find_papers_by_id` 并自行裁决」，把行为契约钉死；②延后——下个里程碑（0.12.0）改签名返回 `Result` 或迭代器。

**风险**：极低（纯注释，零行为变更）。

### P2-3 锁竞争武装 24h 冷却 —— 延期 ⏸（信心：高）

**核实**：`extract/source.rs:17-67` `DownloadLock::acquire` 用 O_EXCL 原子创建，已存在时返回 `Err("another process is already downloading …")`；download-all 错误路径（paper.rs:280-301）对**任何** Err（含此瞬时锁冲突）执行 `mark_failure` + 24h 冷却，需 `--force` 绕过。

**影响**：仅多进程并发 `download-all` 同一工作区时触发。单进程（AI agent 常见形态）无影响。瞬时冲突被误记为失败并冷却 24h，属真实缺陷但触发面窄。

**修复成本/风险**：需在错误分类层区分「瞬时锁冲突（跳过、不冷却、或短重试）」与「真失败」，涉及 P0 冷却状态机 + 并发测试。发布前动核心状态机风险不成比例。

**处理**：记 known-issues，下里程碑修。

### P2-4 Windows/macOS 从未验证 —— 延期 ⏸（信心：高）

**核实**：`.github/workflows/ci.yml` 仅 ubuntu-latest；README 声明「Linux/macOS/Windows」三平台。代码中有平台相关分支：config.rs:6-7（APPDATA）、paper.rs:414（notepad/vi 编辑器回退）、`std::fs::rename`（POSIX 覆盖语义 vs Windows 目标存在即失败）——均仅 Linux 验证。

**影响**：三平台声明与验证覆盖不匹配；Windows 上 rename/路径/编辑器回退可能行为不同。但目标用户群（AI agent CLI）以 Linux 为主。

**修复成本**：CI 矩阵 + 平台兼容修复是大工程（数天级），且无 Windows/macOS runner 可即时验证。

**处理**：延期至主平台稳定后；发布前仅需修正宣传口径（见「人类必须决定」#1）。

### P2-5 config.json 损坏静默覆盖 —— 现在修 ✅（信心：高）

**核实**：`config.rs:89/95/106`——`get_workspace_path`、`save_workspace_path`、`save_token` 均 `Config::load().unwrap_or_default()`；`load_cached_token`（:100-102）用 `.ok()?`。config.json 损坏（JSON 语法错/半写）时静默回退默认，下一次 `save_token`/`save_workspace_path` 的原子写（temp+rename，:53-60）会**直接覆盖**损坏文件——用户丢失 API key 配置，且损坏原件无法找回取证。

**影响**：触发概率低（崩溃/手改/磁盘异常），但后果不可逆且涉及凭据配置。

**修复（最小版）**：在 `save_token`/`save_workspace_path` 的写路径前，若 `Config::load()` 返回 Err，先将损坏文件 rename 为 `config.json.corrupt-{timestamp}`（保留现场），打印警告，再正常写。约 20 行 + 1 个测试。不改 `Config::load` 的公共行为（Err 传播到全部调用方是更大改动，延后）。

**风险**：低。备份步骤不改变正常路径行为。

### P3-1 fetch_titles_batch 尾部 sleep 3s —— 现在修 ✅（信心：高）

**核实**：`extract/arxiv.rs:97-113`，`for chunk in ids.chunks(50)` 循环体内每 chunk 后 `sleep(3s)`，含最后一个 chunk——即使只有 1 个 id 也白等 3 秒。

**影响**：每次批量标题抓取固定多 3 秒；与 P2-1 修复耦合——单篇改走 `fetch_titles_batch` 后若不修此条，每次单篇 download 会多 3 秒。

**修复**：改为仅「非最后 chunk」后 sleep（`if !is_last { sleep }`），1 行。无测试依赖该时序。

### P3-2 list 长标题截断 —— 不修（核实不存在）❌（信心：高）

**核实**：`paper.rs:68`：`println!("{} {:<20} {} [{}]", status, p.arxiv_id, p.title, desc)`——`{:<20}` 作用于 **arxiv_id**（9-11 字符，宽度绰绰有余），标题为第三个裸 `{}`，**无宽度限制、无截断**。全代码库 grep 无针对 list 标题的截断逻辑（仅 chat/preview 的正文截断与标题无关）。

**结论**：该问题在现版本已不存在（推测为早期版本的残留描述）。无需修复，无需 known-issues 条目。

### P3-3 --jobs 双重 clamp —— 不修，记 known-issues 📝（信心：高）

**核实**：main.rs:71 clap `value_parser!(u8).range(1..=8)` + paper.rs:179 `jobs.clamp(1, 8)`，双重校验确认存在。

**影响**：无。clamp 是 clap 校验之外的防御性冗余（未来若改 clap 定义不会意外放行越界值）。

**处理**：发布前删除零用户价值且引入无谓改动面；记 known-issues（下里程碑清理时顺手删）。

### P3-4 token set 明文回显 —— 延期 ⏸，记 known-issues（信心：高）

**核实**：token.rs:103-108 `cmd_set` 用 `print!` + `read_line` 明文输入，无终端回显隐藏。存储侧已正确（config.rs:55-59 强制 0600）。docs/cli.md 仅描述「Prompt to enter token via stdin」，未承诺隐藏。

**影响**：终端肩窥/回显泄露风险，低（本地交互场景）。AI agent 场景可直接用 `DEEPSEEK_API_KEY` 环境变量（docs 已支持），可不走交互输入。

**修复成本**：隐藏回显需新增 `rpassword` 依赖（纯 Rust 跨平台）或平台特判 termios 代码（Windows 需 `crossterm` 类方案）。发布前引入新依赖扩大审核/供应链面。

**处理**：延期至下里程碑（随依赖策略一并决策），记 known-issues；文档补充「敏感环境建议使用 DEEPSEEK_API_KEY」。

---

## 建议的发布前补丁批次（一次验证，不拖延发布）

合并为一个小补丁集（预计 3 个文件、2 个新测试、一次全量验证）：

1. **P2-1**：paper.rs 单篇 title 回填改走 `fetch_titles_batch`
2. **P3-1**：arxiv.rs 仅 chunk 间 sleep（与 #1 捆绑）
3. **P2-5**：config.rs 写路径损坏文件先备份再覆盖（+1 测试）
4. **P2-2**：workspace.rs `find_paper_by_id` 加行为契约 doc 注释

版本建议：以上均非 breaking（P2-2 仅注释），可作为 v0.11.7 patch 或并入当前发布。**是否并入当前发布由人类拍板**。

## 人类利益相关者必须亲自决定的问题点

1. **README 平台宣传口径（P2-4，发布前）**：保持「Linux/macOS/Windows」三平台声明（承担未验证风险），还是改为「Linux 为主验证平台，Windows/macOS 最佳努力支持」（诚实口径，1 行改动）？
2. **P2-2 签名变更版本策略**：`find_paper_by_id` 下里程碑改返回 `Result`（breaking，需 0.12.0 版本号决策）还是长期维持 `Option` + 文档契约？
3. **补丁批次是否并入当前发布**：接受上述 4 项小补丁（v0.11.6 → v0.11.7 或直接随发布），还是严格按 v0.11.6 冻结发布、全部下里程碑处理？（我的建议：并入，成本一天内可消化）
4. **P3-4 依赖策略**：是否接受 `rpassword` 依赖（下里程碑）用于隐藏 token 输入？还是维持 env var 为唯一推荐路径、`token set` 明确标注「明文输入」？
5. **P2-3 冷却语义**：下里程碑修复时，「瞬时锁冲突不冷却」是否可接受为产品行为（与「真失败冷却 24h」并存）？

---

## 附件：known-issues 建议条目（下里程碑 backlog）

- [P2-3] download-all 多进程并发时瞬时锁冲突会被记为失败并武装 24h 冷却（需 --force 绕过）；修复方向：锁冲突错误分类为瞬时、跳过不冷却。
- [P3-3] --jobs 双重 clamp（clap range 1..=8 + 内部 clamp(1,8)），无害冗余，清理时删内部 clamp。
- [P3-4] token set 交互输入明文回显；修复方向：rpassword 依赖或文档引导 env var。
- [P2-4] Windows/macOS 未纳入 CI 矩阵，平台相关分支（APPDATA/notepad/rename 语义）仅 Linux 验证；修复方向：CI 矩阵 + 平台兼容测试（大工程，主平台稳定后）。
