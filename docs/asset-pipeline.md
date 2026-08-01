# 素材管线

状态：设计中
最后更新：2026-08-01

## 原则

- 逻辑代码只引用资源 ID，不硬编码文件路径。
- 素材不入 git（`.gitignore` 排除），占位素材可提交。
- 雀魂等第三方素材仅内部测试用，正式发布前替换。
- 无素材时游戏须仍可正常对局（占位降级）。

## 目录结构

```
apps/game-web/public/assets/
├── tiles/
│   ├── tiles.png           # 牌面 spritesheet (34+ 张牌)
│   ├── tiles.json          # 纹理图集坐标
│   └── tile_back.png       # 牌背
├── characters/
│   └── {character_id}/
│       ├── portrait.png    # 立绘 (≥1024×1024)
│       ├── icon.png        # 头像 (256×256)
│       └── expressions/    # 差分表情
│           ├── happy.png
│           ├── sad.png
│           └── surprised.png
├── table/
│   └── felt.png            # 桌布纹理 (1920×1080)
├── effects/
│   ├── riichi.png          # 立直宣言
│   ├── win.png             # 和牌
│   ├── tsumo.png           # 自摸
│   └── dora.png            # 宝牌闪光
├── sounds/
│   ├── tile_draw.mp3       # 摸牌
│   ├── tile_discard.mp3    # 打牌
│   ├── riichi.mp3          # 立直宣言
│   ├── tsumo.mp3           # 自摸
│   ├── ron.mp3             # 荣和
│   ├── pon.mp3             # 碰
│   ├── chi.mp3             # 吃
│   ├── kan.mp3             # 杠
│   └── win_jingle.mp3      # 和牌短曲
└── placeholder/
    ├── char_portrait.png   # 通用角色剪影 (200×400)
    ├── char_icon.png       # 通用头像 (256×256)
    └── table_bg.png        # 纯色桌布 (1920×1080, #12523C)
```

## .gitignore

```gitignore
# 真实素材不得入库（版权、体积）
public/assets/tiles/*
public/assets/characters/*
public/assets/table/*
public/assets/effects/*
public/assets/sounds/*

# 仅保留占位素材
!public/assets/placeholder/
!public/assets/.gitkeep
```

## 牌面纹理图集

### 格式

标准 TexturePacker JSON Hash 格式：

```json
{
  "frames": {
    "1m":  { "frame": {"x":0,  "y":0, "w":64, "h":88} },
    "1mr": { "frame": {"x":64, "y":0, "w":64, "h":88} },
    "2m":  { "frame": {"x":128,"y":0, "w":64, "h":88} },
    "1p":  { "frame": {"x":0,  "y":88,"w":64, "h":88} },
    "1pr": { "frame": {"x":64, "y":88,"w":64, "h":88} },
    "1s":  { "frame": {"x":0,  "y":176,"w":64,"h":88} },
    "1sr": { "frame": {"x":64, "y":176,"w":64,"h":88} },
    "1z":  { "frame": {"x":0,  "y":264,"w":64,"h":88} },
    "2z":  { "frame": {"x":64, "y":264,"w":64,"h":88} },
    "3z":  { "frame": {"x":128,"y":264,"w":64,"h":88} },
    "4z":  { "frame": {"x":192,"y":264,"w":64,"h":88} },
    "5z":  { "frame": {"x":256,"y":264,"w":64,"h":88} },
    "6z":  { "frame": {"x":320,"y":264,"w":64,"h":88} },
    "7z":  { "frame": {"x":384,"y":264,"w":64,"h":88} }
  },
  "meta": {
    "image": "tiles.png",
    "size": {"w": 448, "h": 352},
    "scale": "1"
  }
}
```

帧名编码：
- `{number}{suit}`：数牌（`1m`-`9m`, `1p`-`9p`, `1s`-`9s`）
- `{number}{suit}r`：赤牌（`5mr`, `5pr`, `5sr`，三麻无 `5mr`）
- `1z`-`7z`：字牌（東南西北白發中）
- 牌背统一为 `back`

### TileFactory 加载逻辑

```typescript
class TileFactory {
  static async create(): Promise<TileFactory> {
    try {
      // 尝试加载真实素材
      const texture = await Assets.load("/assets/tiles/tiles.json");
      return new TileFactory(texture);
    } catch {
      // 回退占位渲染
      console.info("tile textures not found, using placeholders");
      return new TileFactory(null);
    }
  }

  createSprite(code: string): Container {
    if (this.atlas) {
      return new Sprite(this.atlas.textures[code]);
    }
    return this.createPlaceholder(code);
  }
}
```

## 角色素材

### 命名约定

角色 ID 与服务端 `CharacterSummary.id` 一致。素材路径：
```
characters/{id}/portrait.png
characters/{id}/icon.png
characters/{id}/expressions/{type}.png
```

### 占位

`placeholder/char_portrait.png`：200×400 纯色剪影，深灰底色 + 浅灰人物轮廓。
`placeholder/char_icon.png`：256×256，同上。

当角色 ID 对应的目录不存在时，`CharacterDisplay` 组件回退到占位图。

### 未来扩展

- 语音：`characters/{id}/voices/{event}.mp3`
  - 事件名：`discard`, `riichi`, `tsumo`, `ron`, `pon`, `chi`, `kan`, `win`, `lose`, `greeting`
  - 每个事件可有多个变体（`discard_1.mp3`, `discard_2.mp3`）
- Live2D：`characters/{id}/model.model3.json`（Cubism SDK，不在本阶段范围）
- 皮肤：`characters/{id}/skins/{skin_id}/`

## 特效素材

### 当前范围

四个占位特效：
- `riichi`：立直宣言时的光柱/粒子效果
- `win`：和牌时的闪光
- `tsumo`：自摸特效
- `dora`：宝牌指示牌闪烁

特效用 PixiJS `AnimatedSprite` 播放，帧序列用 spritesheet。占位时用单色矩形 +
透明度动画代替。

## 音效管线

### 加载接口

```typescript
interface SoundManager {
  preload(category: "sfx" | "voice" | "bgm"): Promise<void>;
  play(soundId: string, volume?: number): void;
  setMasterVolume(volume: number): void;
  setCategoryVolume(category: string, volume: number): void;
}
```

### 占位行为

音效目录为空时 `preload` 直接 resolve，`play` 为 no-op。
游戏在无任何音效时静默运行，不报错。

### 设置存储

音量设置存储到 `localStorage`：
```json
{
  "mamahjong_volume": {
    "master": 0.8,
    "sfx": 1.0,
    "voice": 0.7,
    "bgm": 0.5
  }
}
```

## 素材版本管理

- 纹理图集 JSON 版本字段预留 `"atlas_version": 1`
- 未来素材更新时增加版本号，客户端检测版本差异后提示清除缓存
- 不在本阶段实现自动更新下载
