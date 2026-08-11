import { describe, expect, it, vi } from "vitest";
import * as THREE from "three";
import { disposeGroup, disposeTileGeometries } from "./runtime";
import { makeTile } from "./tileMesh";
import type { TableRuntime } from "./types";

function geometryList(group: THREE.Group): THREE.BufferGeometry[] {
  const geometries: THREE.BufferGeometry[] = [];
  group.traverse((object) => {
    if (object instanceof THREE.Mesh) geometries.push(object.geometry);
  });
  return geometries;
}

function geometryRuntime(): TableRuntime {
  return {
    tileWidthRatio: 0.72,
    tileScale: 1,
    textures: new Map(),
    tileGeometries: new Map(),
  } as unknown as TableRuntime;
}

describe("三维牌几何缓存", () => {
  it("同尺寸同朝向的牌复用几何体，不随牌面代码重复创建", () => {
    const runtime = geometryRuntime();
    const first = geometryList(makeTile(runtime, "1m", 1));
    const second = geometryList(makeTile(runtime, "9p", 1));

    expect(runtime.tileGeometries.size).toBe(1);
    expect(second).toHaveLength(first.length);
    expect(second.every((geometry, index) => geometry === first[index])).toBe(
      true,
    );
    expect(first.every((geometry) => geometry.userData.sharedTileGeometry)).toBe(
      true,
    );

    disposeTileGeometries(runtime);
    expect(runtime.tileGeometries.size).toBe(0);
  });

  it("牌背和不同尺寸使用各自正确的几何体", () => {
    const runtime = geometryRuntime();
    makeTile(runtime, "1m", 1);
    makeTile(runtime, "back", 1);
    makeTile(runtime, "1m", 0.56);

    expect(runtime.tileGeometries.size).toBe(3);
    disposeTileGeometries(runtime);
  });

  it("场景重建时复用资源只释放一次，共用牌几何留到运行时结束", () => {
    const root = new THREE.Group();
    const geometry = new THREE.BoxGeometry(1, 1, 1);
    const sharedTileGeometry = new THREE.BoxGeometry(1, 1, 1);
    sharedTileGeometry.userData.sharedTileGeometry = true;
    const texture = new THREE.Texture();
    const material = new THREE.MeshBasicMaterial({ map: texture });
    root.add(
      new THREE.Mesh(geometry, material),
      new THREE.Mesh(geometry, material),
      new THREE.Mesh(sharedTileGeometry, material),
    );
    const disposeGeometry = vi.spyOn(geometry, "dispose");
    const disposeSharedGeometry = vi.spyOn(sharedTileGeometry, "dispose");
    const disposeMaterial = vi.spyOn(material, "dispose");
    const disposeTexture = vi.spyOn(texture, "dispose");

    disposeGroup(root);

    expect(disposeGeometry).toHaveBeenCalledTimes(1);
    expect(disposeSharedGeometry).not.toHaveBeenCalled();
    expect(disposeMaterial).toHaveBeenCalledTimes(1);
    expect(disposeTexture).toHaveBeenCalledTimes(1);
    sharedTileGeometry.dispose();
  });
});
