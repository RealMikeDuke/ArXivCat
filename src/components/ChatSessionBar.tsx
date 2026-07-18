import { useState } from "react";
import { ChatSession } from "../hooks/useChatSessions";
import RippleBtn from "./Ripple";

interface Props {
  sessions: ChatSession[];
  activeIdx: number;
  onNew: () => void;
  onSwitch: (idx: number) => void;
  onRename: (idx: number, title: string) => void;
  onDelete: (idx: number) => void;
  kind: "paper" | "global";
}

export default function ChatSessionBar({ sessions, activeIdx, onNew, onSwitch, onRename, onDelete, kind }: Props) {
  const [renamingIdx, setRenamingIdx] = useState(-1);
  const [renameValue, setRenameValue] = useState("");

  const startRename = (idx: number, currentTitle: string) => {
    setRenamingIdx(idx);
    setRenameValue(currentTitle);
  };

  const commitRename = () => {
    if (renamingIdx >= 0 && renameValue.trim()) {
      onRename(renamingIdx, renameValue.trim());
    }
    setRenamingIdx(-1);
  };

  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-1 border-b border-[#313244] px-2 py-1">
        <span className="text-xs font-semibold text-[#a6adc8]">
          {kind === "global" ? "Global Sessions" : "Sessions"}
        </span>
        <div className="flex-1" />
        <RippleBtn
          onClick={onNew}
          className="rounded bg-[#89b4fa] px-2 py-0.5 text-xs text-[#1e1e2e] hover:bg-[#b4d0fb]"
          title="New session"
        >
          + New
        </RippleBtn>
      </div>
      <div className="max-h-40 overflow-y-auto">
        {sessions.length === 0 && (
          <div className="px-3 py-2 text-xs text-[#6c7086]">No sessions yet</div>
        )}
        {sessions.map((s, i) => (
          <div
            key={s.path || i}
            className={`group flex items-center gap-1 px-2 py-1 text-xs cursor-pointer ${
              i === activeIdx
                ? "bg-[#45475a] text-[#cdd6f4]"
                : "text-[#a6adc8] hover:bg-[#313244]"
            }`}
            onClick={() => onSwitch(i)}
          >
            {renamingIdx === i ? (
              <input
                value={renameValue}
                onChange={(e) => setRenameValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") commitRename();
                  if (e.key === "Escape") setRenamingIdx(-1);
                }}
                onBlur={commitRename}
                className="flex-1 rounded bg-[#1e1e2e] px-1 py-0.5 text-xs text-[#cdd6f4] outline-none"
                autoFocus
                onClick={(e) => e.stopPropagation()}
              />
            ) : (
              <span
                className="flex-1 truncate"
                onDoubleClick={() => startRename(i, s.title)}
                title={s.title}
              >
                {s.title}
              </span>
            )}
            <span className="text-[#6c7086]">{s.messages.length}</span>
            <RippleBtn
              onClick={(e) => { e.stopPropagation(); startRename(i, s.title); }}
              className="hidden group-hover:inline text-[#6c7086] hover:text-[#cdd6f4]"
              title="Rename"
            >
              ✎
            </RippleBtn>
            <RippleBtn
              onClick={(e) => { e.stopPropagation(); onDelete(i); }}
              className="hidden group-hover:inline text-[#6c7086] hover:text-[#f38ba8]"
              title="Delete"
            >
              ×
            </RippleBtn>
          </div>
        ))}
      </div>
    </div>
  );
}
