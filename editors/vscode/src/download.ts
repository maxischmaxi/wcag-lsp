import * as https from "https";
import * as fs from "fs";
import * as path from "path";
import * as zlib from "zlib";
import * as vscode from "vscode";

const REPO = "maxischmaxi/wcag-lsp";
const BINARY_NAME = "wcag-lsp";

interface PlatformInfo {
  target: string;
  ext: string;
  binaryName: string;
}

function getPlatformInfo(): PlatformInfo {
  const platform = process.platform;
  const arch = process.arch;

  let target: string;
  switch (`${platform}-${arch}`) {
    case "linux-x64":
      target = "x86_64-unknown-linux-musl";
      break;
    case "darwin-x64":
      target = "x86_64-apple-darwin";
      break;
    case "darwin-arm64":
      target = "aarch64-apple-darwin";
      break;
    case "win32-x64":
      target = "x86_64-pc-windows-msvc";
      break;
    default:
      throw new Error(`Unsupported platform: ${platform}-${arch}`);
  }

  const isWindows = platform === "win32";
  return {
    target,
    ext: isWindows ? "zip" : "tar.gz",
    binaryName: isWindows ? `${BINARY_NAME}.exe` : BINARY_NAME,
  };
}

async function getLatestVersion(): Promise<string> {
  return new Promise((resolve, reject) => {
    const options = {
      hostname: "api.github.com",
      path: `/repos/${REPO}/releases/latest`,
      headers: { "User-Agent": "wcag-lsp-vscode" },
    };
    https
      .get(options, (res) => {
        if (res.statusCode === 302 || res.statusCode === 301) {
          const location = res.headers.location;
          if (location) {
            const match = location.match(/\/tag\/([^/]+)$/);
            if (match) {
              resolve(match[1]);
              return;
            }
          }
        }
        let data = "";
        res.on("data", (chunk: Buffer) => (data += chunk));
        res.on("end", () => {
          try {
            const json = JSON.parse(data);
            if (!json.tag_name) {
              reject(new Error("No tag_name found in GitHub release response"));
              return;
            }
            resolve(json.tag_name);
          } catch {
            reject(new Error("Failed to parse GitHub release info"));
          }
        });
      })
      .on("error", reject);
  });
}

function downloadFile(url: string, maxRedirects = 5): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    if (maxRedirects <= 0) {
      reject(new Error("Too many redirects"));
      return;
    }
    https
      .get(url, { headers: { "User-Agent": "wcag-lsp-vscode" } }, (res) => {
        if (res.statusCode === 302 || res.statusCode === 301) {
          if (res.headers.location) {
            downloadFile(res.headers.location, maxRedirects - 1).then(
              resolve,
              reject,
            );
            return;
          }
        }
        if (res.statusCode !== 200) {
          reject(new Error(`Download failed with status ${res.statusCode}`));
          return;
        }
        const chunks: Buffer[] = [];
        res.on("data", (chunk: Buffer) => chunks.push(chunk));
        res.on("end", () => resolve(Buffer.concat(chunks)));
      })
      .on("error", reject);
  });
}

async function extractTarGz(
  archive: Buffer,
  destDir: string,
  binaryName: string,
): Promise<string> {
  // Simple tar.gz extraction: decompress, then parse tar format
  const decompressed = zlib.gunzipSync(archive);
  // Parse tar: each entry is 512-byte header + data rounded to 512
  let offset = 0;
  while (offset < decompressed.length) {
    const header = decompressed.subarray(offset, offset + 512);
    if (header.every((b) => b === 0)) break;

    const name = header.subarray(0, 100).toString("utf8").replace(/\0/g, "");
    const sizeOctal = header
      .subarray(124, 136)
      .toString("utf8")
      .replace(/\0/g, "")
      .trim();
    const size = parseInt(sizeOctal, 8);
    offset += 512;

    const fileName = path.basename(name);
    if (fileName === binaryName && size > 0) {
      const destPath = path.join(destDir, binaryName);
      fs.writeFileSync(destPath, decompressed.subarray(offset, offset + size));
      fs.chmodSync(destPath, 0o755);
      return destPath;
    }

    offset += Math.ceil(size / 512) * 512;
  }
  throw new Error(`Binary '${binaryName}' not found in archive`);
}

async function extractZip(
  archive: Buffer,
  destDir: string,
  binaryName: string,
): Promise<string> {
  // Minimal zip extraction for a single file
  // Find End of Central Directory
  let eocdOffset = archive.length - 22;
  while (eocdOffset >= 0 && archive.readUInt32LE(eocdOffset) !== 0x06054b50) {
    eocdOffset--;
  }
  if (eocdOffset < 0) throw new Error("Invalid zip file");

  const cdOffset = archive.readUInt32LE(eocdOffset + 16);
  let pos = cdOffset;

  while (pos < eocdOffset) {
    if (archive.readUInt32LE(pos) !== 0x02014b50) break;
    const nameLen = archive.readUInt16LE(pos + 28);
    const extraLen = archive.readUInt16LE(pos + 30);
    const commentLen = archive.readUInt16LE(pos + 32);
    const localHeaderOffset = archive.readUInt32LE(pos + 42);
    const name = archive
      .subarray(pos + 46, pos + 46 + nameLen)
      .toString("utf8");

    if (path.basename(name) === binaryName) {
      const compressionMethod = archive.readUInt16LE(localHeaderOffset + 8);
      const localNameLen = archive.readUInt16LE(localHeaderOffset + 26);
      const localExtraLen = archive.readUInt16LE(localHeaderOffset + 28);
      const compSize = archive.readUInt32LE(localHeaderOffset + 18);
      const dataStart = localHeaderOffset + 30 + localNameLen + localExtraLen;
      let data = archive.subarray(dataStart, dataStart + compSize);

      if (compressionMethod === 8) {
        data = zlib.inflateRawSync(data);
      } else if (compressionMethod !== 0) {
        throw new Error(
          `Unsupported zip compression method: ${compressionMethod}`,
        );
      }

      const destPath = path.join(destDir, binaryName);
      fs.writeFileSync(destPath, data);
      return destPath;
    }

    pos += 46 + nameLen + extraLen + commentLen;
  }
  throw new Error(`Binary '${binaryName}' not found in zip`);
}

export async function ensureBinary(storageDir: string): Promise<string> {
  const info = getPlatformInfo();
  const binaryPath = path.join(storageDir, info.binaryName);

  if (fs.existsSync(binaryPath)) {
    return binaryPath;
  }

  const serverPath = await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: "WCAG LSP: Downloading server...",
      cancellable: false,
    },
    async (progress) => {
      progress.report({ message: "Fetching latest version..." });
      const version = await getLatestVersion();

      progress.report({
        message: `Downloading ${version} for ${info.target}...`,
      });
      const url = `https://github.com/${REPO}/releases/download/${version}/${BINARY_NAME}-${info.target}.${info.ext}`;
      const archive = await downloadFile(url);

      progress.report({ message: "Extracting..." });
      fs.mkdirSync(storageDir, { recursive: true });

      if (info.ext === "zip") {
        return extractZip(archive, storageDir, info.binaryName);
      }
      return extractTarGz(archive, storageDir, info.binaryName);
    },
  );

  return serverPath;
}
