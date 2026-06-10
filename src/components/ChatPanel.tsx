import { useState, useRef, useEffect } from "react";
import { useStore } from "../store";

export default function ChatPanel() {
  const {
    chatMessages,
    chatModel,
    deepThinking,
    addChatMessage,
    clearChat,
    setChatModel,
    toggleDeepThinking,
  } = useStore();

  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [status, setStatus] = useState("");
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [chatMessages]);

  const handleSend = async () => {
    if (!input.trim() || streaming) return;
    const msg = input;
    setInput("");
    addChatMessage({ speaker: "user", content: msg });

    setStreaming(true);
    setStatus("thinking...");

    try {
      const response = await fetch("https://api.deepseek.com/chat/completions", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${localStorage.getItem("deepseek_token") || ""}`,
        },
        body: JSON.stringify({
          model: chatModel === "Flash" ? "deepseek-v4-flash" : "deepseek-v4-pro",
          messages: [
            { role: "system", content: "You are a helpful assistant discussing an arXiv paper." },
            ...chatMessages.map((m) => ({
              role: m.speaker === "user" ? "user" : "assistant",
              content: m.content,
            })),
            { role: "user", content: msg },
          ],
          stream: true,
        }),
      });

      if (!response.ok) {
        setStatus(`API error: ${response.status}`);
        setStreaming(false);
        return;
      }

      const reader = response.body?.getReader();
      const decoder = new TextDecoder();
      let fullText = "";

      if (reader) {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          const text = decoder.decode(value, { stream: true });
          for (const line of text.split("\n")) {
            const data = line.trim();
            if (!data || data === "data: [DONE]") continue;
            if (data.startsWith("data: ")) {
              try {
                const json = JSON.parse(data.slice(6));
                const content = json.choices?.[0]?.delta?.content;
                if (content) {
                  fullText += content;
                }
              } catch {}
            }
          }
        }
      }

      if (fullText) {
        addChatMessage({ speaker: "assistant", content: fullText });
      }
      setStatus("");
    } catch (e) {
      setStatus(`error: ${e}`);
    }
    setStreaming(false);
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

      <div className="flex-1 overflow-y-auto p-3">
        {chatMessages.length === 0 && (
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
            onKeyDown={(e) => e.key === "Enter" && !e.shiftKey && handleSend()}
            placeholder="Ask about this paper..."
            disabled={streaming}
            className="flex-1 rounded bg-[#313244] px-3 py-1.5 text-sm text-[#cdd6f4] outline-none disabled:opacity-50"
          />
          <button
            onClick={handleSend}
            disabled={streaming || !input.trim()}
            className="rounded bg-[#89b4fa] px-4 py-1.5 text-sm text-[#1e1e2e] disabled:opacity-50"
          >
            {streaming ? "..." : "Send"}
          </button>
        </div>
      </div>
    </div>
  );
}
