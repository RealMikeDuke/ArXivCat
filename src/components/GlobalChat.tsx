import { useState, useRef, useEffect } from "react";
import { useStore } from "../store";

export default function GlobalChat() {
  const { chatModel, toggleGlobalChat, papers } = useStore();
  const [messages, setMessages] = useState<{ speaker: string; content: string }[]>([]);
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [status, setStatus] = useState("");
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleSend = async () => {
    if (!input.trim() || streaming) return;
    const msg = input;
    setInput("");
    setMessages((prev) => [...prev, { speaker: "user", content: msg }]);

    setStreaming(true);
    setStatus("thinking...");

    try {
      const ctxParts = papers
        .map((p, i) => `Paper [${i + 1}]\narXiv ID: ${p.arxiv_id}\nTitle: ${p.title}`)
        .join("\n---\n");

      const response = await fetch("https://api.deepseek.com/chat/completions", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${localStorage.getItem("deepseek_token") || ""}`,
        },
        body: JSON.stringify({
          model: chatModel === "Flash" ? "deepseek-v4-flash" : "deepseek-v4-pro",
          messages: [
            {
              role: "system",
              content: `You are a helpful assistant discussing a workspace of arXiv papers.\n\nPapers:\n${ctxParts}`,
            },
            ...messages.map((m) => ({
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
                if (content) fullText += content;
              } catch {}
            }
          }
        }
      }

      if (fullText) {
        setMessages((prev) => [...prev, { speaker: "assistant", content: fullText }]);
      }
      setStatus("");
    } catch (e) {
      setStatus(`error: ${e}`);
    }
    setStreaming(false);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="flex h-[80vh] w-[700px] max-w-[95vw] flex-col rounded-lg border border-[#45475a] bg-[#1e1e2e] shadow-2xl">
        <div className="flex items-center gap-2 border-b border-[#313244] px-4 py-3">
          <span className="font-semibold text-[#cdd6f4]">Global Chat</span>
          <span className="text-xs text-[#6c7086]">{papers.length} papers</span>
          <div className="flex-1" />
          <button
            onClick={toggleGlobalChat}
            className="rounded bg-[#313244] px-3 py-1 text-xs text-[#a6adc8] hover:text-[#cdd6f4]"
          >
            Close
          </button>
        </div>
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
            <button
              onClick={handleSend}
              disabled={streaming || !input.trim()}
              className="rounded bg-[#89b4fa] px-4 py-2 text-sm text-[#1e1e2e] disabled:opacity-50"
            >
              Send
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
