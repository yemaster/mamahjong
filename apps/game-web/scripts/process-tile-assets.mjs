import {
  mkdirSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { basename, join } from "node:path";
import { deflateSync, inflateSync } from "node:zlib";

const PNG_SIGNATURE = Buffer.from([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
]);
const CANVAS_WIDTH = 70;
const CANVAS_HEIGHT = 100;
const CONTENT_X = 1;
const CONTENT_Y = 1;
const CONTENT_WIDTH = 68;
const CONTENT_HEIGHT = 97;
const SOURCE_INSET_RATIO = 0.03;
const PINZU_SOURCE_INSET_RATIO = 0.005;
const LARGE_PINZU_SOURCE_INSET_RATIO = -0.02;
const HONOR_SOURCE_INSET_RATIO = 0.055;
const FACE_COLOR = [223, 223, 220, 255];
const BACK_COLOR = [104, 153, 116, 255];

function paeth(left, above, upperLeft) {
  const estimate = left + above - upperLeft;
  const leftDistance = Math.abs(estimate - left);
  const aboveDistance = Math.abs(estimate - above);
  const upperLeftDistance = Math.abs(estimate - upperLeft);
  if (leftDistance <= aboveDistance && leftDistance <= upperLeftDistance) {
    return left;
  }
  return aboveDistance <= upperLeftDistance ? above : upperLeft;
}

function decodePng(path) {
  const file = readFileSync(path);
  if (!file.subarray(0, 8).equals(PNG_SIGNATURE)) {
    throw new Error(`不是 PNG 文件：${path}`);
  }

  let offset = 8;
  let width = 0;
  let height = 0;
  const compressed = [];
  while (offset < file.length) {
    const length = file.readUInt32BE(offset);
    const type = file.toString("ascii", offset + 4, offset + 8);
    const data = file.subarray(offset + 8, offset + 8 + length);
    offset += length + 12;
    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      if (
        data[8] !== 8 ||
        data[9] !== 6 ||
        data[12] !== 0
      ) {
        throw new Error(`仅支持非交错 8 位 RGBA PNG：${path}`);
      }
    } else if (type === "IDAT") {
      compressed.push(data);
    } else if (type === "IEND") {
      break;
    }
  }

  const bytesPerPixel = 4;
  const rowBytes = width * bytesPerPixel;
  const filtered = inflateSync(Buffer.concat(compressed));
  const pixels = Buffer.alloc(width * height * bytesPerPixel);
  let sourceOffset = 0;
  for (let y = 0; y < height; y += 1) {
    const filter = filtered[sourceOffset];
    sourceOffset += 1;
    const rowOffset = y * rowBytes;
    for (let x = 0; x < rowBytes; x += 1) {
      const raw = filtered[sourceOffset + x];
      const left = x >= bytesPerPixel
        ? pixels[rowOffset + x - bytesPerPixel]
        : 0;
      const above = y > 0
        ? pixels[rowOffset + x - rowBytes]
        : 0;
      const upperLeft = y > 0 && x >= bytesPerPixel
        ? pixels[rowOffset + x - rowBytes - bytesPerPixel]
        : 0;
      let value = raw;
      if (filter === 1) value += left;
      else if (filter === 2) value += above;
      else if (filter === 3) value += Math.floor((left + above) / 2);
      else if (filter === 4) value += paeth(left, above, upperLeft);
      else if (filter !== 0) throw new Error(`未知 PNG 滤镜：${filter}`);
      pixels[rowOffset + x] = value & 0xff;
    }
    sourceOffset += rowBytes;
  }
  return { width, height, pixels };
}

