import mark from "../assets/repoatlas-mark.png";
import enLibraryDark from "../assets/screenshots/en-dark-workspace.jpg";
import enLibraryLight from "../assets/screenshots/en-light-workspace.jpg";
import zhLibraryDark from "../assets/screenshots/zh-dark-workspace.jpg";
import zhLibraryLight from "../assets/screenshots/zh-light-workspace.jpg";
import enTasksShot from "../assets/screenshots/en-light-tasks.png";
import zhTasksShot from "../assets/screenshots/zh-light-tasks.png";

export const markUrl = mark;
export const tasksShots = { en: enTasksShot, zh: zhTasksShot } as const;

export const heroShots = {
  en: { dark: enLibraryDark, light: enLibraryLight },
  zh: { dark: zhLibraryDark, light: zhLibraryLight },
} as const;

import enSessions from "../assets/screenshots/en-dark-sessions.jpg";
import zhSessions from "../assets/screenshots/zh-light-sessions.jpg";
import enResume from "../assets/screenshots/en-dark-resume.jpg";
import zhResume from "../assets/screenshots/zh-light-resume.jpg";
export const sessionShots = { en: enSessions, zh: zhSessions } as const;
export const resumeShots = { en: enResume, zh: zhResume } as const;
