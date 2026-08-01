import { useEffect, useState } from "react";

/* ── Floating particles ─────────────────── */

const PARTICLE_COUNT = 20;

function randomBetween(min: number, max: number): number {
  return Math.random() * (max - min) + min;
}

interface Particle {
  id: number;
  left: number;
  delay: number;
  duration: number;
  size: number;
  opacity: number;
}

function generateParticles(): Particle[] {
  return Array.from({ length: PARTICLE_COUNT }, (_, i) => ({
    id: i,
    left: randomBetween(0, 100),
    delay: randomBetween(0, 4),
    duration: randomBetween(8, 16),
    size: randomBetween(3, 8),
    opacity: randomBetween(0.15, 0.4),
  }));
}

/* ── Styles ──────────────────────────────── */

const container: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  zIndex: 9999,
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  background: `
    radial-gradient(ellipse at 50% 40%, rgba(18,82,60,0.6) 0%, #0D2818 70%),
    #0D2818
  `,
  cursor: "pointer",
  userSelect: "none",
  transition: "opacity 0.6s ease-out",
};

const bgImage: React.CSSProperties = {
  position: "absolute",
  inset: 0,
  backgroundImage: "url('/assets/ui/bg_splash.jpg')",
  backgroundSize: "cover",
  backgroundPosition: "center",
  opacity: 0,
  transition: "opacity 1.5s ease-in",
};

const particleStyle = (p: Particle): React.CSSProperties => ({
  position: "absolute",
  left: `${p.left}%`,
  bottom: "-10px",
  width: p.size,
  height: p.size,
  borderRadius: "50%",
  background: "rgba(212,168,83,0.3)",
  opacity: 0,
  animation: `splashFloat ${p.duration}s ${p.delay}s infinite linear`,
});

const logo: React.CSSProperties = {
  position: "relative",
  zIndex: 1,
  textAlign: "center",
};

const logoImg: React.CSSProperties = {
  width: 320,
  height: 120,
  objectFit: "contain",
  marginBottom: 8,
  display: "block",
};

const mainTitle: React.CSSProperties = {
  fontSize: 52,
  fontWeight: 800,
  color: "var(--color-text)",
  letterSpacing: "0.15em",
  textShadow: "0 2px 12px rgba(0,0,0,0.6)",
  lineHeight: 1.2,
};

const subTitle: React.CSSProperties = {
  fontSize: 18,
  fontWeight: 400,
  color: "var(--color-accent)",
  letterSpacing: "0.3em",
  marginTop: 4,
  textShadow: "0 1px 6px rgba(0,0,0,0.5)",
};

const bottom: React.CSSProperties = {
  position: "absolute",
  bottom: 60,
  left: 0,
  right: 0,
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  gap: 16,
  zIndex: 1,
};

const progressTrack: React.CSSProperties = {
  width: 260,
  height: 3,
  background: "rgba(255,255,255,0.1)",
  borderRadius: 2,
  overflow: "hidden",
};

const progressBar = (pct: number): React.CSSProperties => ({
  width: `${pct}%`,
  height: "100%",
  background: "var(--color-accent)",
  borderRadius: 2,
  transition: "width 0.3s ease-out",
});

const loadingText: React.CSSProperties = {
  fontSize: 13,
  color: "var(--color-text-dim)",
  letterSpacing: "0.2em",
};

const tapText: React.CSSProperties = {
  fontSize: 14,
  color: "var(--color-text)",
  letterSpacing: "0.25em",
  opacity: 0,
  transition: "opacity 0.6s",
  animation: "splashPulse 2s ease-in-out infinite",
};

const version: React.CSSProperties = {
  position: "absolute",
  bottom: 20,
  right: 24,
  fontSize: 11,
  color: "rgba(255,255,255,0.2)",
  letterSpacing: "0.1em",
  zIndex: 1,
};

const copyright: React.CSSProperties = {
  position: "absolute",
  bottom: 20,
  left: 24,
  fontSize: 11,
  color: "rgba(255,255,255,0.15)",
  zIndex: 1,
};

/* ── Component ───────────────────────────── */

interface SplashScreenProps {
  onEnter: () => void;
}

export function SplashScreen({ onEnter }: SplashScreenProps) {
  const [particles] = useState(generateParticles);
  const [progress, setProgress] = useState(0);
  const [bgLoaded, setBgLoaded] = useState(false);
  const [showTap, setShowTap] = useState(false);
  const [fading, setFading] = useState(false);

  /* Simulate loading progress. */
  useEffect(() => {
    const steps = [
      { at: 200, to: 30 },
      { at: 500, to: 60 },
      { at: 900, to: 85 },
      { at: 1400, to: 100 },
    ];
    const timers: ReturnType<typeof setTimeout>[] = [];
    for (const step of steps) {
      timers.push(setTimeout(() => setProgress(step.to), step.at));
    }
    timers.push(
      setTimeout(() => {
        setShowTap(true);
      }, 1800),
    );
    return () => timers.forEach(clearTimeout);
  }, []);

  /* Try to load background image. */
  useEffect(() => {
    const img = new Image();
    img.onload = () => setBgLoaded(true);
    img.src = "/assets/ui/bg_splash.jpg";
  }, []);

  const handleClick = () => {
    if (!showTap) return;
    setFading(true);
    setTimeout(onEnter, 600);
  };

  return (
    <div
      style={{ ...container, opacity: fading ? 0 : 1 }}
      onClick={handleClick}
    >
      {/* Background image fade-in */}
      <div
        style={{
          ...bgImage,
          backgroundImage: bgLoaded
            ? "url('/assets/ui/bg_splash.jpg')"
            : "none",
          opacity: bgLoaded ? 0.35 : 0,
        }}
      />

      {/* Floating particles */}
      {particles.map((p) => (
        <div key={p.id} style={particleStyle(p)} />
      ))}

      {/* Logo */}
      <div style={logo}>
        <img
          src="/assets/ui/logo.png"
          alt=""
          style={{
            ...logoImg,
            display: "none" as const,
          }}
          onError={(e) => {
            /* Hide image element on load failure, show text fallback. */
            (e.target as HTMLImageElement).style.display = "none";
          }}
        />
        <div style={mainTitle}>麻麻的将</div>
        <div style={subTitle}>マージャン</div>
      </div>

      {/* Bottom area */}
      <div style={bottom}>
        <div style={progressTrack}>
          <div style={progressBar(progress)} />
        </div>
        <div style={loadingText}>
          {progress < 100 ? `加载中 ${progress}%` : "加载完成"}
        </div>
        <div
          style={{
            ...tapText,
            opacity: showTap ? 1 : 0,
            cursor: showTap ? "pointer" : "default",
          }}
        >
          点击屏幕开始
        </div>
      </div>

      <div style={copyright}>© mamahjong</div>
      <div style={version}>ver 0.1.0</div>
    </div>
  );
}
