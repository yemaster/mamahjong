import { ref } from "vue";

export function useFileDrop(onFiles: (files: File[]) => void | Promise<void>) {
  const active = ref(false);
  let depth = 0;

  function hasFiles(event: DragEvent) {
    return Array.from(event.dataTransfer?.types ?? []).includes("Files");
  }

  function enter(event: DragEvent) {
    if (!hasFiles(event)) return;
    depth += 1;
    active.value = true;
  }

  function over(event: DragEvent) {
    if (!hasFiles(event) || !event.dataTransfer) return;
    event.dataTransfer.dropEffect = "copy";
  }

  function leave() {
    depth = Math.max(0, depth - 1);
    if (depth === 0) active.value = false;
  }

  function drop(event: DragEvent) {
    depth = 0;
    active.value = false;
    const files = Array.from(event.dataTransfer?.files ?? []);
    if (files.length) void onFiles(files);
  }

  function reset() {
    depth = 0;
    active.value = false;
  }

  return { active, enter, over, leave, drop, reset };
}

export function acceptsAssetFile(file: File, accept: "image" | "audio" | "all"): boolean {
  if (accept === "all" || file.type.startsWith(`${accept}/`)) return true;
  const extension = file.name.split(".").pop()?.toLocaleLowerCase() ?? "";
  const extensions = accept === "image"
    ? ["png", "jpg", "jpeg", "gif", "webp", "svg", "avif"]
    : ["mp3", "ogg", "wav", "m4a", "aac", "flac", "opus"];
  return extensions.includes(extension);
}
