import type { Icon, IconWeight } from "@phosphor-icons/react";

type NavigationMotion = "sessions" | "tasks" | "ports" | "footprints" | "attention" | "help" | "settings" | "usage";

/** Selection feedback stays on the glyph, leaving the button and label stationary. */
export function NavigationIcon({ icon: Glyph, motion, active, weight = "regular" }: {
  icon: Icon;
  motion: NavigationMotion;
  active: boolean;
  weight?: IconWeight;
}) {
  return <Glyph className={`navigation-icon navigation-icon--${motion}`} data-active={active} weight={weight} aria-hidden="true" focusable="false" />;
}
