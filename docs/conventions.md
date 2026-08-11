# Frontend Conventions

> **STATUS: GUI-era doc (React/Tauri).** Applies to the `legacy-gui` branch
> only; the CLI-only `main` build has no frontend. Kept on main so it survives
> future merges of `legacy-gui`.

# ArXivCat 开发约定

## 按钮预设 (`src/store.ts`)

```ts
BTN.surface0   // bg-[#313244] hover:bg-[#45475a]   — 次要/未选中按钮
BTN.surface1   // bg-[#45475a] hover:bg-[#585b70]   — 默认工具栏按钮
BTN.blue       // bg-[#89b4fa] hover:bg-[#b4d0fb]   — 强调/选中状态
BTN.green      // bg-[#a6e3a1] hover:bg-[#b8ebc0]   — 保存/确认
BTN.red        // bg-[#f38ba8] hover:bg-[#f5a0b9]   — 停止/取消
BTN.ghost      // hover:bg-[#313244]                 — 图标按钮/透明底
```

**规则**：所有按钮必须用 `RippleBtn` 包裹，颜色用预设，不允许手写 `bg-[#xxx] hover:bg-[#xxx]`。

```tsx
// ✓ 正确
<RippleBtn className={`rounded px-3 py-1.5 text-sm ${BTN.surface1}`}>Click</RippleBtn>

// 带状态切换
className={`rounded px-2 py-0.5 text-xs ${active ? BTN.blue : BTN.surface0}`}

// ✗ 禁止手写
<RippleBtn className="rounded bg-[#45475a] hover:bg-[#585b70] ...">Old</RippleBtn>
```

**`RippleBtn`** 默认自带 `transition-colors`，不需要手动加。

### 状态按钮 (`src/components/StateBtn.tsx`)

用于多步骤批量操作，按钮按状态自动切换颜色：

| 状态 | 颜色 | 用途 |
|------|------|------|
| `"idle"` | `BTN.blue` | 已选中，待执行 |
| `"running"` | `BTN.blue + pulse` | 正在执行 |
| `"done"` | `BTN.green` | 已完成 |
| `"error"` | `BTN.red` | 已失败 |

```tsx
<StateBtn status="running" disabled className="...">
  论文名
</StateBtn>
```

---

## Toast 提示 (`src/store.ts`, `src/components/Toast.tsx`)

```ts
showToast("Saved!")               // 绿色 (success, 默认)
showToast("Failed", "error")      // 红色
showToast("Info", "info")         // 蓝色
showToast("Warning", "warning")   // 黄色
```

toast 自动 1s 展示 + 滑出动画，无需额外处理。

---

## 居中弹窗 (`src/components/Dialog.tsx`)

居中模态，有 backdrop 遮罩，**抢焦点**。用于 GlobalChat / Log / Regen Desc。

| Prop | 默认 | 说明 |
|------|------|------|
| `open` | — | 显隐控制 |
| `onClose` | — | 关闭回调 |
| `title` | — | 标题栏内容（必填） |
| `children` | — | 正文区 |
| `headerExtra` | — | 标题栏右侧按钮 |
| `defaultWidth` | 600 | 初始宽度 |
| `defaultHeight` | 400 | 初始高度 |

关闭方式：点击 backdrop、按 Esc。

**性能**：resize/drag 直接操作 DOM，不触发 React 重渲染。不要在里面放频繁更新的 state。

## 下拉浮窗 (`src/components/Dropdown.tsx`)

in-place 弹出，**不抢焦点**。用于 Token 配置、Session 选择器。

| Prop | 必填 | 说明 |
|------|------|------|
| `open` | 是 | 显隐控制 |
| `onClose` | 是 | 关闭回调 |
| `anchorRef` | 是 | 触发按钮的 ref，用于定位和点击外部判断 |
| `children` | 是 | 内容 |
| `width` | 否 | 弹窗宽度，用于右边界限幅 |

关闭方式：点击外部、点击触发按钮、按 Esc。

```tsx
// 使用方式
const btnRef = useRef<HTMLDivElement>(null);

<>
  <span ref={btnRef}><RippleBtn onClick={() => setOpen(!open)}>Toggle</RippleBtn></span>
  <Dropdown open={open} onClose={() => setOpen(false)} anchorRef={btnRef}>
    <div className="p-3">内容</div>
  </Dropdown>
</>
```

---

## 共享组件

### `useContextRestore` (`src/hooks/useContextRestore.ts`)

用于在 session 切换时恢复 context selection，避免 `sessions` 变化时重复覆盖：

```ts
useContextRestore(activeIdx, sessions, "side", (session) => {
  // 恢复逻辑
}, [extraDeps]);
```

内置 `restoredKey` guard，同一个 session 只恢复一次。

### `ContextSelector` (`src/components/ContextSelector.tsx`)

Global Chat 的 per-paper context 选择 UI，带 "All X" 批量按钮。需要时复用，不要手写。

### `ChatTitleBar` (`src/components/ChatTitleBar.tsx`)

居中 pill 样式，用于对话线程标题。

---

## 命名约定

| 类型 | 规则 | 例 |
|------|------|----|
| Zustand action | `camelCase`, 动词开头 | `toggleSideChat`, `refreshPapers` |
| Store state | `camelCase` | `sideChatOpen`, `currentPaper` |
| Component | PascalCase | `ChatPanel`, `ContextSelector` |
| Hook | `use` 开头 | `useChatSessions`, `useContextRestore` |
| Icon-only button | `BTN.ghost` + `hover:text-*` | — |

## 文件结构

```
src/
├── components/     # 每个组件一个文件
├── hooks/          # 共享 hook
├── store.ts        # Zustand store + 所有预设
├── index.css       # 全局样式 + @keyframes
```

不要往 `store.ts` 里加业务逻辑，只放 state + action。

---

## 日志规范

所有 `catch` / `.catch()` 必须调用 `addLog`，不允许静默吞错误。

```tsx
// ✓ 正确
try { ... } catch (e) { useStore.getState().addLog(`[ERROR] 描述: ${e}`); }

// ✗ 禁止
try { ... } catch { /* silent */ }
try { ... } catch (e) { console.error(e); }
```

操作成功也应有日志：
```tsx
addLog("[OK] Description regenerated");
addLog("[INFO] Start batch download...");
```

日志级别前缀：
| 前缀 | 用途 |
|------|------|
| `[OK]` | 成功 |
| `[INFO]` | 开始/进度 |
| `[ERROR]` | 失败 |
| `[WARN]` | 警告 |
