import { useState, useRef, useEffect } from "react";

export interface SelectOption {
  value: string;
  label: string;
}

interface SelectProps {
  options: SelectOption[];
  value: string;
  onChange: (value: string) => void;
  className?: string;
  buttonClassName?: string;
  chevron?: boolean;
}

export default function Select({ options, value, onChange, className = "", buttonClassName = "", chevron = true }: SelectProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    if (open) document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const selected = options.find((o) => o.value === value);

  return (
    <div ref={ref} className={`relative select-none ${className}`}>
      <button
        onClick={() => setOpen(!open)}
        className={`flex items-center gap-1.5 rounded px-2 py-0.5 text-xs outline-none transition-colors ${buttonClassName || "bg-[#313244] text-[#cdd6f4] hover:bg-[#45475a]"}`}
      >
        <span>{selected?.label ?? value}</span>
        {chevron && (
          <svg
            className={`h-3 w-3 text-[#6c7086] transition-transform duration-150 ${open ? "rotate-180" : ""}`}
            viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2"
          >
            <polyline points="4 6 8 10 12 6" />
          </svg>
        )}
      </button>
      {open && (
        <div className="absolute right-0 top-full z-50 mt-1 min-w-full overflow-hidden rounded border border-[#45475a] bg-[#1e1e2e] shadow-xl">
          {options.map((opt) => (
            <button
              key={opt.value}
              onClick={() => { onChange(opt.value); setOpen(false); }}
              className={`block w-full whitespace-nowrap px-3 py-1.5 text-left text-xs transition-colors ${
                opt.value === value
                  ? "bg-[#89b4fa] text-[#1e1e2e]"
                  : "text-[#cdd6f4] hover:bg-[#313244]"
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
