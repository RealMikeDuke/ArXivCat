import { useState, useEffect } from "react";
import { useStore } from "../store";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export default function Toolbar() {
  const {
    workspacePath,
    openWorkspace,
    scanPdfs,
    downloadAll,
    toggleSideChat,
    toggleGlobalChat,
    currentPaper,
    sideChatOpen,
    globalChatOpen,
    download,
  } = useStore();

  const [showTokenInput, setShowTokenInput] = useState(false);
  const [tokenInput, setTokenInput] = useState("");
  const [tokenStatus, setTokenStatus] = useState<{ has_token: boolean; masked: string } | null>(null);

  useEffect(() => {
    invoke<{ has_token: boolean; masked: string }>("get_token_status")
      .then(setTokenStatus)
      .catch(() => {});
  }, []);

  // Set up download event listeners
  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];

    const setup = async () => {
      const unlistenProgress = await listen<{
        current: number;
        total: number;
        arxiv_id: string;
        status: string;
      }>("download:progress", (e) => {
        useStore.setState((_) => ({
          download: { inProgress: true, current: e.payload.current, total: e.payload.total },
        }));
        if (e.payload.status === "done") {
          useStore.getState().addLog(`[OK] ${e.payload.arxiv_id} processed`);
        } else if (e.payload.status === "error") {
          useStore.getState().addLog(`[ERROR] ${e.payload.arxiv_id} failed`);
        }
      });
      unlisteners.push(unlistenProgress);

      const unlistenDone = await listen<{ count: number; total: number }>("download:done", (e) => {
        useStore.setState((_) => ({
          download: { inProgress: false, current: 0, total: 0 },
        }));
        useStore.getState().addLog(`[OK] Batch download: ${e.payload.count}/${e.payload.total} papers`);
        useStore.getState().refreshPapers();
      });
      unlisteners.push(unlistenDone);
    };

    setup();

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  const handleOpenFolder = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const folder = await open({ directory: true, multiple: false });
      if (folder) {
        await openWorkspace(folder as string);
      }
    } catch {
      // fallback
    }
  };

  const handleSetToken = async () => {
    try {
      await invoke("set_token", { token: tokenInput });
      setShowTokenInput(false);
      setTokenInput("");
      const status = await invoke<{ has_token: boolean; masked: string }>("get_token_status");
      setTokenStatus(status);
    } catch (e) {
      console.error(e);
    }
  };

  return (
    <div className="flex flex-wrap items-center gap-2">
      <button
        onClick={handleOpenFolder}
        className="rounded bg-[#45475a] px-3 py-1.5 text-sm text-[#cdd6f4] hover:bg-[#585b70]"
      >
        Open Folder
      </button>

      {workspacePath && (
        <>
          <button
            onClick={scanPdfs}
            className="rounded bg-[#45475a] px-3 py-1.5 text-sm text-[#cdd6f4] hover:bg-[#585b70]"
          >
            Scan PDFs
          </button>
          <span className="flex items-center gap-1">
            <button
              onClick={downloadAll}
              disabled={download.inProgress}
              className="rounded bg-[#45475a] px-3 py-1.5 text-sm text-[#cdd6f4] hover:bg-[#585b70] disabled:opacity-50"
            >
              {download.inProgress ? `${download.current}/${download.total}` : "Download All"}
            </button>
          </span>
          {currentPaper && (
            <button
              onClick={toggleSideChat}
              className={`rounded px-3 py-1.5 text-sm ${
                sideChatOpen
                  ? "bg-[#89b4fa] text-[#1e1e2e]"
                  : "bg-[#45475a] text-[#cdd6f4] hover:bg-[#585b70]"
              }`}
            >
              Chat
            </button>
          )}
          <button
            onClick={toggleGlobalChat}
            className={`rounded px-3 py-1.5 text-sm ${
              globalChatOpen
                ? "bg-[#89b4fa] text-[#1e1e2e]"
                : "bg-[#45475a] text-[#cdd6f4] hover:bg-[#585b70]"
            }`}
          >
            Global Chat
          </button>
          <div className="flex-1" />
          <span className="truncate text-xs text-[#a6adc8]">
            {workspacePath}
          </span>
        </>
      )}

      <div className="relative">
        <button
          onClick={() => setShowTokenInput(!showTokenInput)}
          className={`rounded px-2 py-1.5 text-xs ${
            tokenStatus?.has_token
              ? "bg-[#a6e3a1] text-[#1e1e2e]"
              : "bg-[#313244] text-[#a6adc8] hover:text-[#cdd6f4]"
          }`}
        >
          Token
        </button>

        {showTokenInput && (
          <div className="absolute right-0 top-full z-50 mt-1 rounded border border-[#45475a] bg-[#1e1e2e] p-3 shadow-lg">
            {tokenStatus?.has_token && (
              <div className="mb-2 text-xs text-[#a6e3a1]">
                Token: {tokenStatus.masked}
              </div>
            )}
            <input
              type="password"
              value={tokenInput}
              onChange={(e) => setTokenInput(e.target.value)}
              placeholder="DeepSeek API Token"
              className="mb-2 w-64 rounded bg-[#313244] px-2 py-1 text-sm text-[#cdd6f4] outline-none"
            />
            <button
              onClick={handleSetToken}
              className="rounded bg-[#89b4fa] px-3 py-1 text-sm text-[#1e1e2e]"
            >
              Save
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
