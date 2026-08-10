import type { AuditEvent } from "./types";

const dateTimeFormatter = new Intl.DateTimeFormat("zh-CN", {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
});

const categoryLabels: Record<string, string> = {
  auth: "认证",
  room: "房间",
  matchmaking: "匹配",
  game: "对局",
  admin: "管理",
};

const actionLabels: Record<string, string> = {
  "user.registration.succeeded": "用户注册",
  "user.login.succeeded": "用户登录",
  "user.login.failed": "登录失败",
  "room.created": "创建房间",
  "room.joined": "加入房间",
  "room.left": "离开房间",
  "room.started": "房间开局",
  "matchmaking.waiting": "进入匹配",
  "matchmaking.matched": "匹配成功",
  "matchmaking.cancelled": "取消匹配",
  "game.finished": "对局结束",
  "admin.auth.csrf_rejected": "安全校验失败",
  "admin.auth.login_failed": "管理登录失败",
  "admin.auth.login_succeeded": "管理登录",
  "admin.auth.logout": "管理退出",
  "admin.user.activated": "恢复账号",
  "admin.user.suspended": "停用账号",
  "admin.room.closed": "关闭房间",
  "admin.audit.viewed": "查看审计日志",
};

export function formatDateTime(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : dateTimeFormatter.format(date);
}

export function categoryLabel(category: string): string {
  return categoryLabels[category] ?? category;
}

export function actionLabel(action: AuditEvent["action"]): string {
  return actionLabels[action] ?? action;
}
