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
import enSearch from "../assets/screenshots/en-dark-search.jpg";
import zhSearch from "../assets/screenshots/zh-light-search.jpg";
import enUsage from "../assets/screenshots/en-dark-usage.jpg";
import zhUsage from "../assets/screenshots/zh-light-usage.jpg";
import enProjects from "../assets/screenshots/en-dark-library.png";
import zhProjects from "../assets/screenshots/zh-light-library.png";
export const sessionShots = { en: enSessions, zh: zhSessions } as const;
export const searchShots = { en: enSearch, zh: zhSearch } as const;
export const usageShots = { en: enUsage, zh: zhUsage } as const;
export const projectShots = { en: enProjects, zh: zhProjects } as const;
