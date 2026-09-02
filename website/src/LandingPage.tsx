import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  motion,
  useMotionValue,
  useReducedMotion,
  useScroll,
  useSpring,
  useTransform,
  type Variants,
} from "framer-motion";
import {
  FolderOpen,
  GitBranch,
  Graph,
  Notebook,
  PlugsConnected,
  TerminalWindow,
} from "@phosphor-icons/react";
import { copy, type Copy, type Locale } from "./copy";
import { heroShots, markUrl, tasksShotUrl } from "./assets";

const EASE: [number, number, number, number] = [0.16, 1, 0.3, 1];

const RELEASES = "https://github.com/wujer/RepoAtlas/releases";
const SOURCE = "https://github.com/wujer/RepoAtlas";
const CONTRIBUTING = "https://github.com/wujer/RepoAtlas/blob/main/CONTRIBUTING.md";
const SECURITY = "https://github.com/wujer/RepoAtlas/security/policy";

const libraryIcons = [FolderOpen, Graph, GitBranch, Notebook, PlugsConnected];

const heroStack: Variants = {
  hidden: {},
  show: { transition: { staggerChildren: 0.09, delayChildren: 0.05 } },
};

const heroRise: Variants = {
  hidden: { opacity: 0, y: 28 },
  show: { opacity: 1, y: 0, transition: { duration: 0.7, ease: EASE } },
};

function usePrefersLight() {
  const [light, setLight] = useState(
    () => window.matchMedia("(prefers-color-scheme: light)").matches,
  );

  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: light)");
    const onChange = (event: MediaQueryListEvent) => setLight(event.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  return light;
}

function Button({
  href,
  variant,
  children,
}: {
  href: string;
  variant: "primary" | "secondary";
  children: string;
}) {
  const reduce = useReducedMotion();
  const skin =
    variant === "primary"
      ? "bg-amber text-amber-ink shadow-[0_10px_28px_rgba(255,163,26,0.16)]"
      : "border border-line bg-panel text-paper hover:bg-raised";
  return (
    <motion.a
      className={
        "inline-flex min-h-11 shrink-0 items-center justify-center whitespace-nowrap rounded-[8px] px-5 text-[13px] font-semibold focus-visible:outline-2 focus-visible:outline-offset-3 focus-visible:outline-amber " +
        skin
      }
      href={href}
      whileHover={reduce ? undefined : { y: -2 }}
      whileTap={reduce ? undefined : { scale: 0.97 }}
      transition={{ type: "spring", stiffness: 320, damping: 22 }}
    >
      {children}
    </motion.a>
  );
}

function Reveal({
  children,
  delay = 0,
  className,
}: {
  children: ReactNode;
  delay?: number;
  className?: string;
}) {
  const reduce = useReducedMotion();
  return (
    <motion.div
      className={className}
      initial={reduce ? false : { opacity: 0, y: 26 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true, amount: 0.25 }}
      transition={{ duration: 0.65, delay, ease: EASE }}
    >
      {children}
    </motion.div>
  );
}

function TiltCard({ children }: { children: ReactNode }) {
  const reduce = useReducedMotion();
  const rotateX = useMotionValue(0);
  const rotateY = useMotionValue(0);
  const springX = useSpring(rotateX, { stiffness: 140, damping: 18 });
  const springY = useSpring(rotateY, { stiffness: 140, damping: 18 });

  return (
    <motion.div
      style={reduce ? undefined : { rotateX: springX, rotateY: springY, transformPerspective: 1000 }}
      onMouseMove={(event) => {
        const rect = event.currentTarget.getBoundingClientRect();
        const px = (event.clientX - rect.left) / rect.width - 0.5;
        const py = (event.clientY - rect.top) / rect.height - 0.5;
        rotateY.set(px * 5);
        rotateX.set(-py * 5);
      }}
      onMouseLeave={() => {
        rotateX.set(0);
        rotateY.set(0);
      }}
    >
      {children}
    </motion.div>
  );
}

