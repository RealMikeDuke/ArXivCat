import { useState, useCallback, useEffect, useRef, useMemo, memo } from "react";
import { useStore, ViewMode, BTN } from "../store";
import { invoke } from "@tauri-apps/api/core";
import RippleBtn from "./Ripple";

const PreView = memo(({ html }: { html: string }) => (
  <pre className="whitespace-pre-wrap break-words font-mono text-sm text-[#cdd6f4]"
    dangerouslySetInnerHTML={{ __html: html }} />
));

const TAB_OPTIONS: { key: ViewMode; label: string }[] = [
  { key: "body", label: "Body" },
  { key: "appendix", label: "Appendix" },
  { key: "note", label: "Note" },
  { key: "description", label: "Description" },
  { key: "pdf", label: "PDF" },
];

function renderMarkers(text: string) {
  if (!text) return "";
  const html = text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\n/g, '<span class="text-[#6c7086]/50 select-none">\u00B6</span>\n');
  return html;
}

const Preview = memo(function Preview() {
  const workspacePath = useStore((s) => s.workspacePath);
  const currentPaper = useStore((s) => s.currentPaper);
  const previewContent = useStore((s) => s.previewContent);
  const currentView = useStore((s) => s.currentView);
  const switchView = useStore((s) => s.switchView);
  const saveNote = useStore((s) => s.saveNote);
  const saveDescription = useStore((s) => s.saveDescription);
  const getDraftKey = useStore((s) => s.getDraftKey);
  const saveDraft = useStore((s) => s.saveDraft);
  const clearDraft = useStore((s) => s.clearDraft);
  const draftValue = useStore((s) => {
    const key = s.getDraftKey();
    return key ? s.drafts[key] : undefined;
  });

  const [editValue, setEditValue] = useState("");
  const [editing, setEditing] = useState(false);
  const [activeTab, setActiveTab] = useState<ViewMode>(currentView);
  const draftTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const contentRef = useRef<HTMLDivElement>(null);
  const paperKeyRef = useRef("");
  paperKeyRef.current = currentPaper?.arxiv_id ?? "";
  const allScrolls = useRef<Record<string, Record<string, number>>>({});
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const allTextareaScrolls = useRef<Record<string, Record<string, number>>>({});

  const bodyContent = previewContent["body"] || "";
  const appendixContent = previewContent["appendix"] || "";
  const noteContent = previewContent["note"] || "";
  const descContent = previewContent["description"] || "";
  const content = previewContent[currentView] || "";
  const isEditable = currentView === "note" || currentView === "description";

  useEffect(() => {
    setActiveTab(currentView);
    if (!isEditable) return;
    if (draftValue !== undefined) {
      setEditValue(draftValue);
      setEditing(true);
    } else {
      setEditValue(content);
      setEditing(false);
    }
  }, [currentView]);

  useEffect(() => {
    const pk = paperKeyRef.current;
    if (!editing || !pk) return;
    requestAnimationFrame(() => {
      if (textareaRef.current) {
        textareaRef.current.scrollTop = allTextareaScrolls.current[pk]?.[currentView] ?? 0;
      }
    });
  }, [editing, currentView]);

  const renderedBody = useMemo(() => renderMarkers(bodyContent), [bodyContent]);
  const renderedAppendix = useMemo(() => renderMarkers(appendixContent), [appendixContent]);
  const renderedNote = useMemo(() => renderMarkers(noteContent), [noteContent]);
  const renderedDesc = useMemo(() => renderMarkers(descContent), [descContent]);
  const [pdfUrl, setPdfUrl] = useState<string | null>(null);
  const [pdfError, setPdfError] = useState(false);
  const pdfUrlRef = useRef<string | null>(null);

  useEffect(() => {
    if (currentView !== "pdf" || pdfUrl || pdfError || !workspacePath || !currentPaper) return;
    invoke<string>("read_pdf_base64", {
      workspacePath,
      folderName: currentPaper.folder_name,
      arxivId: currentPaper.arxiv_id,
    }).then((b64) => {
      const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
      const blob = new Blob([bytes], { type: "application/pdf" });
      const url = URL.createObjectURL(blob);
      pdfUrlRef.current = url;
      setPdfUrl(url);
    }).catch((e: any) => { useStore.getState().addLog(`[ERROR] Failed to read PDF: ${e}`); setPdfError(true); });
  }, [currentView, workspacePath, currentPaper, pdfUrl, pdfError]);

  useEffect(() => {
    return () => {
      if (pdfUrlRef.current) {
        URL.revokeObjectURL(pdfUrlRef.current);
        pdfUrlRef.current = null;
      }
      setPdfUrl(null);
      setPdfError(false);
    };
  }, [workspacePath, currentPaper]);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(content);
    useStore.getState().showToast("Copied!");
  }, [content]);

  const handleEdit = useCallback(() => {
    setEditValue(draftValue ?? content);
    setEditing(true);
  }, [draftValue, content]);

  const commitSave = useCallback(() => {
    if (currentView === "note") saveNote(editValue);
    else if (currentView === "description") saveDescription(editValue);
    const key = getDraftKey();
    if (key) clearDraft(key);
    setEditing(false);
  }, [currentView, editValue, saveNote, saveDescription, getDraftKey, clearDraft]);

  const handleCancel = useCallback(() => {
    const key = getDraftKey();
    if (key) clearDraft(key);
    setEditing(false);
  }, [getDraftKey, clearDraft]);

  useEffect(() => {
    if (!editing) return;
    const key = getDraftKey();
    if (!key) return;
    clearTimeout(draftTimer.current);
    draftTimer.current = setTimeout(() => saveDraft(key, editValue), 500);
    return () => clearTimeout(draftTimer.current);
  }, [editValue, editing]);

  const tabClick = useCallback((tab: ViewMode) => {
    if (tab === currentView) return;
    const key = paperKeyRef.current;
    if (contentRef.current && key) {
      if (!allScrolls.current[key]) {
        allScrolls.current[key] = { body: 0, appendix: 0, note: 0, description: 0, pdf: 0 };
      }
      allScrolls.current[key][currentView] = contentRef.current.scrollTop;
    }
    if (editing && textareaRef.current && key) {
      if (!allTextareaScrolls.current[key]) {
        allTextareaScrolls.current[key] = { note: 0, description: 0 };
      }
      allTextareaScrolls.current[key][currentView] = textareaRef.current.scrollTop;
    }
    setActiveTab(tab);
    setTimeout(() => {
      if (editing) {
        const key = getDraftKey();
        if (key) saveDraft(key, editValue);
      }
      setEditing(false);
      switchView(tab);
    }, 0);
  }, [editing, getDraftKey, saveDraft, editValue, switchView, currentView]);

  useEffect(() => {
    const pk = paperKeyRef.current;
    if (!pk) return;
    requestAnimationFrame(() => {
      if (contentRef.current) {
        contentRef.current.scrollTop = allScrolls.current[pk]?.[currentView] ?? 0;
      }
    });
  }, [currentView, previewContent]);

  const wordCount = useMemo(() => content.split(/\s+/).filter(Boolean).length, [content]);

  return (
    <div className="relative flex h-full flex-col">
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <div className="flex gap-1">
          {TAB_OPTIONS.map((tab) => (
            <RippleBtn
              key={tab.key}
              onClick={() => tabClick(tab.key)}
              className={`rounded px-3 py-1 text-xs transition-colors duration-150 ${
                activeTab === tab.key
                  ? BTN.blue
                  : BTN.surface0
              }`}
            >
              {tab.label}
            </RippleBtn>
          ))}
        </div>
        <div className="flex-1" />
        <div className="flex items-center gap-1">
          <span className="text-xs text-[#6c7086]">{content.length} chars · {wordCount} words</span>
          <RippleBtn onClick={handleCopy} className={`rounded px-2 py-1 text-xs ${BTN.surface0}`}>
            Copy
          </RippleBtn>
          {isEditable && !editing && (
            <RippleBtn onClick={handleEdit} className={`rounded px-2 py-1 text-xs ${BTN.surface0}`}>
              Edit
            </RippleBtn>
          )}
        </div>
      </div>

      <div ref={contentRef} className="flex-1 overflow-auto rounded border border-[#313244] bg-[#11111b] p-3">
        {currentView === "body" && <PreView html={renderedBody} />}
        {currentView === "appendix" && <PreView html={renderedAppendix} />}
        {currentView === "note" && !editing && <PreView html={renderedNote} />}
        {currentView === "note" && editing && (
          <div className="flex h-full flex-col">
            <textarea ref={textareaRef} value={editValue} onChange={(e) => setEditValue(e.target.value)}
              className="flex-1 resize-none bg-transparent font-mono text-sm text-[#cdd6f4] outline-none" spellCheck={false} />
            <div className="mt-2 flex gap-2">
              <RippleBtn onClick={commitSave} className={`rounded px-3 py-1 text-xs ${BTN.green}`}>Save</RippleBtn>
              <RippleBtn onClick={handleCancel} className={`rounded px-3 py-1 text-xs ${BTN.surface1}`}>Cancel</RippleBtn>
            </div>
          </div>
        )}
        {currentView === "description" && !editing && <PreView html={renderedDesc} />}
        {currentView === "description" && editing && (
          <div className="flex h-full flex-col">
            <textarea ref={textareaRef} value={editValue} onChange={(e) => setEditValue(e.target.value)}
              className="flex-1 resize-none bg-transparent font-mono text-sm text-[#cdd6f4] outline-none" spellCheck={false} />
            <div className="mt-2 flex gap-2">
              <RippleBtn onClick={commitSave} className={`rounded px-3 py-1 text-xs ${BTN.green}`}>Save</RippleBtn>
              <RippleBtn onClick={handleCancel} className={`rounded px-3 py-1 text-xs ${BTN.surface1}`}>Cancel</RippleBtn>
            </div>
          </div>
        )}
        {currentView === "pdf" && pdfError && (
          <div className="flex h-full flex-col items-center justify-center gap-3">
            <span className="text-xs text-[#6c7086]">Built-in PDF viewer unavailable</span>
            <RippleBtn onClick={() => invoke("open_paper_pdf", {
              workspacePath, folderName: currentPaper?.folder_name, arxivId: currentPaper?.arxiv_id,
            })} className={`rounded px-3 py-1.5 text-xs ${BTN.surface1}`}>
              Open Externally
            </RippleBtn>
          </div>
        )}
        {currentView === "pdf" && !pdfUrl && !pdfError && (
          <div className="flex h-full items-center justify-center text-xs text-[#6c7086]">Loading PDF...</div>
        )}
        {pdfUrl && (
          <div style={{ display: currentView === "pdf" ? "" : "none" }} className="h-full w-full">
            <embed src={pdfUrl} type="application/pdf" className="h-full w-full" onError={() => setPdfError(true)} />
          </div>
        )}
      </div>
    </div>
  );
});

export default Preview;
