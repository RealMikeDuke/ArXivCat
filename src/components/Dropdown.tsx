import { useState, useRef, useEffect, type ReactNode, type RefObject } from "react";

interface DropdownProps {
  open: boolean;
  onClose: () => void;
  anchorRef: RefObject<HTMLElement | null>;
  children: ReactNode;
  width?: number;
}

export default function Dropdown({ open, onClose, anchorRef, children, width }: DropdownProps) {
  const [alive, setAlive] = useState(false);
  const [visible, setVisible] = useState(false);
  const prevOpen = useRef(false);
  const panelRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ top: 0, left: 0 });

  useEffect(() => {
    if (open && !prevOpen.current) { setAlive(true); }
    if (!open && prevOpen.current) {
      setVisible(false);
      const t = setTimeout(() => setAlive(false), 120);
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

  // Position
  useEffect(() => {
    if (!alive || !anchorRef?.current) return;
    const rect = anchorRef.current.getBoundingClientRect();
    setPos({ top: rect.bottom + 4, left: Math.max(8, Math.min(rect.left, window.innerWidth - (width ?? 300))) });
  }, [alive, anchorRef, width]);

  // Click-outside + Escape
  useEffect(() => {
    if (!alive) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    const onClick = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node) &&
          anchorRef.current && !anchorRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onClick);
    return () => {
      window.removeEventListener("keydown", onKey);
      document.removeEventListener("mousedown", onClick);
    };
  }, [alive, onClose, anchorRef]);

  if (!alive) return null;

  return (
    <div ref={panelRef} style={{
      position: "fixed",
      top: pos.top, left: pos.left,
      zIndex: 9999,
      opacity: visible ? 1 : 0,
      transform: visible ? "scale(1)" : "scale(0.95)",
      transition: "all 0.12s ease-out",
      transformOrigin: "top left",
    }}
      className="rounded-lg border border-[#45475a] bg-[#1e1e2e] shadow-2xl">
      {children}
    </div>
  );
}
