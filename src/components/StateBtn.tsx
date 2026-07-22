import { type ButtonHTMLAttributes } from "react";
import { BTN } from "../store";

export type BtnStatus = "idle" | "running" | "done" | "error";

const STATUS_CLASS: Record<BtnStatus, string> = {
  idle: BTN.blue,
  running: `${BTN.blue} relative overflow-hidden`,
  done: BTN.green,
  error: BTN.red,
};

interface StateBtnProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  status?: BtnStatus;
  children: React.ReactNode;
}

export default function StateBtn({ status = "idle", children, className = "", ...props }: StateBtnProps) {
  return (
    <button
      className={`rounded px-3 py-2 text-left text-xs transition-colors ${STATUS_CLASS[status]} ${className}`}
      {...props}
    >
      {status === "running" && (
        <span className="absolute inset-0 animate-pulse rounded bg-white/10" />
      )}
      {children}
    </button>
  );
}