function removeConnectedOuterFrame(image) {
  const { width, height, pixels } = image;
  const visited = new Uint8Array(width * height);
  const queue = [];
  let cursor = 0;

  const isOuterFrame = (x, y) => {
    const offset = (y * width + x) * 4;
    const red = pixels[offset];
    const green = pixels[offset + 1];
    const blue = pixels[offset + 2];
    const alpha = pixels[offset + 3];
    if (alpha < 16) return true;
    const brightest = Math.max(red, green, blue);
    const darkest = Math.min(red, green, blue);
    return brightest < 190 && brightest - darkest < 34;
  };
  const enqueue = (x, y) => {
    if (x < 0 || x >= width || y < 0 || y >= height) return;
    const index = y * width + x;
    if (visited[index] || !isOuterFrame(x, y)) return;
    visited[index] = 1;
    queue.push([x, y]);
  };

  for (let x = 0; x < width; x += 1) {
    enqueue(x, 0);
    enqueue(x, height - 1);
  }
  for (let y = 0; y < height; y += 1) {
    enqueue(0, y);
    enqueue(width - 1, y);
  }
  while (cursor < queue.length) {
    const [x, y] = queue[cursor];
    cursor += 1;
    enqueue(x - 1, y);
    enqueue(x + 1, y);
    enqueue(x, y - 1);
    enqueue(x, y + 1);
  }

  for (let index = 0; index < visited.length; index += 1) {
    if (!visited[index]) continue;
    const offset = index * 4;
    pixels[offset] = FACE_COLOR[0];
    pixels[offset + 1] = FACE_COLOR[1];
    pixels[offset + 2] = FACE_COLOR[2];
    pixels[offset + 3] = FACE_COLOR[3];
  }

  const edgeX = Math.ceil(width * 0.16);
  const edgeY = Math.ceil(height * 0.1);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      if (
        x >= edgeX &&
        x < width - edgeX &&
        y >= edgeY &&
        y < height - edgeY
      ) {
        continue;
      }
      const offset = (y * width + x) * 4;
      const red = pixels[offset];
      const green = pixels[offset + 1];
      const blue = pixels[offset + 2];
      if (
        Math.max(red, green, blue) < 250 &&
        Math.max(red, green, blue) - Math.min(red, green, blue) < 34
      ) {
        pixels[offset] = FACE_COLOR[0];
        pixels[offset + 1] = FACE_COLOR[1];
        pixels[offset + 2] = FACE_COLOR[2];
        pixels[offset + 3] = FACE_COLOR[3];
      }
    }
  }
}

function sampleBilinear(image, x, y, channel) {
  const x0 = Math.max(0, Math.min(image.width - 1, Math.floor(x)));
  const y0 = Math.max(0, Math.min(image.height - 1, Math.floor(y)));
  const x1 = Math.min(image.width - 1, x0 + 1);
  const y1 = Math.min(image.height - 1, y0 + 1);
  const xRatio = x - x0;
  const yRatio = y - y0;
  const value = (sampleX, sampleY) =>
    image.pixels[(sampleY * image.width + sampleX) * 4 + channel];
  const top = value(x0, y0) * (1 - xRatio) + value(x1, y0) * xRatio;
  const bottom = value(x0, y1) * (1 - xRatio) + value(x1, y1) * xRatio;
  return Math.round(top * (1 - yRatio) + bottom * yRatio);
}

function normalizeFace(image, sourceInsetRatio = SOURCE_INSET_RATIO) {
  removeConnectedOuterFrame(image);
  const sourceWidth = image.width * (1 - sourceInsetRatio * 2);
  const sourceHeight = image.height * (1 - sourceInsetRatio * 2);
  const sourceLeft = image.width * sourceInsetRatio;
  const sourceTop = image.height * sourceInsetRatio;
  const output = Buffer.alloc(CANVAS_WIDTH * CANVAS_HEIGHT * 4);
  for (let offset = 0; offset < output.length; offset += 4) {
    output[offset] = FACE_COLOR[0];
    output[offset + 1] = FACE_COLOR[1];
    output[offset + 2] = FACE_COLOR[2];
    output[offset + 3] = FACE_COLOR[3];
  }
  for (let y = 0; y < CONTENT_HEIGHT; y += 1) {
    for (let x = 0; x < CONTENT_WIDTH; x += 1) {
      const sourceX =
        sourceLeft + ((x + 0.5) * sourceWidth) / CONTENT_WIDTH - 0.5;
      const sourceY =
        sourceTop + ((y + 0.5) * sourceHeight) / CONTENT_HEIGHT - 0.5;
      const destination =
        ((y + CONTENT_Y) * CANVAS_WIDTH + x + CONTENT_X) * 4;
      for (let channel = 0; channel < 4; channel += 1) {
        output[destination + channel] = sampleBilinear(
          image,
          sourceX,
          sourceY,
          channel,
        );
      }
      const red = output[destination];
      const green = output[destination + 1];
      const blue = output[destination + 2];
      if (
        Math.min(red, green, blue) > 180 &&
        Math.max(red, green, blue) - Math.min(red, green, blue) < 30
      ) {
        output[destination] = Math.round(red * FACE_COLOR[0] / 255);
        output[destination + 1] = Math.round(green * FACE_COLOR[1] / 255);
        output[destination + 2] = Math.round(blue * FACE_COLOR[2] / 255);
      }
    }
  }
  for (let y = 0; y < CANVAS_HEIGHT; y += 1) {
    for (let x = 0; x < CANVAS_WIDTH; x += 1) {
      if (
        x < 4 ||
        x >= CANVAS_WIDTH - 4 ||
        y < 3 ||
        y >= CANVAS_HEIGHT - 3
      ) {
        const offset = (y * CANVAS_WIDTH + x) * 4;
        output[offset] = FACE_COLOR[0];
        output[offset + 1] = FACE_COLOR[1];
        output[offset + 2] = FACE_COLOR[2];
        output[offset + 3] = FACE_COLOR[3];
        continue;
      }
      if (x >= 9 && x < 61 && y >= 9 && y < 91) continue;
      const offset = (y * CANVAS_WIDTH + x) * 4;
      const red = output[offset];
      const green = output[offset + 1];
      const blue = output[offset + 2];
      if (
        Math.max(red, green, blue) - Math.min(red, green, blue) < 34
      ) {
        output[offset] = FACE_COLOR[0];
        output[offset + 1] = FACE_COLOR[1];
        output[offset + 2] = FACE_COLOR[2];
        output[offset + 3] = FACE_COLOR[3];
      }
    }
  }
  return output;
}

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function pngChunk(type, data) {
  const typeBuffer = Buffer.from(type, "ascii");
  const chunk = Buffer.alloc(data.length + 12);
  chunk.writeUInt32BE(data.length, 0);
  typeBuffer.copy(chunk, 4);
  data.copy(chunk, 8);
  chunk.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), data.length + 8);
  return chunk;
}

