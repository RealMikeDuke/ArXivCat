import { useState, useEffect, useRef } from "react";
import { useStore, BTN } from "../store";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import RippleBtn from "./Ripple";
import Dropdown from "./Dropdown";
import { useShallow } from "zustand/react/shallow";

export default function Toolbar() {
  const { workspacePath, currentPaper, sideChatOpen, globalChatOpen, leftPanelOpen, toggleLog, toggleLeftPanel, download, openWorkspace, scanPdfs, downloadAll, downloadPaper, toggleSideChat, toggleGlobalChat } = useStore(
    useShallow((s) => ({
      workspacePath: s.workspacePath,
      currentPaper: s.currentPaper,
      sideChatOpen: s.sideChatOpen,
      globalChatOpen: s.globalChatOpen,
      leftPanelOpen: s.leftPanelOpen,
      toggleLog: s.toggleLog,
      toggleLeftPanel: s.toggleLeftPanel,
      download: s.download,
      openWorkspace: s.openWorkspace,
      scanPdfs: s.scanPdfs,
      downloadAll: s.downloadAll,
      downloadPaper: s.downloadPaper,
      toggleSideChat: s.toggleSideChat,
      toggleGlobalChat: s.toggleGlobalChat,
    }))
  );

  const [arxivInput, setArxivInput] = useState("");

  const tokenBtnRef = useRef<HTMLDivElement>(null);
  const [showTokenInput, setShowTokenInput] = useState(false);
  const [tokenInput, setTokenInput] = useState("");
  const [tokenStatus, setTokenStatus] = useState<{ has_token: boolean; masked: string } | null>(null);

  useEffect(() => {
    invoke<{ has_token: boolean; masked: string }>("get_token_status")
      .then(setTokenStatus)
      .catch((e: any) => { useStore.getState().addLog(`[ERROR] Failed to get token status: ${e}`); });
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
        useStore.setState({
          download: { inProgress: true, current: e.payload.current, total: e.payload.total },
        });
        if (e.payload.status === "done") {
          useStore.getState().addLog(`[OK] ${e.payload.arxiv_id} processed`);
        } else if (e.payload.status === "error") {
          useStore.getState().addLog(`[ERROR] ${e.payload.arxiv_id} failed`);
        }
      });
      unlisteners.push(unlistenProgress);

      const unlistenDone = await listen<{ count: number; total: number }>("download:done", (e) => {
        useStore.setState({
          download: { inProgress: false, current: 0, total: 0 },
        });
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
    } catch (e) {
      useStore.getState().addLog(`[ERROR] Failed to open folder dialog: ${e}`);
    }
  };

  const handleSetToken = async () => {
    try {
      await invoke("set_token", { token: tokenInput });
      setShowTokenInput(false);
      setTokenInput("");
      const status = await invoke<{ has_token: boolean; masked: string }>("get_token_status");
      setTokenStatus(status);
      useStore.getState().addLog("[OK] API token saved");
    } catch (e) {
      useStore.getState().addLog(`[ERROR] Failed to set token: ${e}`);
    }
  };

  return (
    <div className="flex flex-wrap items-center gap-2">
      {workspacePath && (
        <>
          <RippleBtn
            onClick={scanPdfs}
            className={`rounded px-3 py-1.5 text-sm ${BTN.surface1}`}
          >
            Scan PDFs
          </RippleBtn>
          <span className="flex items-center gap-1">
            <RippleBtn
              onClick={downloadAll}
              disabled={download.inProgress}
              className={`rounded px-3 py-1.5 text-sm disabled:opacity-50 ${BTN.surface1}`}
            >
              {download.inProgress ? `${download.current}/${download.total}` : "Download All"}
            </RippleBtn>
          </span>
          <div className="flex items-center gap-1">
            <input
              value={arxivInput}
              onChange={(e) => setArxivInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && arxivInput.trim()) {
                  downloadPaper(arxivInput.trim());
                  setArxivInput("");
                }
              }}
              placeholder="arXiv ID or URL..."
              className="w-48 rounded bg-[#313244] px-2 py-1.5 text-sm text-[#cdd6f4] outline-none placeholder:text-[#6c7086]"
            />
            <RippleBtn
              onClick={() => {
                if (arxivInput.trim()) {
                  downloadPaper(arxivInput.trim());
                  setArxivInput("");
                }
              }}
              className={`rounded px-3 py-1.5 text-sm ${BTN.blue}`}
            >
              Download
            </RippleBtn>
          </div>
          <RippleBtn
            onClick={toggleLeftPanel}
            className={`rounded px-3 py-1.5 text-sm transition-colors duration-150 ${
              leftPanelOpen
                ? BTN.blue
                : BTN.surface1
            }`}
          >
            Papers
          </RippleBtn>
          {currentPaper && (
            <RippleBtn
              onClick={toggleSideChat}
              className={`rounded px-3 py-1.5 text-sm transition-colors duration-150 ${
                sideChatOpen
                  ? BTN.blue
                  : BTN.surface1
              }`}
            >
              Side Chat
            </RippleBtn>
          )}
          <RippleBtn
            onClick={toggleGlobalChat}
            className={`rounded px-3 py-1.5 text-sm transition-colors duration-150 ${
              globalChatOpen
                ? BTN.blue
                : BTN.surface1
            }`}
          >
            Global Chat
          </RippleBtn>
          <div className="flex-1" />
          <RippleBtn
            onClick={handleOpenFolder}
            className={`max-w-[200px] truncate rounded px-2 py-1.5 text-xs ${BTN.surface1}`}
            title="Click to change workspace folder"
          >
            {workspacePath}
          </RippleBtn>
        </>
      )}

      <RippleBtn
        onClick={toggleLog}
        className={`rounded px-2 py-1.5 text-xs ${BTN.surface1}`}
      >
        Log
      </RippleBtn>

      <div className="relative">
        <span ref={tokenBtnRef}>
          <RippleBtn
            onClick={() => setShowTokenInput(!showTokenInput)}
          className={`rounded px-2 py-1.5 text-xs ${
            tokenStatus?.has_token
              ? BTN.green
              : BTN.surface1
          }`}
        >
          Token
        </RippleBtn>
        </span>
        <Dropdown open={showTokenInput} onClose={() => setShowTokenInput(false)}
          anchorRef={tokenBtnRef} width={300}>
          <div className="p-3">
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
            <RippleBtn
              onClick={handleSetToken}
              className={`rounded px-3 py-1 text-sm ${BTN.blue}`}
            >
              Save
            </RippleBtn>
          </div>
        </Dropdown>
      </div>
    </div>
  );
}
