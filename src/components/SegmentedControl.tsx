interface SegmentedControlProps {
  options: { value: string; label: string }[];
  value: string;
  onChange: (value: string) => void;
}

export default function SegmentedControl({ options, value, onChange }: SegmentedControlProps) {
  return (
    <div className="flex overflow-hidden rounded bg-[#313244]">
      {options.map((opt, i) => (
        <button
          key={opt.value}
          onClick={() => onChange(opt.value)}
          className={`px-2.5 py-0.5 text-xs transition-all duration-150 ${
            opt.value === value
              ? "bg-[#89b4fa] text-[#1e1e2e] font-medium"
              : "text-[#a6adc8] hover:bg-[#45475a] hover:text-[#cdd6f4]"
          } ${i > 0 ? "border-l border-[#45475a]" : ""}`}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}
