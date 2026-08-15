import type { AssetMediaType, ManagedAsset } from "./types";

export function publicAssetUrl(path: string): string {
  return `/user-assets/${path.split("/").map(encodeURIComponent).join("/")}`;
}

export function parentAssetPath(path: string): string {
  return path.split("/").filter(Boolean).slice(0, -1).join("/");
}

export function joinAssetPath(parent: string, name: string): string {
  return [parent, name].filter(Boolean).join("/");
}

export function formatAssetSize(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

export function assetIcon(asset: ManagedAsset): string {
  const icons: Record<AssetMediaType, string> = {
    folder: "pi pi-folder",
    image: "pi pi-image",
    audio: "pi pi-volume-up",
    video: "pi pi-video",
    text: "pi pi-file-edit",
    binary: "pi pi-file",
  };
  return icons[asset.media_type];
}
