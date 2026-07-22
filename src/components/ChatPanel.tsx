import { useState, useCallback, useEffect } from "react";
import { useStore, DEFAULT_SIDE_SELECTION, BTN } from "../store";
import { useChatSessions } from "../hooks/useChatSessions";
import { useContextRestore } from "../hooks/useContextRestore";
import ChatSessionBar from "./ChatSessionBar";
import { useShallow } from "zustand/react/shallow";
import RippleBtn from "./Ripple";
import ChatTitleBar from "./ChatTitleBar";
import ChatControls from "./ChatControls";
import ToggleChips from "./ToggleChips";
import ChatMessages from "./ChatMessages";

export default function ChatPanel() {
  const { workspacePath, currentPaper, sideChatModel, sideReasoningEffort, sideContextSelection, previewContent, setSideChatModel, setSideReasoningEffort, setSideSelection } = useStore(
    useShallow((s) => ({
      workspacePath: s.workspacePath,
      currentPaper: s.currentPaper,
      sideChatModel: s.sideChatModel,
      sideReasoningEffort: s.sideReasoningEffort,
      sideContextSelection: s.sideContextSelection,
      previewContent: s.previewContent,
      setSideChatModel: s.setSideChatModel,
      setSideReasoningEffort: s.setSideReasoningEffort,
      setSideSelection: s.setSideSelection,
    }))
  );

  const sessionDir = workspacePath && currentPaper
    ? `${workspacePath}/${currentPaper.folder_name}/arxiv_chats`
    : null;

  const {
    sessions, activeIdx, messages, streaming, status, localBuffer,
    newSession, switchSession, renameSession, deleteSession,
    sendMessage, cancelChat, lockedFields, lockFields, generateTitle,
  } = useChatSessions(sessionDir, sideChatModel, sideReasoningEffort);

  const folderName = currentPaper?.folder_name || "";
  const paperLockedFields = lockedFields[folderName] || [];

  const [input, setInput] = useState("");
  const [showCtx, setShowCtx] = useState(true);

  useContextRestore(activeIdx, sessions, "side", (session) => {
    if (session.context_selection) {
      const sel = session.context_selection;
      setSideSelection({ body: !!sel.body, appendix: !!sel.appendix, description: !!sel.description, note: !!sel.note });
    }
  });

  useEffect(() => {
    setSideSelection({ ...DEFAULT_SIDE_SELECTION });
  }, [sessionDir]);

  const handleSend = useCallback(async () => {
    if (!input.trim() || streaming) return;
    if (activeIdx < 0) newSession("paper");
    const msg = input;
    setInput("");
    const parts: string[] = [];
    if (sideContextSelection.body && previewContent["body"]) parts.push(`body:\n${previewContent["body"]}`);
    if (sideContextSelection.appendix && previewContent["appendix"]) parts.push(`appendix:\n${previewContent["appendix"]}`);
    if (sideContextSelection.description && previewContent["description"]) parts.push(`description:\n${previewContent["description"]}`);
    if (sideContextSelection.note && previewContent["note"]) parts.push(`note:\n${previewContent["note"]}`);
    const activeFields = (["body", "appendix", "description", "note"] as const).filter(
      (k) => sideContextSelection[k]
    );
    if (activeFields.length > 0 && folderName) {
      lockFields({ [folderName]: activeFields });
    }
    await sendMessage(msg, parts.join("\n\n"));
  }, [input, streaming, previewContent, sideContextSelection, sendMessage, lockFields, activeIdx, newSession, folderName]);

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 border-b border-[#313244] px-3 py-2">
        <span className="text-xs font-semibold text-[#a6adc8] whitespace-nowrap">Side Chat</span>
        <ChatSessionBar
          sessions={sessions}
          activeIdx={activeIdx}
          onNew={() => newSession("paper")}
          onSwitch={switchSession}
          onRename={renameSession}
          onDelete={deleteSession}
          onRegenTitle={(i) => generateTitle(sessions[i]?.messages ?? messages, i)}
          kind="paper"
          compact
        />
        <div className="flex-1" />
        <ChatControls
          model={sideChatModel}
          effort={sideReasoningEffort}
          onModelChange={setSideChatModel}
          onEffortChange={(e) => setSideReasoningEffort(e as typeof sideReasoningEffort)}
        />
        <RippleBtn onClick={() => setShowCtx(!showCtx)}
          className={`rounded px-2 py-0.5 text-xs ${
            showCtx ? BTN.blue : BTN.surface0
          }`}>
          Ctx
        </RippleBtn>
      </div>

      {showCtx && (
        <div className="border-b border-[#313244] px-3 py-1.5">
          <ToggleChips
            options={[
              { key: "body", label: "Body" },
              { key: "appendix", label: "Appendix" },
              { key: "description", label: "Description" },
              { key: "note", label: "Note" },
            ]}
            selection={{ ...sideContextSelection, ...Object.fromEntries(paperLockedFields.map((k) => [k, true])) }}
            locked={paperLockedFields}
            onChange={(key) => { if (!paperLockedFields.includes(key)) setSideSelection({ ...sideContextSelection, [key]: !sideContextSelection[key] }); }}
          />
        </div>
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
        placeholder="Ask about this paper..."
        emptyLabel="Ask questions about this paper"
      />
    </div>
  );
}
