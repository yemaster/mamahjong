import { useEffect } from "react";

// 樱花瓣颜色——从主题色盘里取
const PETAL_COLORS = [
  "#F4A7B9",
  "#FDE4EC",
  "#FFB7CB",
  "#FFCCD5",
  "#F9A8C2",
  "#FFE4EF",
];

const PETAL_COUNT = 8;

function spawnPetals(cx: number, cy: number) {
  for (let i = 0; i < PETAL_COUNT; i++) {
    const el = document.createElement("div");

    const size = 7 + Math.random() * 8;          // 7–15 px
    const delay = i * 18;                         // 轻微错开，不同时爆出
    const duration = 600 + Math.random() * 300;  // 600–900 ms

    // 均匀散开的角度
    const baseAngle = (Math.PI * 2 * i) / PETAL_COUNT;
    const angle = baseAngle + (Math.random() - 0.5) * 0.4;

    // 散开的最终距离
    const dist = 50 + Math.random() * 40;
    const dx = Math.cos(angle) * dist;
    const dy = Math.sin(angle) * dist;

    const rotStart = Math.random() * 360;
    const rotEnd = rotStart + (Math.random() * 180 - 90);

    const color = PETAL_COLORS[Math.floor(Math.random() * PETAL_COLORS.length)];

    Object.assign(el.style, {
      position:      "fixed",
      pointerEvents: "none",
      zIndex:        "99999",
      left:          `${cx - size / 2}px`,
      top:           `${cy - size / 2}px`,
      width:         `${size}px`,
      height:        `${size * 1.45}px`,
      // 花瓣形：上圆下尖的椭圆
      borderRadius:  "50% 50% 50% 50% / 65% 65% 35% 35%",
      background:    color,
      willChange:    "transform, opacity",
    });

    document.body.appendChild(el);

    const anim = el.animate(
      [
        {
          transform: `translate(0px, 0px) rotate(${rotStart}deg)`,
          opacity:   0.9,
          offset:    0,
        },
        {
          transform: `translate(${dx}px, ${dy}px) rotate(${rotEnd}deg)`,
          opacity:   0,
          offset:    1,
        },
      ],
      {
        duration,
        delay,
        easing: "ease-out",
        fill:   "forwards",
      },
    );

    anim.onfinish = () => el.remove();
  }
}

/**
 * 在整个页面上注册点击/触摸樱花散落特效。
 * 使用 pointerdown，鼠标和触屏均只触发一次，无重复。
 */
export function useSakuraClickEffect() {
  useEffect(() => {
    const handle = (e: PointerEvent) => spawnPetals(e.clientX, e.clientY);
    window.addEventListener("pointerdown", handle);
    return () => window.removeEventListener("pointerdown", handle);
  }, []);
}
