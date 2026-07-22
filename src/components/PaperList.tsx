import { useState, useCallback } from "react";
import { useStore, BTN } from "../store";
import { invoke } from "@tauri-apps/api/core";
import { useShallow } from "zustand/react/shallow";
import RippleBtn from "./Ripple";
import StateBtn from "./StateBtn";
import type { BtnStatus } from "./StateBtn";
import Tooltip from "./Tooltip";
import Dialog from "./Dialog";

export default function PaperList() {
  const { papers, currentPaper, selectPaper, workspacePath } = useStore(useShallow((s) => ({
    papers: s.papers, currentPaper: s.currentPaper, selectPaper: s.selectPaper,
    workspacePath: s.workspacePath,
  })));

  const [regenOpen, setRegenOpen] = useState(false);
  const [regenStatus, setRegenStatus] = useState<Record<string, BtnStatus>>({});

  const anyRunning = Object.values(regenStatus).some((s) => s === "running");
  const selectedCount = Object.keys(regenStatus).length;
  const doneCount = Object.values(regenStatus).filter((s) => s === "done" || s === "error").length;

  const handleRegen = useCallback(async () => {
    if (!workspacePath || anyRunning) return;
    const folders = Object.entries(regenStatus)
      .filter(([, s]) => s === "idle")
      .map(([f]) => f);
    const addLog = useStore.getState().addLog;

    addLog(`[INFO] Regenerating descriptions for ${folders.length} papers...`);
    setRegenStatus((prev) => {
      const next = { ...prev };
      for (const f of folders) next[f] = "running";
      return next;
    });

    let ok = 0, fail = 0;
    const tasks = folders.map(async (folder) => {
      const p = papers.find((x) => x.folder_name === folder);
      if (!p) return;
      try {
        await invoke("build_description", {
          paperDir: `${workspacePath}/${folder}`,
          arxivId: p.arxiv_id,
          title: p.title,
          context: null as string | null,
        });
        ok++;
        addLog(`[OK] Description regenerated: ${folder}`);
        setRegenStatus((prev) => ({ ...prev, [folder]: "done" }));
      } catch (e) {
        fail++;
        addLog(`[ERROR] Regenerate description failed for ${folder}: ${e}`);
        setRegenStatus((prev) => ({ ...prev, [folder]: "error" }));
      }
    });

    await Promise.allSettled(tasks);
    addLog(`[INFO] Regeneration complete: ${ok} ok, ${fail} failed`);
    useStore.getState().showToast(`Regenerated ${ok} papers${fail ? `, ${fail} failed` : ""}`, fail ? "warning" : "success");
    setRegenOpen(false);
    setTimeout(() => setRegenStatus({}), 200);
    if (currentPaper) {
      const content = await invoke<Record<string, string>>("load_paper", {
        workspacePath, folderName: currentPaper.folder_name,
      });
      useStore.setState({ previewContent: content });
    }
  }, [workspacePath, anyRunning, regenStatus, papers, currentPaper]);

  const togglePaper = useCallback((folder: string) => {
    setRegenStatus((prev) => {
      if (prev[folder]) {
        const next = { ...prev };
        delete next[folder];
        return next;
      }
      return { ...prev, [folder]: "idle" };
    });
  }, []);

  if (papers.length === 0) {
    return (
      <div className="p-4 text-sm text-[#a6adc8]">
        No papers in workspace
      </div>
    );
  }

  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-2 border-b border-[#313244] px-3 py-2 text-xs font-semibold text-[#a6adc8]">
        <span className="flex-1">Papers ({papers.length})</span>
        <RippleBtn onClick={() => { setRegenStatus({}); setRegenOpen(true); }}
          className={`rounded px-2 py-0.5 text-xs ${BTN.surface0}`}>
          Regen Desc
        </RippleBtn>
      </div>

      <Dialog open={regenOpen} onClose={() => { if (!anyRunning) { setRegenOpen(false); setRegenStatus({}); } }}
        title="Regenerate Descriptions" defaultWidth={500} defaultHeight={450}
        headerExtra={
          <>
            <button onClick={() => {
              if (Object.keys(regenStatus).length === papers.length) setRegenStatus({});
              else setRegenStatus(Object.fromEntries(papers.map((p) => [p.folder_name, "idle"])));
            }}
              className={`rounded px-2 py-1 text-xs ${Object.keys(regenStatus).length === papers.length ? BTN.blue : BTN.surface0}`}>
              {Object.keys(regenStatus).length === papers.length ? "Deselect All" : "Select All"}
            </button>
            <RippleBtn onClick={handleRegen} disabled={anyRunning || selectedCount === 0}
              className={`rounded px-3 py-1 text-xs ${BTN.blue}`}>
              {anyRunning ? `${doneCount}/${selectedCount}` : `Regenerate (${selectedCount})`}
            </RippleBtn>
          </>
        }>
        <div className="flex-1 overflow-y-auto p-2">
          {papers.map((p) => {
            const st = regenStatus[p.folder_name];
            if (st === undefined) {
              return (
                <button key={p.folder_name}
                  onClick={() => { if (!anyRunning) togglePaper(p.folder_name); }}
                  className={`mb-1 flex w-full items-center gap-2 rounded px-3 py-2 text-left text-xs ${BTN.surface0}`}>
                  <span className="w-28 truncate font-mono">{p.arxiv_id}</span>
                  <span className="flex-1 truncate text-[#cdd6f4]">{p.title}</span>
                  <span>{p.is_complete ? "✓" : "○"}</span>
                </button>
              );
            }
            return (
              <StateBtn key={p.folder_name} status={st}
                disabled={st === "running"}
                onClick={() => { if (!anyRunning) togglePaper(p.folder_name); }}
                className="mb-1 flex w-full items-center gap-2">
                <span className="w-28 truncate font-mono">{p.arxiv_id}</span>
                <span className="flex-1 truncate">{p.title}</span>
                <span>{st === "running" ? "..." : st === "done" ? "✓" : st === "error" ? "✗" : "·"}</span>
              </StateBtn>
            );
          })}
        </div>
      </Dialog>

      {papers.map((p) => {
        const isSelected = currentPaper?.folder_name === p.folder_name;
        return (
          <RippleBtn
            key={p.folder_name}
            onClick={() => selectPaper(p)}
            className={`px-3 py-2 text-left text-sm transition-colors ${
              isSelected
                ? BTN.surface1
                : BTN.ghost
            }`}
          >
            <div className="flex items-center gap-2">
              <span
                className={`text-xs ${
                  p.is_complete
                    ? "text-[#a6e3a1]"
                    : p.has_body
                      ? "text-[#f9e2af]"
                      : "text-[#6c7086]"
                }`}
              >
                {p.is_complete ? "●" : p.has_body ? "○" : "·"}
              </span>
              <div className="min-w-0 flex-1">
                <div className="truncate text-xs font-mono text-[#89b4fa]">{p.arxiv_id}</div>
                <Tooltip content={
                  <><span className="text-[#6c7086]">{p.arxiv_id}</span><br /><span className="text-[#cdd6f4]">{p.title}</span></>
                }>
                  <div className="truncate text-xs">{p.title}</div>
                </Tooltip>
              </div>
            </div>
          </RippleBtn>
        );
      })}
    </div>
  );
}