function Hero({ t, locale }: { t: Copy; locale: Locale }) {
  const reduce = useReducedMotion();
  const light = usePrefersLight();
  const heroShot = heroShots[locale][light ? "light" : "dark"];
  const shotWrapRef = useRef<HTMLDivElement>(null);
  const { scrollYProgress } = useScroll({
    target: shotWrapRef,
    offset: ["start end", "end start"],
  });
  const shotY = useTransform(scrollYProgress, [0, 1], [44, -44]);

  return (
    <section className="relative overflow-hidden">
      <span aria-hidden className="glow -right-24 -top-40 md:-right-10" />
      <div className="mx-auto grid w-full max-w-[1400px] items-center gap-10 px-5 pb-16 pt-10 md:min-h-[88dvh] md:grid-cols-[minmax(0,0.92fr)_minmax(0,1.08fr)] md:gap-12 md:px-8 md:pb-20 lg:gap-16">
        <motion.div variants={heroStack} initial="hidden" animate="show" className="max-w-[34rem]">
          <h1 className="text-[clamp(2.2rem,4.6vw,3.9rem)] font-semibold leading-[1.14] tracking-[-0.03em]">
            {t.heroHeadline.map((line) => (
              <motion.span key={line.text} variants={heroRise} className="block">
                {line.accent ? (
                  <span className="relative inline-block">
                    {line.text}
                    <motion.span
                      aria-hidden
                      className="underline-bar"
                      initial={reduce ? false : { scaleX: 0 }}
                      animate={{ scaleX: 1 }}
                      transition={{ duration: 0.6, delay: 0.95, ease: EASE }}
                    />
                  </span>
                ) : (
                  line.text
                )}
              </motion.span>
            ))}
          </h1>
          <motion.p
            variants={heroRise}
            className="mt-5 max-w-[46ch] text-[17px] leading-relaxed text-mute"
          >
            {t.heroSub}
          </motion.p>
          <motion.div variants={heroRise} className="mt-8 flex flex-wrap gap-2.5">
            <Button href={RELEASES} variant="primary">
              {t.ctaPrimary}
            </Button>
            <Button href="#tasks" variant="secondary">
              {t.ctaSecondary}
            </Button>
          </motion.div>
        </motion.div>

        <motion.div
          ref={shotWrapRef}
          initial={reduce ? false : { opacity: 0, y: 36 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 0.3, ease: EASE }}
        >
          <motion.div style={reduce ? undefined : { y: shotY }} className="relative">
            <figure className="hero-shot relative overflow-hidden rounded-[16px] border border-line bg-panel">
              <img
                src={heroShot}
                alt={t.heroImageAlt}
                width={1600}
                height={1000}
                className="block aspect-[16/10] w-full object-cover object-top"
              />
              {!reduce && (
                <motion.span
                  aria-hidden
                  className="scanline"
                  initial={{ top: "-30%", opacity: 0 }}
                  animate={{ top: ["-30%", "110%"], opacity: [0, 1, 1, 0] }}
                  transition={{
                    duration: 2,
                    delay: 1.1,
                    ease: "easeInOut",
                    times: [0, 0.2, 0.8, 1],
                  }}
                />
              )}
            </figure>
          </motion.div>
        </motion.div>
      </div>
    </section>
  );
}

function Marquee({ t }: { t: Copy }) {
  return (
    <section aria-label={t.marqueeAria} className="marquee border-y border-line py-4">
      <div aria-hidden className="marquee-track">
        {[0, 1].map((group) => (
          <ul key={group} className="marquee-group">
            {t.marqueeCommands.map((command) => (
              <li
                key={command}
                className="whitespace-nowrap font-mono text-[13px] text-mute"
              >
                <span className="mr-2 text-amber">$</span>
                {command}
              </li>
            ))}
          </ul>
        ))}
      </div>
    </section>
  );
}

