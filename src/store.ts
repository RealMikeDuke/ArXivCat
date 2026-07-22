import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface Paper {
  arxiv_id: string;
  title: string;
  folder_name: string;
  has_body: boolean;
  description_ready: boolean;
  is_complete: boolean;
}

export type ViewMode = "body" | "appendix" | "note" | "description" | "pdf";
export type ReasoningEffort = "off" | "low" | "medium" | "high" | "max";

export interface ChatMessage {
  speaker: string;
  content: string;
}

export interface ContextSelection {
  body: boolean;
  appendix: boolean;
  description: boolean;
  note: boolean;
}

export const DEFAULT_SIDE_SELECTION: ContextSelection = {
  body: false,
  appendix: false,
  description: false,
  note: false,
};

export const EFFORT_BUTTON_CLASS: Record<string, string> = {
  off: "bg-[#313244] text-[#6c7086] hover:bg-[#45475a]",
  low: "bg-[#3a3b52] text-[#8a9bb8] hover:bg-[#45475a]",
  medium: "bg-[#45537b] text-[#aabee0] hover:bg-[#586b99]",
  high: "bg-[#5a7bb5] text-[#eef4ff] hover:bg-[#6b8fcc]",
  max: "bg-[#89b4fa] text-[#1e1e2e] font-medium hover:bg-[#9ec5ff]",
};

export type ToastType = "success" | "info" | "error" | "warning";

export const TOAST: Record<ToastType, { bg: string; bar: string }> = {
  success: { bg: "bg-[#a6e3a1]", bar: "bg-[#1e1e2e]/60" },
  info: { bg: "bg-[#89b4fa]", bar: "bg-[#1e1e2e]/60" },
  error: { bg: "bg-[#f38ba8]", bar: "bg-[#1e1e2e]/60" },
  warning: { bg: "bg-[#f9e2af]", bar: "bg-[#1e1e2e]/60" },
};

export const BTN = {
  surface0: "bg-[#313244] hover:bg-[#45475a] text-[#a6adc8] hover:text-[#cdd6f4]",
  surface1: "bg-[#45475a] hover:bg-[#585b70] text-[#cdd6f4]",
  blue: "bg-[#89b4fa] hover:bg-[#b4d0fb] text-[#1e1e2e]",
  green: "bg-[#a6e3a1] hover:bg-[#b8ebc0] text-[#1e1e2e]",
  red: "bg-[#f38ba8] hover:bg-[#f5a0b9] text-[#1e1e2e]",
  ghost: "hover:bg-[#313244] text-[#a6adc8] hover:text-[#cdd6f4]",
} as const;

export const DEFAULT_GLOBAL_SELECTION: ContextSelection = {
  body: false,
  appendix: false,
  description: false,
  note: false,
};

interface ChatState {
  sessionId: string | null;
  streaming: boolean;
  status: string;
  bufferTokens: string[];
}

interface DownloadState {
  inProgress: boolean;
  current: number;
  total: number;
}

interface StoreState {
  workspacePath: string | null;
  papers: Paper[];
  currentPaper: Paper | null;
  previewContent: Record<string, string>;
  currentView: ViewMode;
  sideChatOpen: boolean;
  globalChatOpen: boolean;
  chatMessages: ChatMessage[];
  sideChatModel: string;
  globalChatModel: string;
  sideReasoningEffort: ReasoningEffort;
  globalReasoningEffort: ReasoningEffort;
  logMessages: string[];
  logOpen: boolean;
  toastMessage: string | null;
  toastType: ToastType;
  leftPanelOpen: boolean;
  drafts: Record<string, string>;

  sideContextSelection: ContextSelection;
  globalContextSelection: Record<string, ContextSelection>;

  chat: ChatState;
  download: DownloadState;

