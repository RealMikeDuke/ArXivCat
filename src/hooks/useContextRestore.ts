import { useEffect, useRef } from "react";
import { ChatSession } from "./useChatSessions";

export function useContextRestore(
  activeIdx: number,
  sessions: ChatSession[],
  kind: string,
  restore: (session: ChatSession) => void,
  extraDeps: unknown[] = [],
) {
  const restoredKey = useRef<string | null>(null);

  useEffect(() => {
    if (activeIdx < 0 || !sessions[activeIdx]) return;
    const session = sessions[activeIdx];
    const key = `${kind}:${activeIdx}:${session.path}`;
    if (restoredKey.current === key) return;
    restoredKey.current = key;
    restore(session);
  }, [activeIdx, sessions, kind, ...extraDeps]);
}
