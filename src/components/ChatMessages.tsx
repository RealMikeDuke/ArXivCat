import { useRef, useEffect } from "react";
import ReactMarkdown from "react-markdown";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import { ChatMessage } from "../store";
import RippleBtn from "./Ripple";

interface ChatMessagesProps {
  messages: ChatMessage[];
  streaming: boolean;
  status: string;
  localBuffer: string;
  input: string;
  onInputChange: (v: string) => void;
  onSend: () => void;
  onCancel: () => void;
  onKeyDown?: (e: React.KeyboardEvent) => void;
  placeholder?: string;
  emptyLabel?: string;
}

export default function ChatMessages({
  messages, streaming, status, localBuffer,
  input, onInputChange, onSend, onCancel, onKeyDown,
  placeholder = "Ask...", emptyLabel = "Start a conversation",
}: ChatMessagesProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, localBuffer]);

  return (
    <>
      <div className="flex-1 overflow-y-auto p-3">
        {messages.length === 0 && !streaming && (
          <div className="py-8 text-center text-xs text-[#6c7086]">{emptyLabel}</div>
        )}
        {messages.map((m, i) => (
          <div key={i} className={`mb-3 ${m.speaker === "user" ? "text-right" : ""}`}>
            <div
              className={`inline-block rounded-lg px-3 py-2 text-sm ${
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
            <div className="inline-block rounded-lg bg-[#313244] px-3 py-2 text-sm text-[#cdd6f4]">
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
            onChange={(e) => onInputChange(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder={placeholder}
            disabled={streaming}
            className="flex-1 rounded bg-[#313244] px-3 py-1.5 text-sm text-[#cdd6f4] outline-none disabled:opacity-50"
          />
          {streaming ? (
            <RippleBtn onClick={onCancel} className="rounded bg-[#f38ba8] px-4 py-1.5 text-sm text-[#1e1e2e]">
              Stop
            </RippleBtn>
          ) : (
            <RippleBtn onClick={onSend} disabled={!input.trim()}
              className="rounded bg-[#89b4fa] px-4 py-1.5 text-sm text-[#1e1e2e] disabled:opacity-50">
              Send
            </RippleBtn>
          )}
        </div>
      </div>
    </>
  );
}
