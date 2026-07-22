import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ChatMessage, useStore } from "../store";

export interface ChatSession {
  path: string;
  title: string;
  kind: string;
  model: string;
  reasoning_effort: string;
  locked_fields: Record<string, string[]>;
  messages: ChatMessage[];
  context_selection: Record<string, boolean>;
  context_snapshot: string;
  view_name: string;
  updated_at: string;
}

export function useChatSessions(sessionDir: string | null, model: string, reasoningEffort: string) {
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

  const lockedFields: Record<string, string[]> = activeIdx >= 0 && activeIdx < sessions.length
    ? sessions[activeIdx].locked_fields || {} : {};

  const lockFields = useCallback((fields: Record<string, string[]>) => {
    if (Object.keys(fields).length === 0) return;
    setSessions((prev) => {
      const idx = activeIdx >= 0 ? activeIdx : prev.length - 1;
      if (idx < 0 || idx >= prev.length) return prev;
      const existing = prev[idx].locked_fields || {};
      const merged: Record<string, string[]> = {};
      for (const key of new Set([...Object.keys(existing), ...Object.keys(fields)]) as Set<string>) {
        merged[key] = [...new Set([...(existing[key] || []), ...(fields[key] || [])])];
      }
      const next = [...prev];
      next[idx] = { ...next[idx], locked_fields: merged };
      return next;
    });
  }, [activeIdx]);

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
    } catch (e) {
      useStore.getState().addLog(`[ERROR] Failed to load sessions: ${e}`);
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
          reasoning_effort: reasoningEffort,
          messages: msgs,
          locked_fields: s.locked_fields || {},
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
    } catch (e) {
      useStore.getState().addLog(`[ERROR] Failed to save chat session: ${e}`);
    }
    savingRef.current = false;
  }, [activeIdx, sessionDir, sessions, model, reasoningEffort]);

  const newSession = useCallback(async (kind: string) => {
    if (!sessionDir) return;
    const now = new Date();
    const label = kind === "global" ? "Global Chat" : "Chat";
    const title = `${label} ${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")} ${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
    setSessions((prev) => [
      ...prev,
      {
        path: "", title, kind, model,
        reasoning_effort: reasoningEffort,
        locked_fields: {},
        messages: [],
        context_selection: {},
        context_snapshot: "",
        view_name: "body",
        updated_at: now.toISOString(),
      },
    ]);
    setActiveIdx(sessions.length);
    setMessages([]);
  }, [sessionDir, model, reasoningEffort, sessions.length]);

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
    } catch (e) {
      useStore.getState().addLog(`[ERROR] Failed to rename session: ${e}`);
    }
  }, [sessions, sessionDir]);

  const deleteSession = useCallback(async (idx: number) => {
    if (idx < 0 || idx >= sessions.length) return;
    const s = sessions[idx];
    if (s.path) {
      try { await invoke("delete_chat_session_data", { path: s.path }); } catch (e) { useStore.getState().addLog(`[ERROR] Failed to delete session: ${e}`); }
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

  const generateTitle = useCallback(async (msgs: ChatMessage[], sessionIdx: number) => {
    const { addLog, showToast } = useStore.getState();
    addLog(`[title] called idx=${sessionIdx} msgs=${msgs.length} sessions=${sessions.length}`);
    if (sessionIdx < 0 || sessionIdx >= sessions.length) { addLog(`[title] bail: idx ${sessionIdx} out of ${sessions.length}`); return; }
    const apiMessages = msgs.map((m) => ({
      role: m.speaker === "user" ? "user" : "assistant",
      content: m.content,
    }));
    try {
      const title = await invoke<string>("generate_chat_title", { messages: apiMessages });
      addLog(`[title] API → "${title}"`);
      showToast(`Title: ${title}`);
      let path = "";
      setSessions((prev) => {
        const next = [...prev];
        if (sessionIdx < next.length) {
          next[sessionIdx] = { ...next[sessionIdx], title };
          path = next[sessionIdx].path;
        }
        return next;
      });
      if (path) {
        invoke("rename_chat_session_data", { path, newTitle: title }).catch((e: any) => { useStore.getState().addLog(`[ERROR] Failed to rename session on disk: ${e}`); });
      }
    } catch (e) {
      addLog(`[title] error: ${e}`);
    }
  }, [sessions]);

  // Auto-save after streaming completes + chain title generation
  const titledLengths = useRef(new Set<number>());

  useEffect(() => {
    if (streaming || sessionId || savingRef.current || messages.length === 0) return;
    const n = messages.length;
    const s = activeIdx >= 0 ? sessions[activeIdx] : null;
    const shouldTitle = s && (s.title.startsWith("Chat ") || s.title.startsWith("Global Chat ")) &&
      (n === 2 || (n > 2 && (n - 2) % 10 === 0)) && !titledLengths.current.has(n);

    const doSave = async () => {
      await saveCurrent(messages);
      if (shouldTitle) {
        titledLengths.current.add(n);
        generateTitle(messages, activeIdx);
      }
    };
    doSave();
  }, [streaming, sessionId, messages.length, activeIdx, sessions, saveCurrent, generateTitle]);

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
        reasoningEffort,
        paperContext: context || null,
      });
      setSessionId(session_id);
    } catch (e) {
      useStore.getState().addLog(`[ERROR] Failed to start chat: ${e}`);
      setStatus(`error: ${e}`);
      setStreaming(false);
    }
  }, [messages, streaming, model, reasoningEffort]);

  const cancelChat = useCallback(async () => {
    if (sessionId) {
      try { await invoke("cancel_chat", { sessionId }); } catch (e) { useStore.getState().addLog(`[ERROR] Failed to cancel chat: ${e}`); }
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
    lockedFields, lockFields, generateTitle,
  };
}
