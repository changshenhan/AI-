import sharp from "sharp";
import { readFileSync } from "fs";
import { fileURLToPath } from "url";
import { dirname, join } from "path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");
const svgPath = join(root, "src-tauri/icons/app-icon.svg");
const outPath = join(root, "src-tauri/icons/icon-source-1024.png");

const buf = readFileSync(svgPath);
await sharp(buf).resize(1024, 1024).png().toFile(outPath);
console.log("wrote", outPath);