function Tasks({ t }: { t: Copy }) {
  return (
    <section id="tasks" className="border-t border-line">
      <div className="mx-auto w-full max-w-[1400px] px-5 py-20 md:px-8 md:py-28">
        <Reveal>
          <h2 className="max-w-[24ch] text-[clamp(1.9rem,3.6vw,3.1rem)] font-semibold leading-[1.14] tracking-[-0.03em]">
            {t.tasksTitle}
          </h2>
          <p className="mt-4 max-w-[64ch] text-[15px] leading-relaxed text-mute">{t.tasksBody}</p>
        </Reveal>
        <div className="mt-12 grid items-start gap-10 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.05fr)] lg:gap-14">
          <ul className="grid content-start gap-x-8 gap-y-9 sm:grid-cols-2">
            {t.tasksFacts.map((fact, index) => (
              <li key={fact.key}>
                <Reveal delay={index * 0.06}>
                  <p className="font-mono text-[12px] text-amber">{fact.key}</p>
                  <h3 className="mt-2 text-[17px] font-semibold tracking-[-0.02em]">{fact.title}</h3>
                  <p className="mt-2 max-w-[42ch] text-[14px] leading-relaxed text-mute">
                    {fact.body}
                  </p>
                </Reveal>
              </li>
            ))}
          </ul>
          <Reveal delay={0.1}>
            <TiltCard>
              <figure className="hero-shot overflow-hidden rounded-[16px] border border-line bg-panel">
                <img
                  src={tasksShotUrl}
                  alt={t.tasksImageAlt}
                  width={1600}
                  height={1000}
                  loading="lazy"
                  className="block aspect-[16/10] w-full object-cover object-top"
                />
              </figure>
            </TiltCard>
          </Reveal>
        </div>
      </div>
    </section>
  );
}

function Library({ t }: { t: Copy }) {
  const [lead, ...rest] = t.libraryCells;
  return (
    <section id="library" className="border-t border-line">
      <div className="mx-auto w-full max-w-[1400px] px-5 py-20 md:px-8 md:py-28">
        <Reveal>
          <h2 className="max-w-[26ch] text-[clamp(1.9rem,3.6vw,3.1rem)] font-semibold leading-[1.14] tracking-[-0.03em]">
            {t.libraryTitle}
          </h2>
          <p className="mt-4 max-w-[58ch] text-[15px] leading-relaxed text-mute">
            {t.libraryBody}
          </p>
        </Reveal>
        <div className="mt-12 grid gap-4 md:grid-cols-3">
          <Reveal className="md:col-span-2">
            <article className="relative h-full overflow-hidden rounded-[16px] border border-line bg-raised p-7 md:p-9">
              <TerminalWindow
                aria-hidden
                weight="thin"
                className="pointer-events-none absolute -right-5 -top-5 size-36 text-line"
              />
              <h3 className="text-[19px] font-semibold tracking-[-0.02em]">{lead.title}</h3>
              <p className="mt-3 max-w-[46ch] text-[14px] leading-relaxed text-mute">{lead.body}</p>
              <p className="mt-6 inline-block rounded-[10px] border border-line bg-panel px-4 py-3 font-mono text-[12.5px] text-amber">
                {lead.mono}
              </p>
            </article>
          </Reveal>
          {rest.map((cell, index) => {
            const Icon = libraryIcons[index + 1];
            return (
              <Reveal key={cell.title} delay={0.05 + index * 0.05} className="h-full">
                <article
                  className={
                    "h-full rounded-[16px] border p-6 md:p-7 " +
                    (index === rest.length - 1
                      ? "border-amber/30 bg-amber/[0.07]"
                      : "border-line bg-panel")
                  }
                >
                  <Icon className="size-6 text-amber" weight="regular" aria-hidden />
                  <h3 className="mt-4 text-[17px] font-semibold tracking-[-0.02em]">{cell.title}</h3>
                  <p className="mt-2.5 max-w-[44ch] text-[14px] leading-relaxed text-mute">
                    {cell.body}
                  </p>
                </article>
              </Reveal>
            );
          })}
        </div>
      </div>
    </section>
  );
}

