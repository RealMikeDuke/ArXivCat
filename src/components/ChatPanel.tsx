import { useState, useRef, useEffect, useCallback } from "react";
import { useStore, ContextSelection } from "../store";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export default function ChatPanel() {
  const {
    chatMessages,
    chatModel,
    deepThinking,
    sideContextSelection,
    chat,
    previewContent,
    addChatMessage,
    clearChat,
    setChatModel,
    toggleDeepThinking,
    setSideSelection,
    sendChat,
    cancelChat,
  } = useStore();

  const [input, setInput] = useState("");
  const [localBuffer, setLocalBuffer] = useState("");
  const setTokenCount = useState(0)[1];
  const bottomRef = useRef<HTMLDivElement>(null);
  const localRef = useRef(localBuffer);
  localRef.current = localBuffer;

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [chatMessages, localBuffer]);

  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];

    const setup = async () => {
      const unlistenToken = await listen<{ session_id: string; token: string }>("chat:token", (e) => {
        const { sessionId } = useStore.getState().chat;
        if (e.payload.session_id !== sessionId) return;
        setLocalBuffer((prev) => prev + e.payload.token);
        useStore.setState((_) => ({
          chat: { ...useStore.getState().chat, status: "" },
        }));
      });
      unlisteners.push(unlistenToken);

      const unlistenStatus = await listen<{ session_id: string; status: string }>("chat:status", (e) => {
        const { sessionId } = useStore.getState().chat;
        if (e.payload.session_id !== sessionId) return;
        useStore.setState((_) => ({
          chat: { ...useStore.getState().chat, status: e.payload.status },
        }));
      });
      unlisteners.push(unlistenStatus);

      const unlistenDone = await listen<{ session_id: string; text: string }>("chat:done", (e) => {
        const { sessionId, bufferTokens } = useStore.getState().chat;
        if (e.payload.session_id !== sessionId) return;
        const finalText = (localRef.current + (bufferTokens.join("")));
        if (finalText) {
          useStore.getState().addChatMessage({ speaker: "assistant", content: finalText });
        }
        setLocalBuffer("");
        setTokenCount(0);
        useStore.setState((_) => ({
          chat: { sessionId: null, streaming: false, status: "", bufferTokens: [] },
        }));
      });
      unlisteners.push(unlistenDone);

      const unlistenError = await listen<{ session_id: string; error: string }>("chat:error", (e) => {
        const { sessionId } = useStore.getState().chat;
        if (e.payload.session_id !== sessionId) return;
        setLocalBuffer("");
        setTokenCount(0);
        useStore.setState((_) => ({
          chat: { sessionId: null, streaming: false, status: `error: ${e.payload.error}`, bufferTokens: [] },
        }));
      });
      unlisteners.push(unlistenError);
    };

    setup();

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  const handleSend = useCallback(async () => {
    if (!input.trim() || chat.streaming) return;
    const msg = input;
    setInput("");

    addChatMessage({ speaker: "user", content: msg });
    const context = buildContextString(previewContent, sideContextSelection);

    const newMessages = [
      ...chatMessages,
      { speaker: "user", content: msg },
    ];

    await sendChat(newMessages, context);
  }, [input, chat.streaming, chatMessages, previewContent, sideContextSelection, addChatMessage, sendChat]);

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
        <span className="text-xs font-semibold text-[#a6adc8]">Chat</span>
        <div className="flex-1" />
        <select
          value={chatModel}
          onChange={(e) => setChatModel(e.target.value)}
          className="rounded bg-[#313244] px-2 py-0.5 text-xs text-[#cdd6f4] outline-none"
        >
          <option value="Flash">Flash</option>
          <option value="Pro">Pro</option>
        </select>
        <button
          onClick={toggleDeepThinking}
          className={`rounded px-2 py-0.5 text-xs ${
            deepThinking
              ? "bg-[#89b4fa] text-[#1e1e2e]"
              : "bg-[#313244] text-[#a6adc8]"
          }`}
        >
          Deep
        </button>
        <button
          onClick={clearChat}
          className="rounded bg-[#313244] px-2 py-0.5 text-xs text-[#a6adc8] hover:text-[#cdd6f4]"
        >
          Clear
        </button>
      </div>

      {/* context checkboxes */}
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
        {chatMessages.length === 0 && !chat.streaming && (
          <div className="py-8 text-center text-xs text-[#6c7086]">
            Ask questions about this paper
          </div>
        )}
        {chatMessages.map((m, i) => (
          <div key={i} className={`mb-3 ${m.speaker === "user" ? "text-right" : ""}`}>
            <div
              className={`inline-block max-w-[90%] rounded-lg px-3 py-2 text-sm ${
                m.speaker === "user"
                  ? "bg-[#89b4fa] text-[#1e1e2e]"
                  : "bg-[#313244] text-[#cdd6f4]"
              }`}
            >
              <pre className="whitespace-pre-wrap font-sans">{m.content}</pre>
            </div>
          </div>
        ))}
        {chat.streaming && localBuffer && (
          <div className="mb-3">
            <div className="inline-block max-w-[90%] rounded-lg bg-[#313244] px-3 py-2 text-sm text-[#cdd6f4]">
              <pre className="whitespace-pre-wrap font-sans">{localBuffer}</pre>
            </div>
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      {chat.status && (
        <div className="px-3 py-1 text-xs text-[#f9e2af]">{chat.status}</div>
      )}

      <div className="border-t border-[#313244] p-2">
        <div className="flex gap-2">
          <input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Ask about this paper..."
            disabled={chat.streaming}
            className="flex-1 rounded bg-[#313244] px-3 py-1.5 text-sm text-[#cdd6f4] outline-none disabled:opacity-50"
          />
          {chat.streaming ? (
            <button
              onClick={cancelChat}
              className="rounded bg-[#f38ba8] px-4 py-1.5 text-sm text-[#1e1e2e]"
            >
              Stop
            </button>
          ) : (
            <button
              onClick={handleSend}
              disabled={!input.trim()}
              className="rounded bg-[#89b4fa] px-4 py-1.5 text-sm text-[#1e1e2e] disabled:opacity-50"
            >
              Send
            </button>
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
