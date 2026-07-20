import { useState, useEffect, useCallback, useRef } from "react";
import { useStore, DEFAULT_GLOBAL_SELECTION } from "../store";
import { invoke } from "@tauri-apps/api/core";
import { useChatSessions } from "../hooks/useChatSessions";
import ChatSessionBar from "./ChatSessionBar";
import { useShallow } from "zustand/react/shallow";
import RippleBtn from "./Ripple";
import ChatControls from "./ChatControls";
import ToggleChips from "./ToggleChips";
import ChatMessages from "./ChatMessages";
import Dialog from "./Dialog";

const ALL_FIELDS = ["body", "appendix", "description", "note"] as const;
const EMPTY_SELECTION = { body: false, appendix: false, description: false, note: false };

export default function GlobalChat() {
  const { globalChatOpen, globalChatModel, globalReasoningEffort, toggleGlobalChat, papers, workspacePath, globalContextSelection, setGlobalSelection, setGlobalChatModel, setGlobalReasoningEffort } = useStore(
    useShallow((s) => ({
      globalChatOpen: s.globalChatOpen,
      globalChatModel: s.globalChatModel,
      globalReasoningEffort: s.globalReasoningEffort,
      toggleGlobalChat: s.toggleGlobalChat,
      papers: s.papers,
      workspacePath: s.workspacePath,
      globalContextSelection: s.globalContextSelection,
      setGlobalSelection: s.setGlobalSelection,
      setGlobalChatModel: s.setGlobalChatModel,
      setGlobalReasoningEffort: s.setGlobalReasoningEffort,
    }))
  );

  const sessionDir = workspacePath ? `${workspacePath}/arxivcat_global_chats` : null;

  const {
    sessions, activeIdx, messages, streaming, status, localBuffer,
    newSession, switchSession, renameSession, deleteSession,
    sendMessage, cancelChat, generateTitle,
    lockedFields, lockFields,
  } = useChatSessions(sessionDir, globalChatModel, globalReasoningEffort);

  const [input, setInput] = useState("");
  const [paperContents, setPaperContents] = useState<Record<string, Record<string, string>>>({});
  const [showCtx, setShowCtx] = useState(true);

  const restoredSessionKey = useRef<string | null>(null);

  // Restore context & locked fields when switching to an existing session
  useEffect(() => {
    if (activeIdx < 0 || !sessions[activeIdx] || papers.length === 0) return;
    const session = sessions[activeIdx];
    const key = `${activeIdx}:${session.path}`;
    if (restoredSessionKey.current === key) return;
    restoredSessionKey.current = key;

    // Restore context_selection: apply flat session selection to ALL papers
    const flatSel = session.context_selection;
    for (const p of papers) {
      setGlobalSelection(p.folder_name, {
        body: flatSel.body ?? false,
        appendix: flatSel.appendix ?? false,
        description: flatSel.description ?? false,
        note: flatSel.note ?? false,
      });
    }
  }, [activeIdx, sessions, papers]);

  // Initialize context for papers that don't have an entry yet (e.g. newly added papers)
  useEffect(() => {
    for (const p of papers) {
      if (!globalContextSelection[p.folder_name]) {
        setGlobalSelection(p.folder_name, { ...EMPTY_SELECTION });
      }
    }
  }, [papers]);

  // Load paper contents
  useEffect(() => {
    const loadAll = async () => {
      const contents: Record<string, Record<string, string>> = {};
      for (const p of papers) {
        if (!workspacePath) continue;
        try {
          const c = await invoke<Record<string, string>>("load_paper", {
            workspacePath,
            folderName: p.folder_name,
          });
          contents[p.folder_name] = c;
        } catch {}
      }
      setPaperContents(contents);
    };
    if (workspacePath && papers.length > 0) loadAll();
  }, [papers, workspacePath]);

  const buildGlobalContext = useCallback((): string => {
    const parts: string[] = [];
    for (const p of papers) {
      const sel = globalContextSelection[p.folder_name] || DEFAULT_GLOBAL_SELECTION;
      const content = paperContents[p.folder_name];
      if (!content) continue;
      const sections: string[] = [];
      if (sel.body && content["body"]) sections.push(`body:\n${content["body"]}`);
      if (sel.appendix && content["appendix"]) sections.push(`appendix:\n${content["appendix"]}`);
      if (sel.description && content["description"]) sections.push(`description:\n${content["description"]}`);
      if (sel.note && content["note"]) sections.push(`note:\n${content["note"]}`);
      if (sections.length === 0) continue;
      parts.push(`Paper [${papers.indexOf(p) + 1}]\narXiv ID: ${p.arxiv_id}\nTitle: ${p.title}\n---\n${sections.join("\n\n")}`);
    }
    return parts.join("\n\n---\n\n");
  }, [papers, globalContextSelection, paperContents]);

  const resetContext = useCallback(() => {
    for (const p of papers) setGlobalSelection(p.folder_name, { ...EMPTY_SELECTION });
  }, [papers]);

  const handleSend = useCallback(async () => {
    if (!input.trim() || streaming) return;
    if (activeIdx < 0) { resetContext(); newSession("global"); }
    const msg = input;
    setInput("");
    const next: Record<string, string[]> = {};
    for (const p of papers) {
      const sel = globalContextSelection[p.folder_name] || DEFAULT_GLOBAL_SELECTION;
      const active = ALL_FIELDS.filter((k) => sel[k]);
      if (active.length > 0) next[p.folder_name] = active;
    }
    lockFields(next);
    await sendMessage(msg, buildGlobalContext());
  }, [input, streaming, buildGlobalContext, sendMessage, papers, globalContextSelection, activeIdx, newSession, resetContext, lockFields]);

  const handleNew = useCallback(() => {
    resetContext();
    newSession("global");
  }, [resetContext, newSession]);

  return (
    <Dialog open={globalChatOpen} onClose={toggleGlobalChat}
      title={<><span>Global Chat</span><span className="text-xs text-[#6c7086] ml-2">{papers.length} papers</span></>}
      defaultWidth={700} defaultHeight={600}
      headerExtra={
        <>
          {activeIdx >= 0 && sessions[activeIdx] && (
            <span className="max-w-32 truncate text-xs text-[#a6adc8]" title={sessions[activeIdx].title}>{sessions[activeIdx].title}</span>
          )}
          <ChatSessionBar
            sessions={sessions}
            activeIdx={activeIdx}
            onNew={handleNew}
            onSwitch={switchSession}
            onRename={renameSession}
            onDelete={deleteSession}
            onRegenTitle={(i) => generateTitle(sessions[i]?.messages ?? messages, i)}
            kind="global"
            compact
          />
          <ChatControls
            model={globalChatModel}
            effort={globalReasoningEffort}
            onModelChange={setGlobalChatModel}
            onEffortChange={(e) => setGlobalReasoningEffort(e as typeof globalReasoningEffort)}
          />
          <RippleBtn onClick={() => setShowCtx(!showCtx)}
            className={`rounded px-2 py-0.5 text-xs transition-colors ${
              showCtx ? "bg-[#89b4fa] text-[#1e1e2e]" : "bg-[#313244] text-[#a6adc8] hover:text-[#cdd6f4]"
            }`}>
            Ctx
          </RippleBtn>
        </>
      }>
      {showCtx && (
        <div className="max-h-[40%] overflow-y-auto border-b border-[#313244] px-4 py-2">
          <div className="mb-2 flex items-center gap-2 text-xs font-semibold text-[#a6adc8]">
            Context
            <div className="flex gap-1 ml-auto">
                {ALL_FIELDS.map((field) => {
                  const allOn = papers.every((p) => {
                    const sel = globalContextSelection[p.folder_name] || DEFAULT_GLOBAL_SELECTION;
                    return sel[field];
                  });
                  const allLocked = papers.length > 0 && papers.every((p) => (lockedFields[p.folder_name] || []).includes(field));
                  return (
                    <button key={field} onClick={() => {
                      if (allLocked) return;
                      for (const p of papers) {
                        setGlobalSelection(p.folder_name, { ...(globalContextSelection[p.folder_name] || DEFAULT_GLOBAL_SELECTION), [field]: !allOn });
                      }
                    }}
                      className={`rounded px-2 py-0.5 text-xs transition-colors ${allLocked ? "bg-[#89b4fa] text-[#1e1e2e] opacity-70 cursor-default" : allOn ? "bg-[#89b4fa] text-[#1e1e2e]" : "bg-[#313244] text-[#a6adc8]"}`}>
                      All {field.charAt(0).toUpperCase() + field.slice(1)}
                    </button>
                  );
                })}
            </div>
          </div>
          {papers.map((p) => {
            const sel = globalContextSelection[p.folder_name] || { ...DEFAULT_GLOBAL_SELECTION };
            const paperLocked = lockedFields[p.folder_name] || [];
            return (
              <div key={p.folder_name} className="mb-1.5 flex items-center gap-2 text-xs">
                <span className="w-28 truncate text-[#89b4fa]" title={`${p.arxiv_id} | ${p.title}`}>{p.arxiv_id}</span>
                <ToggleChips
                  options={[
                    { key: "body", label: "Body" },
                    { key: "appendix", label: "Appendix" },
                    { key: "description", label: "Description" },
                    { key: "note", label: "Note" },
                  ]}
                  selection={{ ...sel, ...Object.fromEntries(paperLocked.map((k) => [k, true])) }}
                  locked={paperLocked}
                    onChange={(key) => { if (!paperLocked.includes(key)) setGlobalSelection(p.folder_name, { ...sel, [key]: !sel[key] }); }}
                />
              </div>
            );
          })}
        </div>
      )}

      <ChatMessages
        messages={messages}
        streaming={streaming}
        status={status}
        localBuffer={localBuffer}
        input={input}
        onInputChange={setInput}
        onSend={handleSend}
        onCancel={cancelChat}
        onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSend(); } }}
        placeholder="Ask about all papers..."
        emptyLabel="Ask questions about all papers in the workspace"
      />
    </Dialog>
  );
}