function encodePng(pixels) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(CANVAS_WIDTH, 0);
  header.writeUInt32BE(CANVAS_HEIGHT, 4);
  header[8] = 8;
  header[9] = 6;
  const raw = Buffer.alloc((CANVAS_WIDTH * 4 + 1) * CANVAS_HEIGHT);
  for (let y = 0; y < CANVAS_HEIGHT; y += 1) {
    const rowOffset = y * (CANVAS_WIDTH * 4 + 1);
    raw[rowOffset] = 0;
    pixels.copy(
      raw,
      rowOffset + 1,
      y * CANVAS_WIDTH * 4,
      (y + 1) * CANVAS_WIDTH * 4,
    );
  }
  return Buffer.concat([
    PNG_SIGNATURE,
    pngChunk("IHDR", header),
    pngChunk("IDAT", deflateSync(raw, { level: 9 })),
    pngChunk("IEND", Buffer.alloc(0)),
  ]);
}

const [sourceRoot, outputRoot] = process.argv.slice(2);
if (!sourceRoot || !outputRoot) {
  throw new Error(
    "用法：node scripts/process-tile-assets.mjs <原始目录> <输出目录>",
  );
}

for (const artwork of ["jp", "cn"]) {
  const sourceDirectory = join(sourceRoot, artwork);
  const outputDirectory = join(outputRoot, artwork);
  mkdirSync(outputDirectory, { recursive: true });
  for (const file of readdirSync(sourceDirectory).filter((name) =>
    name.endsWith(".png")
  )) {
    const source = join(sourceDirectory, file);
    const destination = join(outputDirectory, basename(file));
    let pixels;
    if (file === "back.png") {
      pixels = Buffer.alloc(CANVAS_WIDTH * CANVAS_HEIGHT * 4);
      for (let offset = 0; offset < pixels.length; offset += 4) {
        pixels[offset] = BACK_COLOR[0];
        pixels[offset + 1] = BACK_COLOR[1];
        pixels[offset + 2] = BACK_COLOR[2];
        pixels[offset + 3] = 255;
      }
    } else {
      const sourceInsetRatio = /^(7|9)p\.png$/.test(file)
        ? LARGE_PINZU_SOURCE_INSET_RATIO
        : /^[0-9]p\.png$/.test(file)
          ? PINZU_SOURCE_INSET_RATIO
          : /^[0-9]z\.png$/.test(file)
            ? HONOR_SOURCE_INSET_RATIO
            : SOURCE_INSET_RATIO;
      pixels = normalizeFace(decodePng(source), sourceInsetRatio);
    }
    writeFileSync(destination, encodePng(pixels));
  }
}

console.log(`牌面处理完成：${outputRoot}`);
