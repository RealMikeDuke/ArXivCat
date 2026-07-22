import { useStore, TOAST } from "../store";

export default function Toast() {
  const msg = useStore((s) => s.toastMessage);
  const type = useStore((s) => s.toastType);
  const t = TOAST[type];
  if (!msg) return null;
  return (
    <div className={`fixed left-1/2 bottom-8 z-[9999] -translate-x-1/2 pointer-events-none overflow-hidden rounded shadow-lg animate-[toast-slidein_0.2s_ease-out,toast-slideout_0.35s_cubic-bezier(0.4,0,1,1)_forwards_1s] ${t.bg}`}>
      <div className="px-4 py-2 text-sm text-[#1e1e2e]">{msg}</div>
      <div className="h-0.5 w-full bg-[#1e1e2e]/20">
        <div className={`h-full animate-[toast-shrink_1s_linear_forwards] ${t.bar}`} />
      </div>
    </div>
  );
}
