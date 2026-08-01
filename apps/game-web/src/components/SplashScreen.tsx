import { useEffect, useMemo, useState } from "react";

/* ═══════════════════════════════════════════════════════════════
   SPLASH — warm anime school style with falling sakura petals
   ═══════════════════════════════════════════════════════════════ */

interface Petal {
  id: number;
  left: string;
  delay: string;
  duration: string;
  size: string;
}

function petals(count: number): Petal[] {
  return Array.from({ length: count }, (_, i) => ({
    id: i,
    left: `${Math.random() * 100}%`,
    delay: `${Math.random() * 6}s`,
    duration: `${5 + Math.random() * 8}s`,
    size: `${8 + Math.random() * 10}px`,
  }));
}

const wrapper = (clickable: boolean): React.CSSProperties => ({
  position: "fixed",
  inset: 0,
  zIndex: 9999,
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  background: "#FFF8F0",
  backgroundImage: "url('/assets/ui/bg_splash.jpg')",
  backgroundSize: "cover",
  backgroundPosition: "center",
  backgroundRepeat: "no-repeat",
  cursor: clickable ? "pointer" : "default",
  transition: "opacity 0.6s ease-out",
});

const title: React.CSSProperties = {
  fontSize: 46,
  fontWeight: 900,
  letterSpacing: "0.25em",
  color: "#D4899E",
};

const sub: React.CSSProperties = {
  fontSize: 14,
  fontWeight: 600,
  letterSpacing: "0.4em",
  color: "#C9A96E",
  marginTop: 8,
};

const bottom: React.CSSProperties = {
  position: "absolute",
  bottom: 72,
  left: 0,
  right: 0,
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  gap: 14,
};

const track: React.CSSProperties = {
  width: 220,
  height: 2,
  background: "rgba(74,55,40,0.1)",
  borderRadius: 1,
};

const fill = (pct: number): React.CSSProperties => ({
  width: `${pct}%`,
  height: "100%",
  background: "#D4899E",
  borderRadius: 1,
  transition: "width 0.3s ease-out",
});

const loadText: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 600,
  letterSpacing: "0.25em",
  color: "rgba(74,55,40,0.35)",
};

const tap: React.CSSProperties = {
  fontSize: 14,
  fontWeight: 700,
  letterSpacing: "0.3em",
  color: "rgba(74,55,40,0.55)",
  opacity: 0,
  transition: "opacity 0.8s",
};

const corner: React.CSSProperties = {
  position: "absolute",
  bottom: 18,
  fontSize: 10,
  letterSpacing: "0.1em",
  color: "rgba(74,55,40,0.15)",
};

/* ═══════════════════════════════════════════════════════════════ */

interface Props {
  onEnter: () => void;
}

export function SplashScreen({ onEnter }: Props) {
  const [progress, setProgress] = useState(0);
  const [showTap, setShowTap] = useState(false);
  const [fading, setFading] = useState(false);
  const petalList = useMemo(() => petals(35), []);

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

  const handleClick = () => {
    if (!showTap) return;
    setFading(true);
    setTimeout(onEnter, 600);
  };

  return (
    <div style={wrapper(showTap)} onClick={handleClick}>
      {/* Sakura petals */}
      {petalList.map((p) => (
        <div
          key={p.id}
          className="sakura-petal"
          style={{
            left: p.left,
            animationDelay: p.delay,
            animationDuration: p.duration,
            width: p.size,
            height: p.size,
          }}
        />
      ))}

      {/* Logo */}
      <div style={title}>麻麻的将</div>
      <div style={sub}>MAHJONG</div>

      {/* Bottom bar */}
      <div style={bottom}>
        <div style={track}>
          <div style={fill(progress)} />
        </div>
        <div style={loadText}>
          {progress < 100 ? `LOADING ${progress}%` : "READY"}
        </div>
        <div style={{ ...tap, opacity: showTap ? 1 : 0 }}>
          TOUCH TO START
        </div>
      </div>

      <div style={{ ...corner, left: 20 }}>© mamahjong</div>
      <div style={{ ...corner, right: 20 }}>ver 0.1.0</div>
    </div>
  );
}
