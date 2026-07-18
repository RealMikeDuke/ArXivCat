import { type ButtonHTMLAttributes } from "react";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  children: React.ReactNode;
}

export default function RippleBtn({ children, className = "", ...props }: Props) {
  return <button className={className} {...props}>{children}</button>;
}
