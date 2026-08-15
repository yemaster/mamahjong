export const SCENE_TRANSITION_MIST =
  `${import.meta.env.BASE_URL}assets/ui/scene-transition-mist.png`;

let retainedMistImage: HTMLImageElement | null = null;
let mistLoad: Promise<void> | null = null;

/**
 * 初始/登录页提前完成下载和解码，第一次切场景时只做合成层位移。
 * 保留 Image 引用，避免低内存设备在首次转场前把刚解码的位图立即回收。
 */
export function preloadSceneTransitionMist(): Promise<void> {
  if (mistLoad && retainedMistImage) return mistLoad;

  const image = new Image();
  retainedMistImage = image;
  image.decoding = "async";
  mistLoad = new Promise<void>((resolve, reject) => {
    const finish = () => {
      if (typeof image.decode === "function") {
        void image.decode().catch(() => {}).then(resolve);
      } else {
        resolve();
      }
    };
    image.onload = finish;
    image.onerror = () => reject(new Error("雾气素材加载失败"));
    image.src = SCENE_TRANSITION_MIST;
    if (image.complete && image.naturalWidth > 0) finish();
  });
  void mistLoad.catch(() => {
    mistLoad = null;
    retainedMistImage = null;
  });
  return mistLoad;
}
