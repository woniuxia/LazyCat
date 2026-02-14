import sharp from "sharp";

export interface ConvertImageOptions {
  inputPath: string;
  outputPath: string;
  width?: number;
  height?: number;
  cropX?: number;
  cropY?: number;
  cropWidth?: number;
  cropHeight?: number;
  format?: "png" | "jpeg" | "webp" | "avif";
  quality?: number;
}

export async function convertImage(options: ConvertImageOptions): Promise<{
  outputPath: string;
  width: number;
  height: number;
  size: number;
}> {
  let pipeline = sharp(options.inputPath);
  if (
    typeof options.cropX === "number" &&
    typeof options.cropY === "number" &&
    typeof options.cropWidth === "number" &&
    typeof options.cropHeight === "number"
  ) {
    pipeline = pipeline.extract({
      left: Math.max(0, options.cropX),
      top: Math.max(0, options.cropY),
      width: Math.max(1, options.cropWidth),
      height: Math.max(1, options.cropHeight)
    });
  }

  if (options.width || options.height) {
    pipeline = pipeline.resize(options.width, options.height, {
      fit: "inside",
      withoutEnlargement: true
    });
  }

  const format = options.format ?? inferFormatFromPath(options.outputPath);
  if (format === "jpeg") {
    pipeline = pipeline.jpeg({ quality: options.quality ?? 80 });
  } else if (format === "png") {
    pipeline = pipeline.png({ quality: options.quality ?? 80 });
  } else if (format === "webp") {
    pipeline = pipeline.webp({ quality: options.quality ?? 80 });
  } else if (format === "avif") {
    pipeline = pipeline.avif({ quality: options.quality ?? 70 });
  }

  const info = await pipeline.toFile(options.outputPath);
  return {
    outputPath: options.outputPath,
    width: info.width,
    height: info.height,
    size: info.size
  };
}

function inferFormatFromPath(outputPath: string): "png" | "jpeg" | "webp" | "avif" {
  const lower = outputPath.toLowerCase();
  if (lower.endsWith(".jpg") || lower.endsWith(".jpeg")) return "jpeg";
  if (lower.endsWith(".webp")) return "webp";
  if (lower.endsWith(".avif")) return "avif";
  return "png";
}
