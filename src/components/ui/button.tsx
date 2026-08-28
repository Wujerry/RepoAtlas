import { cva, type VariantProps } from "class-variance-authority";
import type { ButtonHTMLAttributes } from "react";
import { SpinnerGap } from "@phosphor-icons/react";
import { cn } from "../../lib/cn";

const buttonVariants = cva(
  "button",
  {
    variants: {
      variant: {
        primary: "button-primary",
        ghost: "button-ghost",
        quiet: "button-quiet",
        danger: "button-danger",
      },
      size: {
        sm: "button-sm",
        md: "button-md",
        lg: "button-lg",
        icon: "button-icon",
      },
    },
    defaultVariants: { variant: "ghost", size: "sm" },
  },
);

export function Button({
  className,
  variant,
  size,
  loading = false,
  children,
  disabled,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & VariantProps<typeof buttonVariants> & { loading?: boolean }) {
  return (
    <button className={cn(buttonVariants({ variant, size }), className)} disabled={disabled || loading} aria-busy={loading || undefined} {...props}>
      {loading && <SpinnerGap className="button-spinner" aria-hidden="true" />}
      {children}
    </button>
  );
}
