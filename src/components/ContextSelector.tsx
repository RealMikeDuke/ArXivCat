import { Paper, ContextSelection } from "../store";
import ToggleChips from "./ToggleChips";

const ALL_FIELDS = ["body", "appendix", "description", "note"] as const;

interface ContextSelectorProps {
  papers: Paper[];
  selection: Record<string, ContextSelection>;
  lockedFields: Record<string, string[]>;
  onChange: (folderName: string, sel: ContextSelection) => void;
}

export default function ContextSelector({ papers, selection, lockedFields, onChange }: ContextSelectorProps) {
  return (
    <div className="max-h-[40%] overflow-y-auto border-b border-[#313244] px-4 py-2">
      <div className="mb-2 flex items-center gap-2 text-xs font-semibold text-[#a6adc8]">
        <span>Context</span>
        <div className="flex gap-1 ml-auto">
          {ALL_FIELDS.map((field) => {
            const allOn = papers.every((p) => selection[p.folder_name]?.[field] ?? false);
            const allLocked = papers.length > 0 && papers.every((p) => (lockedFields[p.folder_name] || []).includes(field));
            return (
              <button key={field} onClick={() => {
                if (allLocked) return;
                for (const p of papers) {
                  const cur = selection[p.folder_name] || { body: false, appendix: false, description: false, note: false };
                  onChange(p.folder_name, { ...cur, [field]: !allOn });
                }
              }}
                className={`rounded px-2 py-0.5 text-xs transition-colors ${allLocked ? "bg-[#89b4fa] text-[#1e1e2e] opacity-70 cursor-default" : allOn ? "bg-[#89b4fa] text-[#1e1e2e]" : "bg-[#313244] text-[#a6adc8]"}`}>
                All {field.charAt(0).toUpperCase() + field.slice(1)}
              </button>
            );
          })}
        </div>
      </div>
      {papers.map((p) => {
        const sel = selection[p.folder_name] || { body: false, appendix: false, description: false, note: false };
        const paperLocked = lockedFields[p.folder_name] || [];
        return (
          <div key={p.folder_name} className="mb-1.5 flex items-center gap-2 text-xs">
            <span className="w-28 truncate text-[#89b4fa]" title={`${p.arxiv_id} | ${p.title}`}>{p.arxiv_id}</span>
            <ToggleChips
              options={[
                { key: "body", label: "Body" },
                { key: "appendix", label: "Appendix" },
                { key: "description", label: "Description" },
                { key: "note", label: "Note" },
              ]}
              selection={{ ...sel, ...Object.fromEntries(paperLocked.map((k) => [k, true])) }}
              locked={paperLocked}
              onChange={(key) => { if (!paperLocked.includes(key)) onChange(p.folder_name, { ...sel, [key]: !sel[key] }); }}
            />
          </div>
        );
      })}
    </div>
  );
}
