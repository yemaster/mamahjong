import * as THREE from "three";
import { RoundedBoxGeometry } from "three/addons/geometries/RoundedBoxGeometry.js";
import type { MatchView } from "../../types";
import { tableRelativeSeat } from "./geometry";
import type { TableRuntime } from "./types";

const CONSOLE_TEXTURE_SIZE = 512;
const CONSOLE_DESIGN_SIZE = 1024;

/** 桌心那块显示场风、余牌和各家点数的面板。 */
export function addCenterConsole(runtime: TableRuntime, view: MatchView): void {
  const texture = centerConsoleTexture(
    view,
    runtime.scoreDifferenceVisible,
  );
  const side = new THREE.MeshStandardMaterial({
    color: 0x252a27,
    roughness: 0.62,
    metalness: 0.18,
  });
  const top = new THREE.MeshStandardMaterial({
    map: texture,
    roughness: 0.5,
    metalness: 0.08,
  });
  const consoleMesh = new THREE.Mesh(
    new RoundedBoxGeometry(2.72, 0.25, 2.72, 4, 0.14),
    [side, side, top, side, side, side],
  );
  consoleMesh.position.y = 0.16;
  consoleMesh.castShadow = false;
  consoleMesh.receiveShadow = false;
  runtime.centerConsoleMesh = consoleMesh;
  runtime.root.add(consoleMesh);
}

function centerConsoleTexture(
  view: MatchView,
  showDifferences: boolean,
): THREE.CanvasTexture {
  const canvas = document.createElement("canvas");
  canvas.width = CONSOLE_TEXTURE_SIZE;
  canvas.height = CONSOLE_TEXTURE_SIZE;
  const context = canvas.getContext("2d")!;
  context.scale(
    CONSOLE_TEXTURE_SIZE / CONSOLE_DESIGN_SIZE,
    CONSOLE_TEXTURE_SIZE / CONSOLE_DESIGN_SIZE,
  );
  context.fillStyle = "#24272c";
  context.fillRect(0, 0, CONSOLE_DESIGN_SIZE, CONSOLE_DESIGN_SIZE);

  octagon(context, 34, "#666a70");
  octagon(context, 78, "#34373d");
  octagon(context, 122, "#171c21");

  context.strokeStyle = "#8b8068";
  context.lineWidth = 8;
  context.strokeRect(330, 350, 364, 324);
  context.fillStyle = "#20272c";
  context.fillRect(344, 364, 336, 296);

  // 冲击麻将没有场风，局数也不按东一南四数，桌心那行改写连庄次数。
  const impact = view.variant_kind === "impact";
  context.textAlign = "center";
  context.textBaseline = "middle";
  context.fillStyle = "#54c8c8";
  context.font = impact ? "900 76px serif" : "900 112px serif";
  context.fillText(
    impact
      ? `连庄${view.dealer_streak ?? 0}次`
      : `${windName(view.progress.round_wind)}${view.progress.round_number}局`,
    512,
    466,
  );
  context.font = "900 74px sans-serif";
  context.fillText(`余牌 ${view.remaining_live_draws}`, 512, 582);
  const observerPoints =
    view.players.find((player) => player.seat === view.observer_seat)?.points ??
    0;

  view.players.forEach((player) => {
    const relative = tableRelativeSeat(
      player.seat,
      view.observer_seat,
      view.players.length,
    );
    // 冲击麻将没有自风，只有庄闲之分，画风字反而是假信息。
    const wind = impact
      ? player.seat === view.progress.dealer
        ? "庄"
        : "闲"
      : seatWind(view, player.seat);
    const positions = [
      [512, 880, 0],
      [880, 512, -Math.PI / 2],
      [512, 144, Math.PI],
      [144, 512, Math.PI / 2],
    ] as const;
    const [x, y, rotation] = positions[relative] ?? positions[0];
    context.save();
    context.translate(x, y);
    context.rotate(rotation);
    context.fillStyle =
      player.seat === view.progress.dealer ? "#a93f48" : "#20252a";
    context.strokeStyle =
      player.seat === view.progress.dealer ? "#e39a91" : "#83868b";
    context.lineWidth = 7;
    context.fillRect(-200, -62, 400, 124);
    context.strokeRect(-200, -62, 400, 124);
    context.fillStyle = "#f1e1bc";
    context.font = "900 64px serif";
    context.fillText(wind, -150, 0);
    context.fillStyle = "#d9b36b";
    context.font = "900 64px sans-serif";
    context.fillText(
      showDifferences
        ? formatPointDifference(player.points - observerPoints)
        : player.points.toLocaleString("zh-CN"),
      55,
      1,
    );
    context.restore();
  });

  view.players.forEach((player) => {
    if (player.riichi_status !== "established") return;
    drawCenterRiichiStick(
      context,
      tableRelativeSeat(
        player.seat,
        view.observer_seat,
        view.players.length,
      ),
    );
  });

  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  return texture;
}

