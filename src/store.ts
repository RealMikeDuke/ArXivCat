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

export type ViewMode = "body" | "appendix" | "note" | "description";

export interface ChatMessage {
  speaker: string;
  content: string;
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
  chatModel: string;
  deepThinking: boolean;
  logMessages: string[];

  initWorkspace: () => Promise<void>;
  openWorkspace: (path: string) => Promise<void>;
  refreshPapers: () => Promise<void>;
  selectPaper: (paper: Paper) => Promise<void>;
  switchView: (view: ViewMode) => void;
  saveNote: (content: string) => Promise<void>;
  stripComments: () => Promise<void>;
  scanPdfs: () => Promise<void>;
  downloadAll: () => Promise<void>;
  extractPaper: (arxivId: string) => Promise<void>;
  toggleSideChat: () => void;
  toggleGlobalChat: () => void;
  addLog: (msg: string) => void;
  setChatModel: (model: string) => void;
  toggleDeepThinking: () => void;
  addChatMessage: (msg: ChatMessage) => void;
  clearChat: () => void;
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
  chatModel: "Flash",
  deepThinking: true,
  logMessages: [],

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
        sideChatOpen: true,
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
    get().addLog("[INFO] Starting batch download...");
    try {
      const count = await invoke<number>("download_all", { workspacePath });
      get().addLog(`[OK] Processed ${count} papers`);
      await get().refreshPapers();
    } catch (e) {
      get().addLog(`[ERROR] Batch download failed: ${e}`);
    }
  },

  extractPaper: async (arxivId: string) => {
    get().addLog(`[INFO] Extracting ${arxivId}...`);
    try {
      await invoke<string>("extract_paper", { arxivId });
      get().addLog(`[OK] Extracted ${arxivId}`);
      // Show the result and refresh
      await get().refreshPapers();
    } catch (e) {
      get().addLog(`[ERROR] Extraction failed: ${e}`);
    }
  },

  toggleSideChat: () => set((s) => ({ sideChatOpen: !s.sideChatOpen })),
  toggleGlobalChat: () => set((s) => ({ globalChatOpen: !s.globalChatOpen })),

  addLog: (msg: string) =>
    set((s) => ({
      logMessages: [...s.logMessages.slice(-99), msg],
    })),

  setChatModel: (model: string) => set({ chatModel: model }),
  toggleDeepThinking: () => set((s) => ({ deepThinking: !s.deepThinking })),
  addChatMessage: (msg: ChatMessage) =>
    set((s) => ({ chatMessages: [...s.chatMessages, msg] })),
  clearChat: () => set({ chatMessages: [] }),
}));
