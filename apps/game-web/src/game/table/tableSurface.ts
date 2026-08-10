import * as THREE from "three";
import { RoundedBoxGeometry } from "three/addons/geometries/RoundedBoxGeometry.js";
import { TABLE_DEPTH, TABLE_WIDTH } from "./constants";
import type { TableRuntime } from "./types";

/** 桌面本体：一块铺着桌布贴图的圆角板子。 */
export function addTableSurface(runtime: TableRuntime): void {
  const side = new THREE.MeshStandardMaterial({
    color: 0x100a0c,
    roughness: 0.8,
    metalness: 0.02,
  });
  const top = new THREE.MeshStandardMaterial({
    color: 0xffffff,
    map: runtime.tableTexture,
    roughness: 0.92,
    metalness: 0,
  });
  const table = new THREE.Mesh(
    new RoundedBoxGeometry(TABLE_WIDTH, 0.38, TABLE_DEPTH, 4, 0.16),
    [side, side, top, side, side, side],
  );
  table.position.y = -0.12;
  table.receiveShadow = false;
  runtime.root.add(table);
}
