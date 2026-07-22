interface ChatTitleBarProps {
  title: string;
}

export default function ChatTitleBar({ title }: ChatTitleBarProps) {
  return (
    <div className="flex justify-center bg-[#1e1e2e] py-1">
      <span className="truncate rounded bg-[#313244] px-3 py-0.5 text-xs text-[#cdd6f4]">{title}</span>
    </div>
  );
}
