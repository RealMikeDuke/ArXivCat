import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ChatMessage } from "../store";

export interface ChatSession {
  path: string;
  title: string;
  kind: string;
  model: string;
  deep_thinking: boolean;
  messages: ChatMessage[];
  context_selection: Record<string, boolean>;
  context_snapshot: string;
  view_name: string;
  updated_at: string;
}

export function useChatSessions(sessionDir: string | null, model: string, deepThinking: boolean) {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [activeIdx, setActiveIdx] = useState(-1);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [status, setStatus] = useState("");
  const [localBuffer, setLocalBuffer] = useState("");
  const localRef = useRef(localBuffer);
  localRef.current = localBuffer;
  const savingRef = useRef(false);

  const loadSessionList = useCallback(async () => {
    if (!sessionDir) {
      setSessions([]);
      setActiveIdx(-1);
      setMessages([]);
      return;
    }
    try {
      const list = await invoke<ChatSession[]>("get_chat_sessions", { sessionDir });
      setSessions(list);
      if (list.length > 0 && activeIdx < 0) {
        setActiveIdx(0);
        setMessages(list[0].messages || []);
      } else if (list.length === 0) {
        setActiveIdx(-1);
        setMessages([]);
      }
    } catch {
      setSessions([]);
      setActiveIdx(-1);
      setMessages([]);
    }
  }, [sessionDir]);

  useEffect(() => {
    setStreaming(false);
    setSessionId(null);
    setStatus("");
    setLocalBuffer("");
    loadSessionList();
  }, [sessionDir]);

  const saveCurrent = useCallback(async (msgs: ChatMessage[]) => {
    if (activeIdx < 0 || !sessionDir || msgs.length === 0) return;
    const s = sessions[activeIdx];
    if (!s) return;
    savingRef.current = true;
    try {
      const savedPath = await invoke<string>("save_chat_session_data", {
        sessionDir,
        sessionData: {
          path: s.path,
          title: s.title,
          kind: s.kind,
          model,
          deep_thinking: deepThinking,
          messages: msgs,
          context_selection: s.context_selection,
          context_snapshot: s.context_snapshot,
          view_name: s.view_name,
        },
      });
      if (savedPath) {
        setSessions((prev) => {
          const next = [...prev];
          if (activeIdx < next.length) {
            next[activeIdx] = { ...next[activeIdx], path: savedPath, messages: msgs };
          }
          return next;
        });
      }
    } catch { /* silent */ }
    savingRef.current = false;
  }, [activeIdx, sessionDir, sessions, model, deepThinking]);

  const newSession = useCallback(async (kind: string) => {
    if (!sessionDir) return;
    const now = new Date();
    const label = kind === "global" ? "Global Chat" : "Chat";
    const title = `${label} ${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")} ${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
    setSessions((prev) => [
      ...prev,
      {
        path: "", title, kind, model,
        deep_thinking: deepThinking,
        messages: [],
        context_selection: {},
        context_snapshot: "",
        view_name: "body",
        updated_at: now.toISOString(),
      },
    ]);
    setActiveIdx(sessions.length);
    setMessages([]);
  }, [sessionDir, model, deepThinking, sessions.length]);

  const switchSession = useCallback(async (idx: number) => {
    if (idx === activeIdx) return;
    await saveCurrent(messages);
    setActiveIdx(idx);
    setMessages(sessions[idx]?.messages || []);
  }, [activeIdx, saveCurrent, messages, sessions]);

  const renameSession = useCallback(async (idx: number, title: string) => {
    if (idx < 0 || idx >= sessions.length || !sessionDir) return;
    const s = sessions[idx];
    if (!s.path) return;
    try {
      await invoke("rename_chat_session_data", { path: s.path, newTitle: title });
      setSessions((prev) => {
        const next = [...prev];
        next[idx] = { ...next[idx], title };
        return next;
      });
    } catch { /* silent */ }
  }, [sessions, sessionDir]);

  const deleteSession = useCallback(async (idx: number) => {
    if (idx < 0 || idx >= sessions.length) return;
    const s = sessions[idx];
    if (s.path) {
      try { await invoke("delete_chat_session_data", { path: s.path }); } catch { /* silent */ }
    }
    await loadSessionList();
    setActiveIdx(-1);
    setMessages([]);
  }, [sessions, loadSessionList]);

  // Event listeners for the current stream
  useEffect(() => {
    if (!sessionId) return;
    const unlisteners: UnlistenFn[] = [];

    const setup = async () => {
      const unlistenToken = await listen<{ session_id: string; token: string }>("chat:token", (e) => {
        if (e.payload.session_id !== sessionId) return;
        setLocalBuffer((prev) => {
          const token = e.payload.token;
          if (prev.startsWith(token)) return token;
          return prev + token;
        });
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
        const finalText = e.payload.text || localRef.current;
        if (finalText) {
          setMessages((prev) => [...prev, { speaker: "assistant", content: finalText }]);
        }
        setLocalBuffer("");
        setSessionId(null);
        setStreaming(false);
      });
      unlisteners.push(unlistenDone);

      const unlistenError = await listen<{ session_id: string; error: string }>("chat:error", (e) => {
        if (e.payload.session_id !== sessionId) return;
        setLocalBuffer("");
        setSessionId(null);
        setStreaming(false);
        setStatus(`error: ${e.payload.error}`);
      });
      unlisteners.push(unlistenError);
    };

    setup();
    return () => unlisteners.forEach((u) => u());
  }, [sessionId]);

  // Auto-save after streaming completes
  useEffect(() => {
    if (streaming || sessionId || savingRef.current || messages.length === 0) return;
    saveCurrent(messages);
  }, [streaming, sessionId]);

  const sendMessage = useCallback(async (content: string, context: string) => {
    if (!content.trim() || streaming) return;
    const userMsg: ChatMessage = { speaker: "user", content };
    const newMsgs = [...messages, userMsg];
    setMessages(newMsgs);
    setStreaming(true);
    setStatus("thinking...");

    try {
      const apiMessages = newMsgs.map((m) => ({
        role: m.speaker === "user" ? "user" : "assistant",
        content: m.content,
      }));
      const { session_id } = await invoke<{ session_id: string }>("start_chat", {
        messages: apiMessages,
        model,
        deepThinking,
        paperContext: context || null,
      });
      setSessionId(session_id);
    } catch (e) {
      setStatus(`error: ${e}`);
      setStreaming(false);
    }
  }, [messages, streaming, model, deepThinking]);

  const cancelChat = useCallback(async () => {
    if (sessionId) {
      try { await invoke("cancel_chat", { sessionId }); } catch {}
    }
    setSessionId(null);
    setStreaming(false);
    setStatus("cancelled");
    setLocalBuffer("");
  }, [sessionId]);

  return {
    sessions, activeIdx, messages, streaming, status, localBuffer,
    newSession, switchSession, renameSession, deleteSession,
    sendMessage, cancelChat,
  };
}
