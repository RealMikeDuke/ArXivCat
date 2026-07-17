import { useState, useRef, useEffect, useCallback } from "react";
import { useStore, DEFAULT_GLOBAL_SELECTION } from "../store";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export default function GlobalChat() {
  const {
    chatModel,
    toggleGlobalChat,
    papers,
    workspacePath,
    globalContextSelection,
    setGlobalSelection,
  } = useStore();

  const [messages, setMessages] = useState<{ speaker: string; content: string }[]>([]);
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [status, setStatus] = useState("");
  const [localBuffer, setLocalBuffer] = useState("");
  const [paperContents, setPaperContents] = useState<
    Record<string, Record<string, string>>
  >({});
  const setTokenCount = useState(0)[1];
  const [showConfig, setShowConfig] = useState(true);
  const bottomRef = useRef<HTMLDivElement>(null);
  const bufferRef = useRef(localBuffer);
  bufferRef.current = localBuffer;

  // Load paper contents on mount
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

      // init default selections
      for (const p of papers) {
        if (!globalContextSelection[p.folder_name]) {
          setGlobalSelection(p.folder_name, { ...DEFAULT_GLOBAL_SELECTION });
        }
      }
    };
    loadAll();
  }, [papers, workspacePath]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, localBuffer]);

  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];

    const setup = async () => {
      const unlistenToken = await listen<{ session_id: string; token: string }>("chat:token", (e) => {
        if (e.payload.session_id !== sessionId) return;
        setLocalBuffer((prev) => prev + e.payload.token);
        setStatus("");
      });
      unlisteners.push(unlistenToken);

      const unlistenStatus = await listen<{ session_id: string; status: string }>("chat:status", (e) => {
        if (e.payload.session_id !== sessionId) return;
        setStatus(e.payload.status);
      });
      unlisteners.push(unlistenStatus);

      const unlistenDone = await listen<{ session_id: string; text: string }>("chat:done", (e) => {
        if (e.payload.session_id !== sessionId) return;
        const finalText = bufferRef.current;
        if (finalText) {
          setMessages((prev) => [...prev, { speaker: "assistant", content: finalText }]);
        }
        setLocalBuffer("");
        setSessionId(null);
        setStreaming(false);
        setTokenCount(0);
      });
      unlisteners.push(unlistenDone);

      const unlistenError = await listen<{ session_id: string; error: string }>("chat:error", (e) => {
        if (e.payload.session_id !== sessionId) return;
        setLocalBuffer("");
        setSessionId(null);
        setStreaming(false);
        setTokenCount(0);
        setStatus(`error: ${e.payload.error}`);
      });
      unlisteners.push(unlistenError);
    };

    if (sessionId) {
      setup();
    }

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, [sessionId]);

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
      parts.push(
        `Paper [${papers.indexOf(p) + 1}]\narXiv ID: ${p.arxiv_id}\nTitle: ${p.title}\n---\n${sections.join("\n\n")}`
      );
    }
    return parts.join("\n\n---\n\n");
  }, [papers, globalContextSelection, paperContents]);

  const handleSend = useCallback(async () => {
    if (!input.trim() || streaming) return;
    const msg = input;
    setInput("");

    const newMessages = [...messages, { speaker: "user", content: msg }] as {
      speaker: string;
      content: string;
    }[];
    setMessages(newMessages);

    setStreaming(true);
    setStatus("thinking...");
    setShowConfig(false);

    try {
      const apiMessages = newMessages.map((m) => ({
        role: m.speaker === "user" ? "user" : "assistant",
        content: m.content,
      }));
      const context = buildGlobalContext();

      const { session_id } = await invoke<{ session_id: string }>("start_chat", {
        messages: apiMessages,
        model: chatModel,
        deepThinking: true,
        paperContext: context || null,
      });

      setSessionId(session_id);
    } catch (e) {
      setStatus(`error: ${e}`);
      setStreaming(false);
    }
  }, [input, streaming, messages, chatModel, buildGlobalContext]);

  const handleCancel = useCallback(async () => {
    if (sessionId) {
      try {
        await invoke("cancel_chat", { sessionId });
      } catch {}
    }
    setSessionId(null);
    setStreaming(false);
    setStatus("cancelled");
    setLocalBuffer("");
  }, [sessionId]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="flex h-[80vh] w-[700px] max-w-[95vw] flex-col rounded-lg border border-[#45475a] bg-[#1e1e2e] shadow-2xl">
        <div className="flex items-center gap-2 border-b border-[#313244] px-4 py-3">
          <span className="font-semibold text-[#cdd6f4]">Global Chat</span>
          <span className="text-xs text-[#6c7086]">{papers.length} papers</span>
          <div className="flex-1" />
          <button
            onClick={() => setShowConfig(!showConfig)}
            className="rounded bg-[#313244] px-2 py-0.5 text-xs text-[#a6adc8] hover:text-[#cdd6f4]"
          >
            {showConfig ? "Hide Config" : "Configure Context"}
          </button>
          <button
            onClick={toggleGlobalChat}
            className="rounded bg-[#313244] px-3 py-1 text-xs text-[#a6adc8] hover:text-[#cdd6f4]"
          >
            Close
          </button>
        </div>

        {showConfig && (
          <div className="max-h-[40%] overflow-y-auto border-b border-[#313244] px-4 py-2">
            <div className="mb-2 text-xs font-semibold text-[#a6adc8]">
              Per-paper context selection (default: description only)
            </div>
            <div className="mb-2 flex gap-3 text-xs text-[#6c7086]">
              <span>Body</span>
              <span>Appendix</span>
              <span>Description</span>
              <span>Note</span>
            </div>
            {papers.map((p) => {
              const sel = globalContextSelection[p.folder_name] || { ...DEFAULT_GLOBAL_SELECTION };
              return (
                <div key={p.folder_name} className="mb-1 flex items-center gap-2 text-xs">
                  <span className="w-28 truncate text-[#89b4fa]" title={`${p.arxiv_id} | ${p.title}`}>
                    {p.arxiv_id}
                  </span>
                  {(["body", "appendix", "description", "note"] as const).map((field) => (
                    <label key={field} className="flex items-center gap-1 text-[#a6adc8] cursor-pointer">
                      <input
                        type="checkbox"
                        checked={sel[field]}
                        onChange={() => {
                          setGlobalSelection(p.folder_name, {
                            ...sel,
                            [field]: !sel[field],
                          });
                        }}
                        className="accent-[#89b4fa]"
                      />
                    </label>
                  ))}
                </div>
              );
            })}
          </div>
        )}

        <div className="flex-1 overflow-y-auto p-4">
          {messages.map((m, i) => (
            <div key={i} className={`mb-3 ${m.speaker === "user" ? "text-right" : ""}`}>
              <div
                className={`inline-block max-w-[85%] rounded-lg px-3 py-2 text-sm ${
                  m.speaker === "user"
                    ? "bg-[#89b4fa] text-[#1e1e2e]"
                    : "bg-[#313244] text-[#cdd6f4]"
                }`}
              >
                <pre className="whitespace-pre-wrap font-sans">{m.content}</pre>
              </div>
            </div>
          ))}
          {streaming && localBuffer && (
            <div className="mb-3">
              <div className="inline-block max-w-[85%] rounded-lg bg-[#313244] px-3 py-2 text-sm text-[#cdd6f4]">
                <pre className="whitespace-pre-wrap font-sans">{localBuffer}</pre>
              </div>
            </div>
          )}
          <div ref={bottomRef} />
        </div>

        {status && (
          <div className="px-4 py-1 text-xs text-[#f9e2af]">{status}</div>
        )}

        <div className="border-t border-[#313244] p-3">
          <div className="flex gap-2">
            <input
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && !e.shiftKey && handleSend()}
              placeholder="Ask about all papers..."
              disabled={streaming}
              className="flex-1 rounded bg-[#313244] px-3 py-2 text-sm text-[#cdd6f4] outline-none"
            />
            {streaming ? (
              <button
                onClick={handleCancel}
                className="rounded bg-[#f38ba8] px-4 py-2 text-sm text-[#1e1e2e]"
              >
                Stop
              </button>
            ) : (
              <button
                onClick={handleSend}
                disabled={streaming || !input.trim()}
                className="rounded bg-[#89b4fa] px-4 py-2 text-sm text-[#1e1e2e] disabled:opacity-50"
              >
                Send
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
