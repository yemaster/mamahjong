import { useEffect, useState, type CSSProperties } from "react";

/*
 * 结算的点数变动卡片。冲击麻将的杠点浮层照抄的就是这一套演出，两边共用同一份
 * 节拍和同一套样式类，改一处两边一起变。
 *
 * 每张卡片的出场节拍：增减数字先在分数底下淡入，停一拍让人看清，再浮上去贴到分数
 * 上，分数才开始滚。面板那一颤要和最先开始滚的那一家对上，所以节拍算在外面，父子
 * 两边看的是同一份。
 */
const DELTA_APPEAR_MS = 120;
const CARD_STAGGER_MS = 90;
/** 淡入本身的时长，和 `pointDeltaAppear` 那条动画对齐。 */
const DELTA_APPEAR_ANIM_MS = 420;
/*
 * 数字站住之后停的那一拍。玩家这一刻要看的就是自己这局加了还是减了多少，数字刚
 * 显示完就往上飞，人眼还没读完整段演出就过去了。
 */
const DELTA_HOLD_MS = 620;
const DELTA_LAND_MS = 340;
/** 分数滚动本身的时长，和 `animatePoints` 里的 `duration` 对齐。 */
export const DELTA_COUNT_MS = 600;

export function cardBeats(index: number): {
  appearAt: number;
  riseAt: number;
  countAt: number;
} {
  const appearAt = DELTA_APPEAR_MS + index * CARD_STAGGER_MS;
  const riseAt = appearAt + DELTA_APPEAR_ANIM_MS + DELTA_HOLD_MS;
  return { appearAt, riseAt, countAt: riseAt + DELTA_LAND_MS };
}

export const fallbackAvatar =
  `${import.meta.env.BASE_URL}assets/local-characters/mahjong-soul/ichihime/emotes/8.png`;

export function PointChangeCard({
  avatarPath,
  nickname,
  isSelf,
  before,
  after,
  delta,
  index,
  /** 数字下面那行小字，比如杠点浮层用来写「杠点」。 */
  caption,
}: {
  avatarPath: string | null;
  nickname: string;
  isSelf: boolean;
  before: number;
  after: number;
  delta: number;
  index: number;
  caption?: string;
}) {
  const [displayPoints, setDisplayPoints] = useState(before);
  const [deltaPhase, setDeltaPhase] = useState<"appear" | "rise" | "done">(
    "appear",
  );
  const [pointsBumping, setPointsBumping] = useState(false);

  const { appearAt, riseAt, countAt } = cardBeats(index);

  useEffect(() => {
    if (delta === 0) {
      setDeltaPhase("done");
      setDisplayPoints(after);
      return;
    }

    setDisplayPoints(before);
    setDeltaPhase("appear");

    const riseTimer = window.setTimeout(() => setDeltaPhase("rise"), riseAt);
    const countTimer = window.setTimeout(() => {
      setDeltaPhase("done");
      setPointsBumping(true);
      animatePoints(before, after, setDisplayPoints, () =>
        setPointsBumping(false),
      );
    }, countAt);

    return () => {
      window.clearTimeout(riseTimer);
      window.clearTimeout(countTimer);
    };
  }, [before, after, delta, riseAt, countAt]);

  const tone = delta > 0 ? " is-positive" : delta < 0 ? " is-negative" : "";

  return (
    <div
      className={`match-point-change__card${tone}${isSelf ? " is-self" : ""}`}
      style={{ "--card-index": index } as CSSProperties}
    >
      <div className="match-point-change__portrait">
        <img
          className="match-point-change__avatar"
          src={avatarPath ?? fallbackAvatar}
          alt=""
          onError={(e) => {
            (e.target as HTMLImageElement).src = fallbackAvatar;
          }}
        />
        {isSelf && <span className="match-point-change__wind">自家</span>}
      </div>
      <span className="match-point-change__name">{nickname}</span>
      <div className="match-point-change__meter">
        <strong
          className={`match-point-change__points${
            pointsBumping ? " is-bumping" : ""
          }`}
        >
          {displayPoints.toLocaleString("en-US")}
        </strong>
        {delta !== 0 && (
          <span
            className={`match-point-change__delta${tone} is-phase-${deltaPhase}`}
            style={
              {
                "--delta-appear-delay": `${appearAt}ms`,
              } as CSSProperties
            }
          >
            {delta > 0
              ? `+${delta.toLocaleString("en-US")}`
              : `-${Math.abs(delta).toLocaleString("en-US")}`}
          </span>
        )}
      </div>
      {caption && (
        <span className="match-point-change__caption">{caption}</span>
      )}
    </div>
  );
}

export function animatePoints(
  from: number,
  to: number,
  setValue: (v: number) => void,
  onDone: () => void,
) {
  const startTime = performance.now();
  const animate = (now: number) => {
    const progress = Math.min(1, (now - startTime) / DELTA_COUNT_MS);
    const eased = 1 - Math.pow(1 - progress, 3);
    setValue(Math.round(from + (to - from) * eased));
    if (progress < 1) {
      requestAnimationFrame(animate);
    } else {
      onDone();
    }
  };
  requestAnimationFrame(animate);
}
