import { createHash } from "node:crypto";
import { createReadStream, createWriteStream, promises as fs } from "node:fs";
import path from "node:path";

export async function splitLargeFile(sourcePath: string, outputDir: string, chunkSizeMb = 100): Promise<{
  chunkCount: number;
  outputDir: string;
  totalBytes: number;
  manifestPath: string;
}> {
  await fs.mkdir(outputDir, { recursive: true });
  const stat = await fs.stat(sourcePath);
  const chunkBytes = chunkSizeMb * 1024 * 1024;
  const chunkCount = Math.ceil(stat.size / chunkBytes);
  const basename = path.basename(sourcePath);
  const partFiles: Array<{ file: string; sha256: string; size: number }> = [];

  for (let i = 0; i < chunkCount; i += 1) {
    const start = i * chunkBytes;
    const end = Math.min(stat.size - 1, start + chunkBytes - 1);
    const file = path.join(outputDir, `${basename}.part${String(i + 1).padStart(4, "0")}`);
    const hash = createHash("sha256");

    await new Promise<void>((resolve, reject) => {
      const reader = createReadStream(sourcePath, { start, end });
      const writer = createWriteStream(file);
      reader.on("data", (chunk: string | Buffer) => hash.update(chunk));
      reader.on("error", reject);
      writer.on("error", reject);
      writer.on("close", resolve);
      reader.pipe(writer);
    });

    const partStat = await fs.stat(file);
    partFiles.push({ file: path.basename(file), sha256: hash.digest("hex"), size: partStat.size });
  }

  const manifest = {
    sourceFile: basename,
    totalBytes: stat.size,
    chunkCount,
    chunkSizeMb,
    parts: partFiles
  };
  const manifestPath = path.join(outputDir, `${basename}.manifest.json`);
  await fs.writeFile(manifestPath, JSON.stringify(manifest, null, 2), "utf8");

  return {
    chunkCount,
    outputDir,
    totalBytes: stat.size,
    manifestPath
  };
}

export async function mergeSmallFiles(parts: string[], outputPath: string): Promise<{
  outputPath: string;
  totalBytes: number;
  sha256: string;
}> {
  if (parts.length === 0) {
    throw new Error("No part files provided.");
  }
  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  const writer = createWriteStream(outputPath);

  let totalBytes = 0;
  for (const part of parts) {
    await new Promise<void>((resolve, reject) => {
      const reader = createReadStream(part);
      reader.on("error", reject);
      writer.on("error", reject);
      reader.on("end", resolve);
      reader.pipe(writer, { end: false });
    });
    const stat = await fs.stat(part);
    totalBytes += stat.size;
  }
  await new Promise<void>((resolve) => writer.end(resolve));

  const hash = createHash("sha256");
  await new Promise<void>((resolve, reject) => {
    const reader = createReadStream(outputPath);
    reader.on("data", (chunk: string | Buffer) => hash.update(chunk));
    reader.on("end", resolve);
    reader.on("error", reject);
  });

  return {
    outputPath,
    totalBytes,
    sha256: hash.digest("hex")
  };
}
