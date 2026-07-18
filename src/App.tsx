import { useState, useEffect, useCallback } from "react";
import Toolbar from "./components/Toolbar";
import PaperList from "./components/PaperList";
import Preview from "./components/Preview";
import ChatPanel from "./components/ChatPanel";
import GlobalChat from "./components/GlobalChat";
import RippleBtn from "./components/Ripple";
import { useStore } from "./store";
import { useShallow } from "zustand/react/shallow";

export default function App() {
  const { workspacePath, papers, currentPaper, sideChatOpen, globalChatOpen, logMessages, logOpen, toggleLog, initWorkspace } = useStore(
    useShallow((s) => ({
      workspacePath: s.workspacePath,
      papers: s.papers,
      currentPaper: s.currentPaper,
      sideChatOpen: s.sideChatOpen,
      globalChatOpen: s.globalChatOpen,
      logMessages: s.logMessages,
      logOpen: s.logOpen,
      toggleLog: s.toggleLog,
      initWorkspace: s.initWorkspace,
    }))
  );

  const [leftWidth, setLeftWidth] = useState(250);
  const [rightWidth, setRightWidth] = useState(350);
  const [dragging, setDragging] = useState<"left" | "right" | null>(null);

  useEffect(() => {
    initWorkspace();
  }, [initWorkspace]);

  const handleMouseMove = useCallback(
    (e: MouseEvent) => {
      if (dragging === "left") {
        const w = Math.max(180, Math.min(500, e.clientX));
        setLeftWidth(w);
      } else if (dragging === "right") {
        const w = Math.max(250, Math.min(600, window.innerWidth - e.clientX));
        setRightWidth(w);
      }
    },
    [dragging],
  );

  const handleMouseUp = useCallback(() => setDragging(null), []);

  useEffect(() => {
    if (dragging) {
      window.addEventListener("mousemove", handleMouseMove);
      window.addEventListener("mouseup", handleMouseUp);
      return () => {
        window.removeEventListener("mousemove", handleMouseMove);
        window.removeEventListener("mouseup", handleMouseUp);
      };
    }
  }, [dragging, handleMouseMove, handleMouseUp]);

  if (!workspacePath) {
    return (
      <div className="flex h-screen items-center justify-center bg-[#1e1e2e] text-[#cdd6f4]">
        <div className="text-center">
          <h1 className="mb-4 text-3xl font-bold">ArxivCat</h1>
          <p className="mb-6 text-[#a6adc8]">arXiv paper extraction &amp; chat</p>
          <Toolbar />
        </div>
      </div>
    );
  }

  return (
    <div className="relative flex h-screen flex-col bg-[#1e1e2e] text-[#cdd6f4]">
      <div className="border-b border-[#313244] bg-[#181825] px-4 py-2">
        <Toolbar />
      </div>
      <div className="flex flex-1 overflow-hidden">
        <div style={{ width: leftWidth }} className="flex-shrink-0 overflow-y-auto border-r border-[#313244]">
          <PaperList />
        </div>

        <div
          className="w-1 cursor-col-resize bg-[#313244] hover:bg-[#89b4fa] active:bg-[#89b4fa]"
          onMouseDown={() => setDragging("left")}
        />

        <div className="flex-1 overflow-y-auto p-4">
          {currentPaper ? (
            <Preview />
          ) : (
            <div className="flex h-full items-center justify-center text-[#a6adc8]">
              <div className="text-center">
                <p className="text-lg">Select a paper or enter an arXiv ID</p>
                <p className="mt-2 text-sm">
                  {papers.length > 0
                    ? `${papers.length} papers in workspace`
                    : "No papers yet — scan PDFs or download"}
                </p>
              </div>
            </div>
          )}
        </div>

        {sideChatOpen && currentPaper && (
          <>
            <div
              className="w-1 cursor-col-resize bg-[#313244] hover:bg-[#89b4fa] active:bg-[#89b4fa]"
              onMouseDown={() => setDragging("right")}
            />
            <div style={{ width: rightWidth }} className="flex-shrink-0 overflow-y-auto border-l border-[#313244]">
              <ChatPanel />
            </div>
          </>
        )}
      </div>
      {globalChatOpen && <GlobalChat />}

      {logOpen && (
        <div className="border-t border-[#313244] bg-[#11111b]">
          <div className="flex items-center justify-between px-3 py-1">
            <span className="text-xs font-semibold text-[#a6adc8]">Log</span>
            <RippleBtn onClick={toggleLog} className="text-xs text-[#6c7086] hover:text-[#cdd6f4]">Close</RippleBtn>
          </div>
          <div className="max-h-20 overflow-y-auto px-3 pb-1">
            {logMessages.map((msg, i) => (
              <div key={i} className="font-mono text-xs leading-5 text-[#a6adc8]">{msg}</div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
