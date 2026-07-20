import { useState, useRef, useCallback, useEffect, type ReactNode } from "react";
import RippleBtn from "./Ripple";

interface DialogProps {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  children: ReactNode;
  headerExtra?: ReactNode;
  defaultWidth?: number;
  defaultHeight?: number;
  minWidth?: number;
  minHeight?: number;
}

export default function Dialog({
  open, onClose, title, children, headerExtra,
  defaultWidth = 600, defaultHeight = 400,
  minWidth = 400, minHeight = 300,
}: DialogProps) {
  const [alive, setAlive] = useState(false);
  const [visible, setVisible] = useState(false);
  const prevOpen = useRef(false);

  useEffect(() => {
    if (open && !prevOpen.current) {
      setAlive(true);
    }
    if (!open && prevOpen.current) {
      setVisible(false);
      const t = setTimeout(() => setAlive(false), 150);
      prevOpen.current = false;
      return () => clearTimeout(t);
    }
    prevOpen.current = open;
  }, [open]);

  useEffect(() => {
    if (!alive) return;
    const raf = requestAnimationFrame(() => {
      requestAnimationFrame(() => setVisible(true));
    });
    return () => cancelAnimationFrame(raf);
  }, [alive]);

  const [pos, setPos] = useState<{ x: number; y: number } | null>(null);
  const [size, setSize] = useState({ w: defaultWidth, h: defaultHeight });
  const [resizing, setResizing] = useState(false);
  const resizeStart = useRef({ x: 0, y: 0, w: 0, h: 0 });
  const [moving, setMoving] = useState(false);
  const moveStart = useRef({ x: 0, y: 0, px: 0, py: 0 });
  const titleRef = useRef<HTMLDivElement>(null);

  const onResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    if (!pos) {
      const rect = titleRef.current?.parentElement?.getBoundingClientRect();
      if (rect) setPos({ x: rect.left, y: rect.top });
    }
    resizeStart.current = { x: e.clientX, y: e.clientY, w: size.w, h: size.h };
    setResizing(true);
  }, [size, pos]);

  useEffect(() => {
    if (!resizing) return;
    const onMove = (e: MouseEvent) => {
      const dx = e.clientX - resizeStart.current.x;
      const dy = e.clientY - resizeStart.current.y;
      const pw = window.innerWidth, ph = window.innerHeight;
      const px = pos?.x ?? (pw - size.w) / 2;
      const py = pos?.y ?? (ph - size.h) / 2;
      setSize({
        w: Math.max(minWidth, Math.min(pw - px - 20, resizeStart.current.w + dx)),
        h: Math.max(minHeight, Math.min(ph - py - 20, resizeStart.current.h + dy)),
      });
    };
    const onUp = () => setResizing(false);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => { window.removeEventListener("mousemove", onMove); window.removeEventListener("mouseup", onUp); };
  }, [resizing, pos, minWidth, minHeight, size.w, size.h]);

  const onMoveStart = useCallback((e: React.MouseEvent) => {
    if (resizing) return;
    e.preventDefault();
    const rect = titleRef.current?.parentElement?.getBoundingClientRect();
    moveStart.current = {
      x: e.clientX, y: e.clientY,
      px: pos?.x ?? (rect ? rect.left : 0),
      py: pos?.y ?? (rect ? rect.top : 0),
    };
    setMoving(true);
  }, [resizing, pos]);

  useEffect(() => {
    if (!moving) return;
    const onMove = (e: MouseEvent) => {
      setPos({
        x: Math.max(20, Math.min(window.innerWidth - size.w - 20, moveStart.current.px + e.clientX - moveStart.current.x)),
        y: Math.max(20, Math.min(window.innerHeight - size.h - 20, moveStart.current.py + e.clientY - moveStart.current.y)),
      });
    };
    const onUp = () => setMoving(false);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => { window.removeEventListener("mousemove", onMove); window.removeEventListener("mouseup", onUp); };
  }, [moving, size.w, size.h]);

  if (!alive) return null;

  return (
    <div style={{ opacity: visible ? 1 : 0, transition: "opacity 0.15s ease-out" }}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onMouseDown={(e) => { if (e.target === e.currentTarget && !moving && !resizing) onClose(); }}>
      <div style={{
        width: size.w, height: size.h,
        transform: visible ? "scale(1)" : "scale(0.95)",
        opacity: visible ? 1 : 0,
        transition: "all 0.15s ease-out",
        ...(pos ? { position: "fixed", top: pos.y, left: pos.x, margin: 0 } : {}),
      }}
        className="relative flex flex-col rounded-lg border border-[#45475a] bg-[#1e1e2e] shadow-2xl">
        <div ref={titleRef} className="flex items-center gap-2 border-b border-[#313244] px-4 py-3 cursor-grab active:cursor-grabbing select-none" onMouseDown={onMoveStart}>
          <span className="font-semibold text-[#cdd6f4] text-sm">{title}</span>
          <div className="flex-1" />
          {headerExtra}
          <RippleBtn onClick={onClose} className="rounded bg-[#313244] px-3 py-1 text-xs text-[#a6adc8] hover:bg-[#45475a] hover:text-[#cdd6f4] transition-colors">Close</RippleBtn>
        </div>
        {children}
        <div onMouseDown={onResizeStart}
          className="absolute bottom-0 right-0 z-50 h-5 w-5 cursor-nw-resize flex items-end justify-end overflow-hidden opacity-60 hover:opacity-100 transition-opacity">
          <svg viewBox="0 0 20 20" className="h-full w-full">
            <defs>
              <linearGradient id="rg" x1="0" y1="1" x2="1" y2="0">
                <stop offset="0%" stopColor="transparent" />
                <stop offset="60%" stopColor="#585b70" stopOpacity="0.15" />
                <stop offset="90%" stopColor="#585b70" stopOpacity="0.4" />
                <stop offset="100%" stopColor="#89b4fa" stopOpacity="0.7" />
              </linearGradient>
            </defs>
            <rect x="0" y="0" width="20" height="20" fill="url(#rg)" rx="2" />
            <line x1="14" y1="18" x2="18" y2="14" stroke="#89b4fa" strokeWidth="1.5" strokeLinecap="round" opacity="0.6" />
            <line x1="10" y1="18" x2="18" y2="10" stroke="#89b4fa" strokeWidth="1" strokeLinecap="round" opacity="0.35" />
            <line x1="6" y1="18" x2="18" y2="6" stroke="#89b4fa" strokeWidth="0.5" strokeLinecap="round" opacity="0.2" />
          </svg>
        </div>
      </div>
    </div>
  );
}
