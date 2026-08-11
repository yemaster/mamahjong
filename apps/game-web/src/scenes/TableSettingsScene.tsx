import { useCallback, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { LogOut } from "lucide-react";
import { gameApi } from "../api";
import { useSceneReady } from "../components/SceneTransition";
import {
  DEFAULT_TABLECLOTH_ASSET,
  GameTable,
  type GameTableHandle,
} from "../game/table";
import { MatchHud } from "../game/MatchHud";
import { MatchStage } from "../game/MatchStage";
import { PlayerHand2D } from "../game/PlayerHand2D";
import {
  loadTablePerspectiveSettings,
  saveTablePerspectiveSettings,
  tableCameraConfigFromSettings,
  type TablePerspectiveSettings,
} from "../game/tableDisplaySettings";
import { tablePreviewView } from "../game/tablePreviewData";
import { navigateTo } from "../routing";
import { useAuthStore } from "../stores/authStore";
import { CameraParameter, round } from "./table/CameraParameter";

/** 预览牌桌上的骰子点数，固定一组就行，这里不掷骰。 */
const PREVIEW_DICE: [number, number] = [3, 5];

const noop = () => {};

/**
 * 个人牌桌设置。
 *
 * 页面上只有两样东西：一张和正式对局一模一样的牌桌，和右侧那栏镜头设置。玩家
 * 来这儿是调镜头的，多一个按钮就多一样和调镜头无关的东西。
 *
 * 镜头只有透视一种，没有正交可选：正式对局用的就是透视，设置页给出另一种模式
 * 只会调出一份对局里用不上的参数。
 *
 * 牌桌用的是正式的 `GameTable`、`PlayerHand2D` 和 `MatchHud`，三维牌的缩放和
 * 宽长比一律不传、走正式对局的默认值——这里看到的画面必须就是之后对局里的画面，
 * 否则调出来的角度是白调。
 */
export default function TableSettingsScene() {
  const userId = useAuthStore((state) => state.identity?.id);
  const selectedTableclothId = useAuthStore(
    (state) => state.identity?.profile.selected_tablecloth_id,
  );
  const tablecloths = useQuery({
    queryKey: ["tablecloths"],
    queryFn: gameApi.tablecloths,
  });
  const tableclothPath =
    tablecloths.data?.tablecloths.find(
      (tablecloth) => tablecloth.id === selectedTableclothId,
    )?.texture_path ??
    tablecloths.data?.tablecloths.find((tablecloth) => tablecloth.is_default)
      ?.texture_path ??
    DEFAULT_TABLECLOTH_ASSET;

  const [settings, setSettings] = useState<TablePerspectiveSettings>(() =>
    loadTablePerspectiveSettings(userId),
  );
  const [collapsed, setCollapsed] = useState(false);
  const gameTableRef = useRef<GameTableHandle>(null);
  const focusTableTile = useCallback((code: string | null) => {
    gameTableRef.current?.setFocusedTileCode(code);
  }, []);
  useSceneReady(true);

  const cameraConfig = useMemo(
    () => tableCameraConfigFromSettings(settings),
    [settings],
  );

  const update = <Key extends keyof TablePerspectiveSettings>(
    key: Key,
    value: TablePerspectiveSettings[Key],
  ) => {
    setSettings((current) => ({ ...current, [key]: value }));
  };

  const leave = () => navigateTo({ kind: "profile", tab: "interface" });

  return (
    <div className="match-screen match-preview">
      <GameTable
        ref={gameTableRef}
        view={tablePreviewView}
        openingPhase="play"
        dice={PREVIEW_DICE}
        onTileDiscard={noop}
        cameraConfig={cameraConfig}
        tableclothPath={tableclothPath}
      />
      {/* 退出按钮沿用对局右上角那颗，位置和样式都一样：这一页盖着整块屏幕，
          没有它就只能靠右侧面板底下的「取消」出去。 */}
      <div className="match-utility" aria-label="牌桌设置功能">
        <button
          type="button"
          onClick={leave}
          aria-label="退出牌桌设置"
          title="退出牌桌设置"
        >
          <LogOut aria-hidden="true" />
        </button>
      </div>
      <MatchStage>
        <PlayerHand2D
          view={tablePreviewView}
          openingPhase="play"
          onTileDiscard={noop}
          riichiSelecting={false}
          onFocusedTileChange={focusTableTile}
        />
        <MatchHud view={tablePreviewView} />
      </MatchStage>

      <aside
        className={`match-preview__controls${collapsed ? " is-collapsed" : ""}`}
      >
        <button
          type="button"
          className="match-preview__collapse"
          onClick={() => setCollapsed((value) => !value)}
        >
          {collapsed ? "参数" : "收起"}
        </button>
        {!collapsed && (
          <div className="match-preview__panel">
            <h1>牌桌设置</h1>
            <CameraParameter
              label="摄像机高度"
              value={settings.height}
              min={10}
              max={100}
              step={1}
              onChange={(value) => update("height", value)}
            />
            <CameraParameter
              label="桌面夹角"
              value={settings.angle}
              min={15}
              max={75}
              step={0.1}
              suffix="°"
              onChange={(value) => update("angle", value)}
            />
            <CameraParameter
              label="视野角"
              value={settings.fov}
              min={2}
              max={20}
              step={0.1}
              suffix="°"
              onChange={(value) => update("fov", value)}
            />
            <CameraParameter
              label="注视点高度"
              value={settings.targetY}
              min={-2}
              max={3}
              step={0.05}
              onChange={(value) => update("targetY", value)}
            />
            <CameraParameter
              label="注视点前后"
              value={settings.targetZ}
              min={-5}
              max={5}
              step={0.1}
              onChange={(value) => update("targetZ", value)}
            />
            <div className="match-preview__derived">
              <span>摄像机前后位置</span>
              <strong>{round(cameraConfig.z)}</strong>
            </div>
            <div className="match-preview__buttons">
              <button type="button" onClick={leave}>
                取消
              </button>
              <button
                type="button"
                onClick={() => {
                  if (userId) {
                    saveTablePerspectiveSettings(userId, settings);
                  }
                  leave();
                }}
              >
                保存
              </button>
            </div>
          </div>
        )}
      </aside>
    </div>
  );
}
