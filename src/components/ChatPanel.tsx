import { useState, useRef, useEffect, useCallback } from "react";
import { useStore, ContextSelection } from "../store";
import { useChatSessions } from "../hooks/useChatSessions";
import ChatSessionBar from "./ChatSessionBar";
import ReactMarkdown from "react-markdown";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import { useShallow } from "zustand/react/shallow";
import RippleBtn from "./Ripple";

export default function ChatPanel() {
  const { workspacePath, currentPaper, chatModel, deepThinking, sideContextSelection, previewContent, setChatModel, toggleDeepThinking, setSideSelection } = useStore(
    useShallow((s) => ({
      workspacePath: s.workspacePath,
      currentPaper: s.currentPaper,
      chatModel: s.chatModel,
      deepThinking: s.deepThinking,
      sideContextSelection: s.sideContextSelection,
      previewContent: s.previewContent,
      setChatModel: s.setChatModel,
      toggleDeepThinking: s.toggleDeepThinking,
      setSideSelection: s.setSideSelection,
    }))
  );

  const sessionDir = workspacePath && currentPaper
    ? `${workspacePath}/${currentPaper.folder_name}/arxiv_chats`
    : null;

  const {
    sessions, activeIdx, messages, streaming, status, localBuffer,
    newSession, switchSession, renameSession, deleteSession,
    sendMessage, cancelChat,
  } = useChatSessions(sessionDir, chatModel, deepThinking);

  const [input, setInput] = useState("");
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, localBuffer]);

  const handleSend = useCallback(async () => {
    if (!input.trim() || streaming) return;
    const msg = input;
    setInput("");
    const context = buildContextString(previewContent, sideContextSelection);
    await sendMessage(msg, context);
  }, [input, streaming, previewContent, sideContextSelection, sendMessage]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const toggleContextField = (field: keyof ContextSelection) => {
    setSideSelection({
      ...sideContextSelection,
      [field]: !sideContextSelection[field],
    });
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 border-b border-[#313244] px-3 py-2">
        <span className="text-xs font-semibold text-[#a6adc8]">Side Chat</span>
        <div className="flex-1" />
        <select
          value={chatModel}
          onChange={(e) => setChatModel(e.target.value)}
          className="rounded bg-[#313244] px-2 py-0.5 text-xs text-[#cdd6f4] outline-none"
        >
          <option value="Flash">Flash</option>
          <option value="Pro">Pro</option>
        </select>
        <RippleBtn
          onClick={toggleDeepThinking}
          className={`rounded px-2 py-0.5 text-xs transition-colors duration-150 ${
            deepThinking
              ? "bg-[#89b4fa] text-[#1e1e2e]"
              : "bg-[#313244] text-[#a6adc8]"
          }`}
        >
          Deep
        </RippleBtn>
      </div>

      <ChatSessionBar
        sessions={sessions}
        activeIdx={activeIdx}
        onNew={() => newSession("paper")}
        onSwitch={switchSession}
        onRename={renameSession}
        onDelete={deleteSession}
        kind="paper"
      />

      <div className="flex gap-2 border-b border-[#313244] px-3 py-1.5">
        {(["body", "appendix", "description", "note"] as const).map((field) => (
          <label key={field} className="flex items-center gap-1 text-xs text-[#a6adc8] cursor-pointer">
            <input
              type="checkbox"
              checked={sideContextSelection[field]}
              onChange={() => toggleContextField(field)}
              className="accent-[#89b4fa]"
            />
            {field.charAt(0).toUpperCase() + field.slice(1)}
          </label>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto p-3">
        {messages.length === 0 && !streaming && (
          <div className="py-8 text-center text-xs text-[#6c7086]">
            Ask questions about this paper
          </div>
        )}
        {messages.map((m, i) => (
          <div key={i} className={`mb-3 ${m.speaker === "user" ? "text-right" : ""}`}>
            <div
              className={`inline-block max-w-[90%] rounded-lg px-3 py-2 text-sm ${
                m.speaker === "user"
                  ? "bg-[#89b4fa] text-[#1e1e2e]"
                  : "bg-[#313244] text-[#cdd6f4]"
              }`}
            >
              {m.speaker === "user" ? (
                <pre className="whitespace-pre-wrap font-sans">{m.content}</pre>
              ) : (
                <div className="prose prose-sm prose-invert max-w-none [&_.katex-display]:my-2 [&_.katex]:text-inherit [&_p]:leading-relaxed [&_code]:bg-[#45475a] [&_code]:rounded [&_code]:px-1 [&_pre]:bg-[#11111b] [&_pre]:rounded [&_pre]:p-2 [&_pre]:overflow-x-auto [&_ul]:list-disc [&_ul]:pl-4 [&_ol]:list-decimal [&_ol]:pl-4 [&_a]:text-[#89b4fa]">
                  <ReactMarkdown remarkPlugins={[remarkMath]} rehypePlugins={[rehypeKatex]}>
                    {m.content}
                  </ReactMarkdown>
                </div>
              )}
            </div>
          </div>
        ))}
        {streaming && localBuffer && (
          <div className="mb-3">
            <div className="inline-block max-w-[90%] rounded-lg bg-[#313244] px-3 py-2 text-sm text-[#cdd6f4]">
              <pre className="whitespace-pre-wrap font-sans">{localBuffer}</pre>
            </div>
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      {status && (
        <div className="px-3 py-1 text-xs text-[#f9e2af]">{status}</div>
      )}

      <div className="border-t border-[#313244] p-2">
        <div className="flex gap-2">
          <input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Ask about this paper..."
            disabled={streaming}
            className="flex-1 rounded bg-[#313244] px-3 py-1.5 text-sm text-[#cdd6f4] outline-none disabled:opacity-50"
          />
          {streaming ? (
            <RippleBtn
              onClick={cancelChat}
              className="rounded bg-[#f38ba8] px-4 py-1.5 text-sm text-[#1e1e2e]"
            >
              Stop
            </RippleBtn>
          ) : (
            <RippleBtn
              onClick={handleSend}
              disabled={!input.trim()}
              className="rounded bg-[#89b4fa] px-4 py-1.5 text-sm text-[#1e1e2e] disabled:opacity-50"
            >
              Send
            </RippleBtn>
          )}
        </div>
      </div>
    </div>
  );
}

function buildContextString(
  content: Record<string, string>,
  sel: ContextSelection
): string {
  const parts: string[] = [];
  if (sel.body && content["body"]) parts.push(`body:\n${content["body"]}`);
  if (sel.appendix && content["appendix"])
    parts.push(`appendix:\n${content["appendix"]}`);
  if (sel.description && content["description"])
    parts.push(`description:\n${content["description"]}`);
  if (sel.note && content["note"]) parts.push(`note:\n${content["note"]}`);
  return parts.join("\n\n");
}