function drawCenterRiichiStick(
  context: CanvasRenderingContext2D,
  relative: number,
): void {
  const positions = [
    [512, 751, 0],
    [771, 512, Math.PI / 2],
    [512, 273, 0],
    [253, 512, Math.PI / 2],
  ] as const;
  const [x, y, rotation] = positions[relative] ?? positions[0];
  context.save();
  context.translate(x, y);
  context.rotate(rotation);
  context.fillStyle = "#eee9dd";
  context.strokeStyle = "#4a4c50";
  context.lineWidth = 5;
  context.beginPath();
  context.roundRect(-182, -17, 364, 34, 17);
  context.fill();
  context.stroke();
  context.beginPath();
  context.fillStyle = "#b33b43";
  context.arc(0, 0, 9, 0, Math.PI * 2);
  context.fill();
  context.restore();
}

function formatPointDifference(value: number): string {
  if (value > 0) return `＋${value.toLocaleString("zh-CN")}`;
  if (value < 0) return `－${Math.abs(value).toLocaleString("zh-CN")}`;
  return "±0";
}

/** 点一下桌心，点数切换成与自家的差分，四秒后自动切回。 */
export function toggleCenterConsolePoints(runtime: TableRuntime): void {
  runtime.scoreDifferenceVisible = !runtime.scoreDifferenceVisible;
  runtime.scoreDifferenceUntil = runtime.scoreDifferenceVisible
    ? performance.now() + 4_000
    : 0;
  updateCenterConsoleTexture(runtime);
}

export function updateCenterConsoleTexture(runtime: TableRuntime): void {
  if (!runtime.centerConsoleMesh || !runtime.latestView) return;
  const materials = runtime.centerConsoleMesh
    .material as THREE.MeshStandardMaterial[];
  const top = materials[2];
  if (!top) return;
  top.map?.dispose();
  top.map = centerConsoleTexture(
    runtime.latestView,
    runtime.scoreDifferenceVisible,
  );
  top.needsUpdate = true;
}

function octagon(
  context: CanvasRenderingContext2D,
  inset: number,
  color: string,
): void {
  const cut = 88;
  const far = 1024 - inset;
  context.beginPath();
  context.moveTo(inset + cut, inset);
  context.lineTo(far - cut, inset);
  context.lineTo(far, inset + cut);
  context.lineTo(far, far - cut);
  context.lineTo(far - cut, far);
  context.lineTo(inset + cut, far);
  context.lineTo(inset, far - cut);
  context.lineTo(inset, inset + cut);
  context.closePath();
  context.fillStyle = color;
  context.fill();
}

function seatWind(view: MatchView, seat: number): string {
  const winds = ["东", "南", "西", "北"];
  return (
    winds[
      (seat + view.players.length - view.progress.dealer) %
        view.players.length
    ] ?? "东"
  );
}

function windName(wind: string): string {
  return (
    {
      east: "东",
      south: "南",
      west: "西",
      north: "北",
    }[wind] ?? "东"
  );
}
