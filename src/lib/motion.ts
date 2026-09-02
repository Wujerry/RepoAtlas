export const MOTION_EASE: [number, number, number, number] = [0.2, 0.8, 0.2, 1];

export const MOTION = {
  press: 0.14,
  state: 0.18,
  overlay: 0.24,
} as const;

export const overlayMotion = {
  initial: { opacity: 0, y: 8, scale: 0.98 },
  animate: { opacity: 1, y: 0, scale: 1 },
  exit: { opacity: 0, y: 6, scale: 0.985 },
  transition: { duration: MOTION.overlay, ease: MOTION_EASE },
};

export const fadeMotion = {
  initial: { opacity: 0 },
  animate: { opacity: 1 },
  exit: { opacity: 0 },
  transition: { duration: MOTION.state, ease: MOTION_EASE },
};

export function pageMotion(direction: 1 | -1 = 1) {
  return {
    initial: { opacity: 0, x: 4 * direction },
    animate: { opacity: 1, x: 0 },
    exit: { opacity: 0, x: -4 * direction },
    transition: { duration: MOTION.state, ease: MOTION_EASE },
  };
}