  initWorkspace: () => Promise<void>;
  openWorkspace: (path: string) => Promise<void>;
  refreshPapers: () => Promise<void>;
  selectPaper: (paper: Paper) => Promise<void>;
  switchView: (view: ViewMode) => void;
  saveNote: (content: string) => Promise<void>;
  saveDescription: (content: string) => Promise<void>;
  stripComments: () => Promise<void>;
  scanPdfs: () => Promise<void>;
  downloadAll: () => Promise<void>;
  downloadPaper: (arxivId: string) => Promise<void>;
  toggleSideChat: () => void;
  toggleGlobalChat: () => void;
  addLog: (msg: string) => void;
  toggleLog: () => void;
  showToast: (msg: string, type?: ToastType) => void;
  toggleLeftPanel: () => void;
  setSideChatModel: (model: string) => void;
  setGlobalChatModel: (model: string) => void;
  setSideReasoningEffort: (effort: ReasoningEffort) => void;
  setGlobalReasoningEffort: (effort: ReasoningEffort) => void;

  getDraftKey: () => string | null;
  saveDraft: (key: string, content: string) => void;
  clearDraft: (key: string) => void;
  setSideSelection: (sel: ContextSelection) => void;
  setGlobalSelection: (folderName: string, sel: ContextSelection) => void;
}

