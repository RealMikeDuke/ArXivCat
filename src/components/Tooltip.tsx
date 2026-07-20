import { useState, useRef, useCallback, type ReactNode } from "react";

interface TooltipProps {
  content: ReactNode;
  children: ReactNode;
  delay?: number;
}

export default function Tooltip({ content, children, delay = 500 }: TooltipProps) {
  const [visible, setVisible] = useState(false);
  const [pos, setPos] = useState({ top: 0, left: 0 });
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const targetRef = useRef<HTMLDivElement>(null);

  const show = useCallback(() => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      if (targetRef.current) {
        const rect = targetRef.current.getBoundingClientRect();
        setPos({ top: rect.top, left: rect.left });
      }
      setVisible(true);
    }, delay);
  }, [delay]);

  const hide = useCallback(() => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    setVisible(false);
  }, []);

  return (
    <div ref={targetRef} className="inline-flex" onMouseEnter={show} onMouseLeave={hide}>
      {children}
      {visible && (
        <div
          style={{ top: pos.top, left: pos.left, transform: "translateY(calc(-100% - 8px))" }}
          className="fixed z-[9999] max-w-xs rounded-md border border-[#45475a] bg-[#1e1e2e] px-3 py-1.5 shadow-xl"
        >
          <div className="text-xs leading-relaxed">{content}</div>
          <div className="absolute -bottom-1 left-3 h-2 w-2 rotate-45 border-b border-r border-[#45475a] bg-[#1e1e2e]" />
        </div>
      )}
    </div>
  );
}
