import { useState, useRef, useEffect, useCallback } from "react";
import { useStore, DEFAULT_GLOBAL_SELECTION } from "../store";
import { invoke } from "@tauri-apps/api/core";
import { useChatSessions } from "../hooks/useChatSessions";
import ChatSessionBar from "./ChatSessionBar";
import { useShallow } from "zustand/react/shallow";
import ReactMarkdown from "react-markdown";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import RippleBtn from "./Ripple";

export default function GlobalChat() {
  const { chatModel, toggleGlobalChat, papers, workspacePath, globalContextSelection, setGlobalSelection } = useStore(
    useShallow((s) => ({
      chatModel: s.chatModel,
      toggleGlobalChat: s.toggleGlobalChat,
      papers: s.papers,
      workspacePath: s.workspacePath,
      globalContextSelection: s.globalContextSelection,
      setGlobalSelection: s.setGlobalSelection,
    }))
  );

  const sessionDir = workspacePath ? `${workspacePath}/arxivcat_global_chats` : null;

  const {
    sessions, activeIdx, messages, streaming, status, localBuffer,
    newSession, switchSession, renameSession, deleteSession,
    sendMessage, cancelChat,
  } = useChatSessions(sessionDir, chatModel, true);

  const [input, setInput] = useState("");
  const [paperContents, setPaperContents] = useState<Record<string, Record<string, string>>>({});
  const [showConfig, setShowConfig] = useState(true);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, localBuffer]);

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
      for (const p of papers) {
        if (!globalContextSelection[p.folder_name]) {
          setGlobalSelection(p.folder_name, { ...DEFAULT_GLOBAL_SELECTION });
        }
      }
    };
    if (workspacePath && papers.length > 0) {
      loadAll();
    }
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
    setShowConfig(false);
    const context = buildGlobalContext();
    await sendMessage(msg, context);
  }, [input, streaming, buildGlobalContext, sendMessage]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="flex h-[80vh] w-[700px] max-w-[95vw] flex-col rounded-lg border border-[#45475a] bg-[#1e1e2e] shadow-2xl">
        <div className="flex items-center gap-2 border-b border-[#313244] px-4 py-3">
          <span className="font-semibold text-[#cdd6f4]">Global Chat</span>
          <span className="text-xs text-[#6c7086]">{papers.length} papers</span>
          <div className="flex-1" />
          <RippleBtn
            onClick={() => setShowConfig(!showConfig)}
            className="rounded bg-[#313244] px-2 py-0.5 text-xs text-[#a6adc8] hover:text-[#cdd6f4]"
          >
            {showConfig ? "Hide Config" : "Configure Context"}
          </RippleBtn>
          <RippleBtn
            onClick={toggleGlobalChat}
            className="rounded bg-[#313244] px-3 py-1 text-xs text-[#a6adc8] hover:text-[#cdd6f4]"
          >
            Close
          </RippleBtn>
        </div>

        <ChatSessionBar
          sessions={sessions}
          activeIdx={activeIdx}
          onNew={() => newSession("global")}
          onSwitch={switchSession}
          onRename={renameSession}
          onDelete={deleteSession}
          kind="global"
        />

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
              <RippleBtn
                onClick={cancelChat}
                className="rounded bg-[#f38ba8] px-4 py-2 text-sm text-[#1e1e2e]"
              >
                Stop
              </RippleBtn>
            ) : (
              <RippleBtn
                onClick={handleSend}
                disabled={streaming || !input.trim()}
                className="rounded bg-[#89b4fa] px-4 py-2 text-sm text-[#1e1e2e] disabled:opacity-50"
              >
                Send
              </RippleBtn>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
