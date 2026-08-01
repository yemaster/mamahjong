import { useEffect, useState } from "react";

/* ═══════════════════════════════════════════════════════════════
   SPLASH SCREEN
   Pattern: full-screen artwork + minimal overlay.
   The background illustration IS the experience.
   ═══════════════════════════════════════════════════════════════ */

const WRAPPER: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  zIndex: 9999,
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  background: "#060f0c",
  cursor: "pointer",
  userSelect: "none",
  transition: "opacity 0.6s ease-out",
};

const BG_IMAGE: React.CSSProperties = {
  position: "absolute",
  inset: 0,
  backgroundSize: "cover",
  backgroundPosition: "center",
  backgroundRepeat: "no-repeat",
};

const LOGO_TEXT: React.CSSProperties = {
  position: "relative",
  zIndex: 1,
  fontSize: 40,
  fontWeight: 800,
  letterSpacing: "0.2em",
  color: "#c9a034",
};

const SUB_TEXT: React.CSSProperties = {
  position: "relative",
  zIndex: 1,
  fontSize: 13,
  fontWeight: 500,
  letterSpacing: "0.35em",
  color: "#8a6d28",
  marginTop: 8,
};

const BOTTOM: React.CSSProperties = {
  position: "absolute",
  bottom: 64,
  left: 0,
  right: 0,
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  gap: 14,
  zIndex: 1,
};

const TRACK: React.CSSProperties = {
  width: 240,
  height: 2,
  background: "rgba(255,255,255,0.08)",
};

const FILL = (pct: number): React.CSSProperties => ({
  width: `${pct}%`,
  height: "100%",
  background: "#c9a034",
  transition: "width 0.3s ease-out",
});

const LOADING_TEXT: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 600,
  letterSpacing: "0.2em",
  color: "rgba(255,255,255,0.35)",
};

const TAP_TEXT: React.CSSProperties = {
  fontSize: 13,
  fontWeight: 600,
  letterSpacing: "0.25em",
  color: "rgba(255,255,255,0.5)",
  opacity: 0,
  transition: "opacity 0.8s",
};

const CORNER: React.CSSProperties = {
  position: "absolute",
  bottom: 18,
  fontSize: 10,
  letterSpacing: "0.1em",
  color: "rgba(255,255,255,0.1)",
  zIndex: 1,
};

/* ═══════════════════════════════════════════════════════════════ */

interface Props {
  onEnter: () => void;
}

export function SplashScreen({ onEnter }: Props) {
  const [progress, setProgress] = useState(0);
  const [showTap, setShowTap] = useState(false);
  const [fading, setFading] = useState(false);
  const [bgSrc, setBgSrc] = useState<string | null>(null);

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

  useEffect(() => {
    const img = new Image();
    img.onload = () => setBgSrc("/assets/ui/bg_splash.jpg");
    img.src = "/assets/ui/bg_splash.jpg";
  }, []);

  const handleClick = () => {
    if (!showTap) return;
    setFading(true);
    setTimeout(onEnter, 600);
  };

  return (
    <div
      style={{ ...WRAPPER, opacity: fading ? 0 : 1 }}
      onClick={handleClick}
    >
      {bgSrc && (
        <div style={{ ...BG_IMAGE, backgroundImage: `url(${bgSrc})` }} />
      )}

      <div style={LOGO_TEXT}>麻麻的将</div>
      <div style={SUB_TEXT}>MAHJONG</div>

      <div style={BOTTOM}>
        <div style={TRACK}>
          <div style={FILL(progress)} />
        </div>
        <div style={LOADING_TEXT}>
          {progress < 100 ? `LOADING ${progress}%` : "READY"}
        </div>
        <div style={{ ...TAP_TEXT, opacity: showTap ? 1 : 0 }}>
          TOUCH TO START
        </div>
      </div>

      <div style={{ ...CORNER, left: 20 }}>© mamahjong</div>
      <div style={{ ...CORNER, right: 20 }}>ver 0.1.0</div>
    </div>
  );
}
