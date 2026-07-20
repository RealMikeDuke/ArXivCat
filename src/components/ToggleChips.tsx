interface ToggleChipsProps<T extends string> {
  options: { key: T; label: string }[];
  selection: Record<T, boolean>;
  onChange: (key: T) => void;
  locked?: string[];
}

export default function ToggleChips<T extends string>({ options, selection, onChange, locked }: ToggleChipsProps<T>) {
  const lockedSet = locked ? new Set(locked) : new Set();
  return (
    <div className="flex gap-1.5">
      {options.map((opt) => {
        const active = selection[opt.key];
        const isLocked = lockedSet.has(opt.key);
        return (
          <button
            key={opt.key}
            onClick={() => { if (!isLocked) onChange(opt.key); }}
            className={`rounded px-2 py-0.5 text-xs transition-colors duration-150 ${
              isLocked
                ? "bg-[#89b4fa] text-[#1e1e2e] cursor-default opacity-70"
                : active
                  ? "bg-[#89b4fa] text-[#1e1e2e]"
                  : "bg-[#313244] text-[#a6adc8] hover:bg-[#45475a] hover:text-[#cdd6f4]"
            }`}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}
