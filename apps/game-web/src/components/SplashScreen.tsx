import { useEffect, useState } from "react";

const PARTICLE_COUNT = 30;

interface Particle {
  id: number;
  left: number;
  delay: number;
  duration: number;
  size: number;
  opacity: number;
}

function genParticles(): Particle[] {
  return Array.from({ length: PARTICLE_COUNT }, (_, i) => ({
    id: i,
    left: Math.random() * 100,
    delay: Math.random() * 3,
    duration: 6 + Math.random() * 10,
    size: 3 + Math.random() * 7,
    opacity: 0.15 + Math.random() * 0.35,
  }));
}

/* ══════ Styles ══════ */

const wrapper: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  zIndex: 9999,
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  background: `
    radial-gradient(ellipse at 50% 35%, rgba(20,60,40,0.5) 0%, #060f0c 65%),
    #060f0c
  `,
  cursor: "pointer",
  userSelect: "none",
  transition: "opacity 0.8s ease-out",
};

const bgImg: React.CSSProperties = {
  position: "absolute",
  inset: 0,
  backgroundSize: "cover",
  backgroundPosition: "center",
  opacity: 0,
  transition: "opacity 2s ease-in",
};

const particle = (p: Particle): React.CSSProperties => ({
  position: "absolute",
  left: `${p.left}%`,
  bottom: -10,
  width: p.size,
  height: p.size,
  borderRadius: "50%",
  background: `radial-gradient(circle, rgba(232,197,71,${p.opacity}) 0%, transparent 70%)`,
  animation: `splashFloat ${p.duration}s ${p.delay}s infinite linear`,
  filter: "blur(0.5px)",
});

/* ── Logo ──────────────────────────── */

const logoWrap: React.CSSProperties = {
  position: "relative",
  zIndex: 1,
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
};

const ornamentBar = (color: string): React.CSSProperties => ({
  width: 200,
  height: 1,
  background: `linear-gradient(90deg, transparent, ${color}, transparent)`,
  margin: "12px 0",
});

const ornamentDiamond: React.CSSProperties = {
  width: 6,
  height: 6,
  background: "var(--color-gold-bright)",
  transform: "rotate(45deg)",
  margin: "0 auto",
};

const title: React.CSSProperties = {
  fontSize: 54,
  fontWeight: 900,
  letterSpacing: "0.2em",
  color: "var(--color-gold-bright)",
  textShadow:
    "0 0 20px rgba(200,160,50,0.4), 0 2px 4px rgba(0,0,0,0.8)",
};

const sub: React.CSSProperties = {
  fontSize: 16,
  fontWeight: 400,
  letterSpacing: "0.45em",
  color: "var(--color-gold-dim)",
  textShadow: "0 1px 4px rgba(0,0,0,0.5)",
};

/* ── Bottom ────────────────────────── */

const bottomArea: React.CSSProperties = {
  position: "absolute",
  bottom: 80,
  left: 0,
  right: 0,
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  gap: 18,
  zIndex: 1,
};

const progressTrack = (): React.CSSProperties => ({
  width: 280,
  height: 2,
  background: "rgba(255,255,255,0.06)",
  position: "relative",
});

const progressFill = (pct: number): React.CSSProperties => ({
  width: `${pct}%`,
  height: "100%",
  background: `linear-gradient(90deg, var(--color-gold-dim), var(--color-gold-bright))`,
  boxShadow: "0 0 6px var(--color-gold-dim)",
  transition: "width 0.4s ease-out",
});

const loadLabel: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 600,
  letterSpacing: "0.25em",
  color: "var(--color-text-dim)",
};

const tapPrompt: React.CSSProperties = {
  fontSize: 14,
  fontWeight: 600,
  letterSpacing: "0.3em",
  color: "var(--color-gold-bright)",
  textShadow: "0 0 10px var(--color-gold-dim)",
  opacity: 0,
  transition: "opacity 0.8s",
  animation: "splashPulse 2s ease-in-out infinite",
};

/* ── Corners ───────────────────────── */

const cornerText: React.CSSProperties = {
  position: "absolute",
  bottom: 22,
  fontSize: 10,
  fontWeight: 500,
  letterSpacing: "0.12em",
  color: "rgba(255,255,255,0.12)",
  zIndex: 1,
};

/* ══════ Component ══════ */

interface SplashScreenProps {
  onEnter: () => void;
}

export function SplashScreen({ onEnter }: SplashScreenProps) {
  const [particles] = useState(genParticles);
  const [progress, setProgress] = useState(0);
  const [bgLoaded, setBgLoaded] = useState(false);
  const [showTap, setShowTap] = useState(false);
  const [fading, setFading] = useState(false);

  /* Simulated loading. */
  useEffect(() => {
    const steps = [
      { at: 200, to: 25 },
      { at: 500, to: 55 },
      { at: 900, to: 85 },
      { at: 1400, to: 100 },
    ];
    const timers: ReturnType<typeof setTimeout>[] = [];
    for (const s of steps) timers.push(setTimeout(() => setProgress(s.to), s.at));
    timers.push(setTimeout(() => setShowTap(true), 1800));
    return () => timers.forEach(clearTimeout);
  }, []);

  /* Optional background image. */
  useEffect(() => {
    const img = new Image();
    img.onload = () => setBgLoaded(true);
    img.src = "/assets/ui/bg_splash.jpg";
  }, []);

  const handleClick = () => {
    if (!showTap) return;
    setFading(true);
    setTimeout(onEnter, 800);
  };

  return (
    <div
      style={{ ...wrapper, opacity: fading ? 0 : 1 }}
      onClick={handleClick}
    >
      {/* Background image */}
      <div
        style={{
          ...bgImg,
          backgroundImage: bgLoaded
            ? "url('/assets/ui/bg_splash.jpg')"
            : "none",
          opacity: bgLoaded ? 0.3 : 0,
        }}
      />

      {/* Particles */}
      {particles.map((p) => (
        <div key={p.id} style={particle(p)} />
      ))}

      {/* Logo region */}
      <div style={logoWrap}>
        <div style={ornamentBar("var(--color-gold-dim)")} />
        <div style={ornamentDiamond} />
        <div style={{ height: 12 }} />
        <div style={title}>麻麻的将</div>
        <div style={sub}>ＭＡＨＪＯＮＧ</div>
        <div style={{ height: 12 }} />
        <div style={ornamentDiamond} />
        <div style={ornamentBar("var(--color-gold-dim)")} />
      </div>

      {/* Bottom */}
      <div style={bottomArea}>
        <div style={progressTrack()}>
          <div style={progressFill(progress)} />
        </div>
        <div style={loadLabel}>
          {progress < 100 ? `LOADING ${progress}%` : "READY"}
        </div>
        <div
          style={{
            ...tapPrompt,
            opacity: showTap ? 1 : 0,
            cursor: showTap ? "pointer" : "default",
          }}
        >
          TOUCH TO START
        </div>
      </div>

      <div style={{ ...cornerText, left: 24 }}>© mamahjong</div>
      <div style={{ ...cornerText, right: 24 }}>ver 0.1.0</div>
    </div>
  );
}
