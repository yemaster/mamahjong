import { describe, expect, it, vi } from "vitest";
import * as THREE from "three";
import {
  disposeGroup,
  disposeTileGeometries,
  disposeTileMaterials,
} from "./runtime";
import { dimTile, makeTile, markTileAsDora } from "./tileMesh";
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
    tileMaterials: new Map(),
    doraShine: new THREE.ShaderMaterial(),
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

  it("局部图层重建时复用牌身和图案材质", () => {
    const runtime = geometryRuntime();
    const firstBack = makeTile(runtime, "back", 1);
    const secondBack = makeTile(runtime, "back", 1);
    const firstFace = makeTile(runtime, "1m", 1);
    const secondFace = makeTile(runtime, "1m", 1);

    expect(firstBack.userData.faceMaterial).toBe(
      secondBack.userData.faceMaterial,
    );
    expect(firstFace.userData.faceMaterial).toBe(
      secondFace.userData.faceMaterial,
    );
    expect(firstFace.userData.facePlateMaterial).not.toBe(
      secondFace.userData.facePlateMaterial,
    );
    expect(
      [...runtime.tileMaterials.values()].every(
        (material) => material.userData.shared === true,
      ),
    ).toBe(true);

    disposeGroup(firstBack);
    disposeGroup(secondBack);
    disposeGroup(firstFace);
    disposeGroup(secondFace);
    disposeTileGeometries(runtime);
    disposeTileMaterials(runtime);
  });

  it("牌面图案使用稳定的深度偏移和不透明 cutout，不参与全桌透明排序", () => {
    const runtime = geometryRuntime();
    const tile = makeTile(runtime, "1m", 1);
    const artwork = tile.userData.artworkMesh as THREE.Mesh;
    const material = artwork.material as THREE.MeshBasicMaterial;

    expect(material.transparent).toBe(false);
    expect(material.depthWrite).toBe(true);
    expect(material.alphaTest).toBeGreaterThan(0);
    expect(material.polygonOffset).toBe(true);
    expect(artwork.position.y).toBeGreaterThan(
      (tile.userData.tileDepth as number) / 2 + 0.003,
    );

    disposeGroup(tile);
    disposeTileGeometries(runtime);
    disposeTileMaterials(runtime);
  });

  it("宝牌和摸切牌写时复制，不会染到其他复用材质", () => {
    const runtime = geometryRuntime();
    const dora = makeTile(runtime, "1m", 1);
    const normal = makeTile(runtime, "1m", 1);
    const dimmedBack = makeTile(runtime, "back", 1);
    const normalBack = makeTile(runtime, "back", 1);

    markTileAsDora(runtime, dora);
    dimTile(dora, 0.68);
    dimTile(dimmedBack, 0.68);

    expect(dora.userData.faceMaterial).not.toBe(normal.userData.faceMaterial);
    expect(dimmedBack.userData.faceMaterial).not.toBe(
      normalBack.userData.faceMaterial,
    );
    expect(
      (normal.userData.faceMaterial as THREE.MeshBasicMaterial).color.getHex(),
    ).toBe(0xffffff);
    expect(
      (normalBack.userData.faceMaterial as THREE.MeshStandardMaterial).color.getHex(),
    ).toBe(0x689974);
    let shineMaterial: THREE.Material | null = null;
    dora.traverse((object) => {
      if (object instanceof THREE.Mesh && object.material instanceof THREE.ShaderMaterial) {
        shineMaterial = object.material;
      }
    });
    expect(shineMaterial).toBe(runtime.doraShine);

    disposeGroup(dora);
    disposeGroup(normal);
    disposeGroup(dimmedBack);
    disposeGroup(normalBack);
    disposeTileGeometries(runtime);
    disposeTileMaterials(runtime);
    runtime.doraShine.dispose();
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