function Safety({ t }: { t: Copy }) {
  return (
    <section id="safety" className="border-t border-line">
      <div className="mx-auto grid w-full max-w-[1400px] gap-12 px-5 py-20 md:grid-cols-[minmax(0,0.85fr)_minmax(0,1.15fr)] md:px-8 md:py-28">
        <Reveal>
          <h2 className="max-w-[18ch] text-[clamp(1.9rem,3.6vw,3.1rem)] font-semibold leading-[1.14] tracking-[-0.03em]">
            {t.safetyTitle}
          </h2>
          <p className="mt-4 max-w-[44ch] text-[15px] leading-relaxed text-mute">{t.safetyBody}</p>
        </Reveal>
        <ol className="grid">
          {t.safetyRules.map((rule, index) => (
            <li key={rule} className={index === 0 ? "" : "border-t border-line"}>
              <Reveal delay={index * 0.05}>
                <p className="py-7 text-[clamp(1.05rem,1.6vw,1.3rem)] font-medium leading-snug tracking-[-0.01em]">
                  {rule}
                </p>
              </Reveal>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}

export function LandingPage({ locale }: { locale: Locale }) {
  const t: Copy = copy[locale];
  const homeHref = locale === "en" ? "./" : "../";

  return (
    <div className="min-h-[100dvh] bg-ink text-paper">
      <a className="skip-link" href="#main">
        {t.skip}
      </a>

      <header className="mx-auto flex h-16 w-full max-w-[1400px] items-center justify-between px-5 md:px-8">
        <a
          className="flex items-center gap-2.5 text-[15px] font-semibold tracking-[-0.02em]"
          href={homeHref}
          aria-label={t.homeAria}
        >
          <img src={markUrl} alt="" width={28} height={28} className="rounded-[8px]" />
          <span>RepoAtlas</span>
        </a>
        <nav className="flex items-center gap-5 text-[13px] text-mute" aria-label={t.navAria}>
          {[
            { href: "#tasks", label: t.navTasks },
            { href: "#library", label: t.navLibrary },
            { href: "#safety", label: t.navSafety },
          ].map((link) => (
            <a
              key={link.href}
              className="hidden hover:text-paper md:inline"
              href={link.href}
            >
              {link.label}
            </a>
          ))}
          <a href={t.navLangHref} lang={t.navLangLang}>
            {t.navLang}
          </a>
          <a
            className="inline-flex min-h-9 items-center whitespace-nowrap rounded-[8px] border border-line bg-panel px-3.5 text-paper hover:bg-raised"
            href={SOURCE}
          >
            {t.navSource}
          </a>
        </nav>
      </header>

      <main id="main">
        <Hero t={t} locale={locale} />
        <Marquee t={t} />
        <Tasks t={t} />
        <Library t={t} />
        <Safety t={t} />

        <section className="border-t border-line">
          <div className="mx-auto flex w-full max-w-[1400px] flex-col gap-8 px-5 py-20 md:flex-row md:items-end md:justify-between md:px-8 md:py-28">
            <Reveal>
              <h2 className="max-w-[18ch] text-[clamp(1.9rem,3.6vw,3.1rem)] font-semibold leading-[1.14] tracking-[-0.03em]">
                {t.closeTitle}
              </h2>
              <p className="mt-4 max-w-[50ch] text-[15px] leading-relaxed text-mute">
                {t.closeBody}
              </p>
            </Reveal>
            <Reveal delay={0.08}>
              <div className="flex flex-wrap gap-2.5">
                <Button href={SOURCE} variant="primary">
                  {t.closePrimary}
                </Button>
                <Button href={CONTRIBUTING} variant="secondary">
                  {t.closeSecondary}
                </Button>
              </div>
            </Reveal>
          </div>
        </section>
      </main>

      <footer className="border-t border-line">
        <div className="mx-auto flex w-full max-w-[1400px] flex-col gap-5 px-5 py-8 text-[12px] text-mute md:flex-row md:items-center md:justify-between md:px-8">
          <a className="flex items-center gap-2 text-paper" href={homeHref}>
            <img src={markUrl} alt="" width={24} height={24} className="rounded-[7px]" />
            <span>RepoAtlas</span>
          </a>
          <p>{t.footerBlurb}</p>
          <div className="flex gap-5">
            <a href={SOURCE}>GitHub</a>
            <a href={SECURITY}>{t.footerSecurity}</a>
            <a href={t.navLangHref} lang={t.navLangLang}>
              {t.navLang}
            </a>
          </div>
        </div>
      </footer>
    </div>
  );
}
