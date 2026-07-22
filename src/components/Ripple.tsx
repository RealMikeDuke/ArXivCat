import { type ButtonHTMLAttributes } from "react";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  children: React.ReactNode;
}

export default function RippleBtn({ children, className = "", ...props }: Props) {
  return <button className={`transition-colors ${className}`} {...props}>{children}</button>;
}
