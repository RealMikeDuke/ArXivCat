import { useStore } from "../store";

export default function PaperList() {
  const { papers, currentPaper, selectPaper } = useStore();

  if (papers.length === 0) {
    return (
      <div className="p-4 text-sm text-[#a6adc8]">
        No papers in workspace
      </div>
    );
  }

  return (
    <div className="flex flex-col">
      <div className="border-b border-[#313244] px-3 py-2 text-xs font-semibold text-[#a6adc8]">
        Papers ({papers.length})
      </div>
      {papers.map((p) => {
        const isSelected = currentPaper?.folder_name === p.folder_name;
        return (
          <button
            key={p.folder_name}
            onClick={() => selectPaper(p)}
            className={`px-3 py-2 text-left text-sm transition-colors ${
              isSelected
                ? "bg-[#45475a] text-[#cdd6f4]"
                : "text-[#a6adc8] hover:bg-[#313244] hover:text-[#cdd6f4]"
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
                <div className="truncate text-xs">{p.title}</div>
              </div>
            </div>
          </button>
        );
      })}
    </div>
  );
}
