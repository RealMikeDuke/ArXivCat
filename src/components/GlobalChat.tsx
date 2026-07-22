import { useState, useEffect, useCallback } from "react";
import { useStore, DEFAULT_GLOBAL_SELECTION, BTN } from "../store";
import { invoke } from "@tauri-apps/api/core";
import { useChatSessions } from "../hooks/useChatSessions";
import { useContextRestore } from "../hooks/useContextRestore";
import ChatSessionBar from "./ChatSessionBar";
import ChatTitleBar from "./ChatTitleBar";
import ContextSelector from "./ContextSelector";
import { useShallow } from "zustand/react/shallow";
import RippleBtn from "./Ripple";
import ChatControls from "./ChatControls";
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

  useContextRestore(activeIdx, sessions, "global", (session) => {
    if (!session.context_selection || papers.length === 0) return;
    const flatSel = session.context_selection;
    for (const p of papers) {
      setGlobalSelection(p.folder_name, {
        body: flatSel.body ?? false,
        appendix: flatSel.appendix ?? false,
        description: flatSel.description ?? false,
        note: flatSel.note ?? false,
      });
    }
  }, [papers]);

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
        } catch (e) {
          useStore.getState().addLog(`[ERROR] Failed to load paper content: ${e}`);
        }
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
            className={`rounded px-2 py-0.5 text-xs ${
              showCtx ? BTN.blue : BTN.surface0
            }`}>
            Ctx
          </RippleBtn>
        </>
      }>
      {showCtx && (
        <ContextSelector
          papers={papers}
          selection={globalContextSelection}
          lockedFields={lockedFields}
          onChange={(folder, sel) => setGlobalSelection(folder, sel)}
        />
      )}

      {activeIdx >= 0 && sessions[activeIdx] && (
        <ChatTitleBar title={sessions[activeIdx].title} />
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
