/**
 * 版本号同步脚本
 * 用法: npm run version -- 0.3.0
 * 会同时更新 package.json、src-tauri/Cargo.toml、src-tauri/tauri.conf.json
 */
import { readFileSync, writeFileSync } from 'fs';
import { resolve } from 'path';

const newVersion = process.argv[2];
if (!newVersion) {
  console.error('用法: npm run version -- <版本号>');
  console.error('示例: npm run version -- 0.3.0');
  process.exit(1);
}

if (!/^\d+\.\d+\.\d+$/.test(newVersion)) {
  console.error('版本号格式错误，应为 x.y.z');
  process.exit(1);
}

const root = resolve(import.meta.dirname, '..');

// 更新 package.json
const pkgPath = resolve(root, 'package.json');
const pkg = JSON.parse(readFileSync(pkgPath, 'utf-8'));
const oldVersion = pkg.version;
pkg.version = newVersion;
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
console.log(`package.json: ${oldVersion} -> ${newVersion}`);

// 更新 tauri.conf.json
const tauriConfPath = resolve(root, 'src-tauri/tauri.conf.json');
const tauriConf = JSON.parse(readFileSync(tauriConfPath, 'utf-8'));
tauriConf.version = newVersion;
writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n');
console.log(`tauri.conf.json: ${oldVersion} -> ${newVersion}`);

// 更新 Cargo.toml
const cargoPath = resolve(root, 'src-tauri/Cargo.toml');
let cargo = readFileSync(cargoPath, 'utf-8');
cargo = cargo.replace(/^version = ".*"$/m, `version = "${newVersion}"`);
writeFileSync(cargoPath, cargo);
console.log(`Cargo.toml: ${oldVersion} -> ${newVersion}`);

console.log(`\n版本已同步为 ${newVersion}`);
