export interface ProjectAboutSection {
  title: string;
  paragraphs: readonly string[];
}

export interface ProjectChangelogEntry {
  version: string;
  date: string;
  changes: readonly string[];
}

/**
 * “帮助 → 关于本项目”的内容模板。
 *
 * 只需替换下方占位文字；页面布局和导航不需要随内容一起修改。
 */
export const projectAboutTemplate = {
  projectName: "MaMahjong",
  sections: [
    {
      title: "麻麻的将",
      paragraphs: [
        "自部署网页端的多人联机麻将游戏服务，支持日本麻将（四麻/三麻）与冲击麻将。采用领域驱动设计，规则引擎与传输层解耦，可扩展至其他麻将玩法。",
        "Made by yemaster with ❤️",
      ],
    },
  ] satisfies readonly ProjectAboutSection[],
  changelog: [
    {
      version: "v0.2.6",
      date: "2026-08-14",
      changes: ["修复了音量调整失效的问题", "优化了部分 UI", "修复了部分 bug"],
    },
    {
      version: "v0.2.5",
      date: "2026-08-13",
      changes: ["修复游戏内对局聊天无法接受的问题", "修复了牌局多次渲染刷新的问题", "优化了部分 UI", "修复了部分 bug"],
    },
    {
      version: "v0.2.4",
      date: "2026-08-12",
      changes: ["修复杠牌动画的问题", "优化了部分 UI", "修复了部分 bug"],
    },
  ] satisfies readonly ProjectChangelogEntry[],
} as const;
