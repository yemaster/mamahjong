-- 内置曲目：从实际音频文件提取的时长，建表后一次性填充。
-- 冲突时更新时长等客观元数据，保留 enabled/is_default 这类人工调整。
INSERT INTO mamahjong_music_tracks
    (id, version, name, scene, audio_path, duration_ms, enabled, is_default)
VALUES
    ('lobby-default',    1, '默认',      'lobby',  '/game/assets/local-game-assets/music/lobby-default.mp3',    75572,  true,  true),
    ('fusheng-touxian',  1, '浮生偷闲',  'lobby',  '/game/assets/local-game-assets/music/fusheng-touxian.mp3',  198740, true,  false),
    ('zhiying-zhuiguang',1, '织影缀光',  'lobby',  '/game/assets/local-game-assets/music/zhiying-zhuiguang.mp3',130795, true,  false),
    ('zhuqu-zhiyu',      1, '竹取之语',  'match',  '/game/assets/local-game-assets/music/zhuqu-zhiyu.mp3',      71889,  true,  true),
    ('chuzhen',          1, '出阵',      'riichi', '/game/assets/local-game-assets/music/chuzhen.mp3',          69721,  true,  false),
    ('guangzhouta',      1, '广州塔',    'riichi', '/game/assets/local-game-assets/music/guangzhouta.mp3',      116532, true,  false)
ON CONFLICT (id) DO UPDATE SET
    duration_ms = EXCLUDED.duration_ms,
    name = EXCLUDED.name,
    scene = EXCLUDED.scene,
    audio_path = EXCLUDED.audio_path;