export const useStore = create<StoreState>((set, get) => ({
  workspacePath: null,
  papers: [],
  currentPaper: null,
  previewContent: {} as Record<string, string>,
  currentView: "body",
  sideChatOpen: false,
  globalChatOpen: false,
  chatMessages: [],
  sideChatModel: "Flash",
  globalChatModel: "Flash",
  sideReasoningEffort: "low",
  globalReasoningEffort: "low",
  logMessages: [],
  logOpen: false,
  toastMessage: null,
  toastType: "success",
  leftPanelOpen: true,
  drafts: (() => {
    try {
      const d: Record<string, string> = {};
      for (let i = 0; i < localStorage.length; i++) {
        const k = localStorage.key(i);
        if (k?.startsWith("ac_draft_")) d[k.slice(9)] = localStorage.getItem(k) || "";
      }
      return d;
    } catch { return {}; }
  })(),

  sideContextSelection: { ...DEFAULT_SIDE_SELECTION },
  globalContextSelection: {},

  chat: { sessionId: null, streaming: false, status: "", bufferTokens: [] },
  download: { inProgress: false, current: 0, total: 0 },

  initWorkspace: async () => {
    try {
      const path = await invoke<string | null>("get_last_workspace");
      if (path) {
        await get().openWorkspace(path);
      }
    } catch {
      // no saved workspace
    }
  },

  openWorkspace: async (path: string) => {
    try {
      const papers = await invoke<Paper[]>("open_workspace", { path });
      set({ workspacePath: path, papers, currentPaper: null, previewContent: {} });
    } catch (e) {
      get().addLog(`[ERROR] Failed to open workspace: ${e}`);
    }
  },

  refreshPapers: async () => {
    const { workspacePath } = get();
    if (!workspacePath) return;
    try {
      const papers = await invoke<Paper[]>("get_paper_list", { workspacePath });
      set({ papers });
    } catch (e) {
      get().addLog(`[ERROR] Failed to refresh papers: ${e}`);
    }
  },

  selectPaper: async (paper: Paper) => {
    const { workspacePath } = get();
    if (!workspacePath) return;
    try {
      const content = await invoke<Record<string, string>>("load_paper", {
        workspacePath,
        folderName: paper.folder_name,
      });
      set({
        currentPaper: paper,
        previewContent: content,
        currentView: "body",
      });
    } catch (e) {
      get().addLog(`[ERROR] Failed to load paper: ${e}`);
    }
  },

  switchView: (view: ViewMode) => set({ currentView: view }),

  saveNote: async (content: string) => {
    const { workspacePath, currentPaper } = get();
    if (!workspacePath || !currentPaper) return;
    try {
      await invoke("save_note", {
        workspacePath,
        folderName: currentPaper.folder_name,
        content,
      });
      set((s) => ({
        previewContent: { ...s.previewContent, note: content },
      }));
    } catch (e) {
      get().addLog(`[ERROR] Failed to save note: ${e}`);
    }
  },

  saveDescription: async (content: string) => {
    const { workspacePath, currentPaper } = get();
    if (!workspacePath || !currentPaper) return;
    try {
      await invoke("save_description", {
        workspacePath,
        folderName: currentPaper.folder_name,
        content,
      });
      set((s) => ({
        previewContent: { ...s.previewContent, description: content },
      }));
    } catch (e) {
      get().addLog(`[ERROR] Failed to save description: ${e}`);
    }
  },

  stripComments: async () => {
    const { previewContent, currentView } = get();
    const content = previewContent[currentView];
    if (!content) return;
    try {
      const stripped = await invoke<string>("strip_comments", { content });
      set((s) => ({
        previewContent: { ...s.previewContent, [s.currentView]: stripped },
      }));
    } catch (e) {
      get().addLog(`[ERROR] Failed to strip comments: ${e}`);
    }
  },

  scanPdfs: async () => {
    const { workspacePath } = get();
    if (!workspacePath) return;
    get().addLog("[INFO] Scanning workspace for PDFs...");
    try {
      const count = await invoke<number>("scan_pdfs", { workspacePath });
      get().addLog(`[OK] Found ${count} new papers`);
      await get().refreshPapers();
    } catch (e) {
      get().addLog(`[ERROR] Scan failed: ${e}`);
    }
  },

  downloadAll: async () => {
    const { workspacePath } = get();
    if (!workspacePath) return;
    set({ download: { inProgress: true, current: 0, total: 0 } });
    get().addLog("[INFO] Starting batch download...");
    try {
      await invoke("download_all", { workspacePath });
    } catch (e) {
      get().addLog(`[ERROR] Batch download failed: ${e}`);
      set({ download: { inProgress: false, current: 0, total: 0 } });
    }
  },

  downloadPaper: async (rawInput: string) => {
    const { workspacePath } = get();
    if (!workspacePath) {
      get().addLog("[ERROR] No workspace open");
      return;
    }
    get().addLog(`[INFO] Downloading ${rawInput}...`);
    try {
      const paper = await invoke<Paper>("download_paper", {
        rawInput,
        workspacePath,
      });
      get().addLog(`[OK] ${paper.arxiv_id} → ${paper.folder_name}`);
      await get().refreshPapers();
      set({ currentPaper: paper, previewContent: {} });
      await get().selectPaper(paper);
    } catch (e) {
      get().addLog(`[ERROR] Download failed: ${e}`);
    }
  },

  toggleSideChat: () => set((s) => ({ sideChatOpen: !s.sideChatOpen })),
  toggleGlobalChat: () => set((s) => ({ globalChatOpen: !s.globalChatOpen })),

  addLog: (msg: string) =>
    set((s) => ({
      logMessages: [...s.logMessages.slice(-99), msg],
    })),
  toggleLog: () => set((s) => ({ logOpen: !s.logOpen })),
  showToast: (msg, type = "success") => { set({ toastMessage: msg, toastType: type }); setTimeout(() => set({ toastMessage: null }), 1400); },
  toggleLeftPanel: () => set((s) => ({ leftPanelOpen: !s.leftPanelOpen })),

  setSideChatModel: (model: string) => set({ sideChatModel: model }),
  setGlobalChatModel: (model: string) => set({ globalChatModel: model }),
  setSideReasoningEffort: (effort) => set({ sideReasoningEffort: effort }),
  setGlobalReasoningEffort: (effort) => set({ globalReasoningEffort: effort }),

  getDraftKey: () => {
    const { currentPaper, currentView } = get();
    if (!currentPaper) return null;
    const v = currentView === "note" || currentView === "description" ? currentView : null;
    return v ? `${currentPaper.folder_name}_${v}` : null;
  },

  saveDraft: (key: string, content: string) => {
    set((s) => ({ drafts: { ...s.drafts, [key]: content } }));
    try { localStorage.setItem(`ac_draft_${key}`, content); } catch (e) { get().addLog(`[ERROR] Failed to save draft to localStorage: ${e}`); }
  },

  clearDraft: (key: string) => {
    set((s) => {
      const next = { ...s.drafts };
      delete next[key];
      return { drafts: next };
    });
    try { localStorage.removeItem(`ac_draft_${key}`); } catch (e) { get().addLog(`[ERROR] Failed to clear draft from localStorage: ${e}`); }
  },

  setSideSelection: (sel: ContextSelection) => set({ sideContextSelection: sel }),

  setGlobalSelection: (folderName: string, sel: ContextSelection) =>
    set((s) => ({
      globalContextSelection: { ...s.globalContextSelection, [folderName]: sel },
    })),
}));
