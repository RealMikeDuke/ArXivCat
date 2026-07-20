import { useStore } from "../store";

export default function Toast() {
  const msg = useStore((s) => s.toastMessage);
  if (!msg) return null;
  return (
    <div className="fixed left-1/2 bottom-8 z-[9999] -translate-x-1/2 pointer-events-none overflow-hidden rounded bg-[#a6e3a1] shadow-lg animate-[toast-slideout_0.35s_cubic-bezier(0.4,0,1,1)_forwards_1.5s]">
      <div className="px-4 py-2 text-sm text-[#1e1e2e]">{msg}</div>
      <div className="h-0.5 w-full bg-[#1e1e2e]/20">
        <div className="h-full bg-[#1e1e2e]/60 animate-[toast-shrink_1.5s_linear_forwards]" />
      </div>
    </div>
  );
}
