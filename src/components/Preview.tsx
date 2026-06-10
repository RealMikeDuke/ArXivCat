import { useState, useCallback } from "react";
import { useStore, ViewMode } from "../store";

const TAB_OPTIONS: { key: ViewMode; label: string }[] = [
  { key: "body", label: "Body" },
  { key: "appendix", label: "Appendix" },
  { key: "note", label: "Note" },
  { key: "description", label: "Description" },
];

export default function Preview() {
  const {
    currentPaper,
    previewContent,
    currentView,
    switchView,
    saveNote,
    stripComments,
  } = useStore();

  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState("");

  const content = previewContent[currentView] || "";
  const isNote = currentView === "note";

  const handleEdit = useCallback(() => {
    setEditValue(previewContent["note"] || "");
    setEditing(true);
  }, [previewContent]);

  const handleSave = useCallback(async () => {
    await saveNote(editValue);
    setEditing(false);
  }, [saveNote, editValue]);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(content);
  }, [content]);

  const handleOpenPdf = useCallback(async () => {
    if (!currentPaper) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("open_paper_pdf", {
        workspacePath: useStore.getState().workspacePath,
        folderName: currentPaper.folder_name,
        arxivId: currentPaper.arxiv_id,
      });
    } catch {
      window.open(`https://arxiv.org/pdf/${currentPaper.arxiv_id}`, "_blank");
    }
  }, [currentPaper]);

  const handleOpenFolder = useCallback(async () => {
    if (!currentPaper) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("open_paper_folder", {
        workspacePath: useStore.getState().workspacePath,
        folderName: currentPaper.folder_name,
      });
    } catch {}
  }, [currentPaper]);

  const wordCount = content.split(/\s+/).filter(Boolean).length;

  return (
    <div className="flex h-full flex-col">
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <div className="flex gap-1">
          {TAB_OPTIONS.map((tab) => (
            <button
              key={tab.key}
              onClick={() => {
                switchView(tab.key);
                setEditing(false);
              }}
              className={`rounded px-3 py-1 text-xs ${
                currentView === tab.key
                  ? "bg-[#89b4fa] text-[#1e1e2e]"
                  : "bg-[#313244] text-[#a6adc8] hover:text-[#cdd6f4]"
              }`}
            >
              {tab.label}
            </button>
          ))}
        </div>
        <div className="flex-1" />
        <div className="flex items-center gap-1">
          <span className="text-xs text-[#6c7086]">{content.length} chars · {wordCount} words</span>
          <button onClick={handleCopy} className="rounded bg-[#313244] px-2 py-1 text-xs text-[#a6adc8] hover:text-[#cdd6f4]">
            Copy
          </button>
          <button onClick={stripComments} className="rounded bg-[#313244] px-2 py-1 text-xs text-[#a6adc8] hover:text-[#cdd6f4]">
            Strip
          </button>
          <button onClick={handleOpenPdf} className="rounded bg-[#313244] px-2 py-1 text-xs text-[#a6adc8] hover:text-[#cdd6f4]">
            PDF
          </button>
          <button onClick={handleOpenFolder} className="rounded bg-[#313244] px-2 py-1 text-xs text-[#a6adc8] hover:text-[#cdd6f4]">
            Folder
          </button>
          {isNote && !editing && (
            <button onClick={handleEdit} className="rounded bg-[#313244] px-2 py-1 text-xs text-[#a6adc8] hover:text-[#cdd6f4]">
              Edit
            </button>
          )}
        </div>
      </div>

      <div className="flex-1 overflow-auto rounded border border-[#313244] bg-[#11111b] p-3">
        {isNote && editing ? (
          <div className="flex h-full flex-col">
            <textarea
              value={editValue}
              onChange={(e) => setEditValue(e.target.value)}
              className="flex-1 resize-none bg-transparent font-mono text-sm text-[#cdd6f4] outline-none"
              spellCheck={false}
            />
            <div className="mt-2 flex gap-2">
              <button
                onClick={handleSave}
                className="rounded bg-[#a6e3a1] px-3 py-1 text-xs text-[#1e1e2e]"
              >
                Save
              </button>
              <button
                onClick={() => setEditing(false)}
                className="rounded bg-[#45475a] px-3 py-1 text-xs text-[#cdd6f4]"
              >
                Cancel
              </button>
            </div>
          </div>
        ) : (
          <pre className="whitespace-pre-wrap font-mono text-sm text-[#cdd6f4]">
            {content || "(empty)"}
          </pre>
        )}
      </div>
    </div>
  );
}
