import { useState, useRef, useCallback } from "react";
import { BTN } from "../store";
import { ChatSession } from "../hooks/useChatSessions";
import RippleBtn from "./Ripple";
import Dropdown from "./Dropdown";

interface Props {
  sessions: ChatSession[];
  activeIdx: number;
  onNew: () => void;
  onSwitch: (idx: number) => void;
  onRename: (idx: number, title: string) => void;
  onDelete: (idx: number) => void;
  onRegenTitle?: (idx: number) => void;
  kind: "paper" | "global";
  compact?: boolean;
}

export default function ChatSessionBar({ sessions, activeIdx, onNew, onSwitch, onRename, onDelete, onRegenTitle, kind, compact }: Props) {
  const [open, setOpen] = useState(false);
  const [renamingIdx, setRenamingIdx] = useState(-1);
  const [renameValue, setRenameValue] = useState("");
  const btnRef = useRef<HTMLDivElement>(null);

  const handleOpen = useCallback(() => {
    setOpen((v) => !v);
  }, []);

  const startRename = (idx: number, cur: string) => { setRenamingIdx(idx); setRenameValue(cur); };
  const commitRename = () => { if (renamingIdx >= 0 && renameValue.trim()) onRename(renamingIdx, renameValue.trim()); setRenamingIdx(-1); };

  const sessionList = (
    <div className="flex flex-col">
      <div className="flex items-center justify-between border-b border-[#313244] px-3 py-1.5">
        <span className="text-xs font-semibold text-[#a6adc8]">{kind === "global" ? "Global Sessions" : "Sessions"}</span>
        <RippleBtn onClick={onNew} className={`rounded px-2 py-0.5 text-xs ${BTN.blue}`}>+ New</RippleBtn>
      </div>
      <div className="max-h-44 overflow-y-auto">
        {sessions.length === 0 && <div className="px-3 py-3 text-xs text-[#6c7086]">No sessions yet</div>}
        {sessions.map((s, i) => (
          <div key={s.path || i}
            className={`group flex items-center gap-1 px-3 py-1.5 text-xs cursor-pointer ${
              i === activeIdx ? "bg-[#45475a] text-[#cdd6f4]" : "text-[#a6adc8] hover:bg-[#313244]"
            }`}
            onClick={() => { onSwitch(i); setOpen(false); }}>
            {renamingIdx === i ? (
              <input value={renameValue} onChange={(e) => setRenameValue(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter") commitRename(); if (e.key === "Escape") setRenamingIdx(-1); }}
                onBlur={commitRename} autoFocus onClick={(e) => e.stopPropagation()}
                className="flex-1 rounded bg-[#1e1e2e] px-1 py-0.5 text-xs text-[#cdd6f4] outline-none" />
            ) : (
              <span className="flex-1 truncate" onDoubleClick={() => startRename(i, s.title)} title={s.title}>{s.title}</span>
            )}
            <span className="text-[#6c7086]">{s.messages.length}</span>
            <RippleBtn onClick={(e) => { e.stopPropagation(); startRename(i, s.title); }}
              className="hidden group-hover:inline text-[#6c7086] hover:text-[#cdd6f4]" title="Rename">✎</RippleBtn>
            <RippleBtn onClick={(e) => { e.stopPropagation(); onRegenTitle?.(i); }}
              className="hidden group-hover:inline text-[#6c7086] hover:text-[#f9e2af]" title="Regenerate title">↻</RippleBtn>
            <RippleBtn onClick={(e) => { e.stopPropagation(); onDelete(i); }}
              className="hidden group-hover:inline text-[#6c7086] hover:text-[#f38ba8]" title="Delete">×</RippleBtn>
          </div>
        ))}
      </div>
    </div>
  );

  if (compact) {
    return (
      <>
        <span ref={btnRef}>
          <RippleBtn onClick={handleOpen}
            className="flex items-center gap-1.5 rounded bg-[#313244] px-2 py-0.5 text-xs text-[#a6adc8] hover:bg-[#45475a] hover:text-[#cdd6f4] transition-colors">
            <svg className="h-3 w-3" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
              <rect x="2" y="3" width="12" height="10" rx="1" />
              <line x1="2" y1="7" x2="14" y2="7" />
              <line x1="8" y1="7" x2="8" y2="13" />
            </svg>
            {sessions.length}
          </RippleBtn>
        </span>
        <Dropdown open={open} onClose={() => setOpen(false)} anchorRef={btnRef} width={264}>
          <div className="w-64 overflow-hidden rounded-lg">
            {sessionList}
          </div>
        </Dropdown>
      </>
    );
  }

  return sessionList;
}
