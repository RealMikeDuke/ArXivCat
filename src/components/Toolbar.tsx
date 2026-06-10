import { useState } from "react";
import { useStore } from "../store";

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
  } = useStore();

  const [showTokenInput, setShowTokenInput] = useState(false);
  const [tokenInput, setTokenInput] = useState("");

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
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("set_token", { token: tokenInput });
      setShowTokenInput(false);
      setTokenInput("");
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
          <button
            onClick={downloadAll}
            className="rounded bg-[#45475a] px-3 py-1.5 text-sm text-[#cdd6f4] hover:bg-[#585b70]"
          >
            Download All
          </button>
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

      <button
        onClick={() => setShowTokenInput(!showTokenInput)}
        className="rounded bg-[#313244] px-2 py-1.5 text-xs text-[#a6adc8] hover:text-[#cdd6f4]"
      >
        Token
      </button>

      {showTokenInput && (
        <div className="absolute right-2 top-10 z-50 rounded border border-[#45475a] bg-[#1e1e2e] p-3 shadow-lg">
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
  );
}
