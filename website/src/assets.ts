import mark from "../assets/repoatlas-mark.png";
import enLibraryDark from "../assets/screenshots/en-dark-library.png";
import enLibraryLight from "../assets/screenshots/en-light-library.png";
import zhLibraryDark from "../assets/screenshots/zh-dark-library.png";
import zhLibraryLight from "../assets/screenshots/zh-light-library.png";
import tasksShot from "../assets/screenshots/en-light-tasks.png";

export const markUrl = mark;
export const tasksShotUrl = tasksShot;

export const heroShots = {
  en: { dark: enLibraryDark, light: enLibraryLight },
  zh: { dark: zhLibraryDark, light: zhLibraryLight },
} as const;
