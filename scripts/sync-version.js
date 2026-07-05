/**
 * 版本号同步脚本
 * 用法:
 *   npm run version -- patch       # 0.7.0 -> 0.7.1
 *   npm run version -- minor       # 0.7.1 -> 0.8.0
 *   npm run version -- major       # 0.8.0 -> 1.0.0
 *   npm run version -- 1.2.3       # 直接指定版本号
 *   npm run version                # 显示当前版本号
 *
 * 会同时更新 package.json、src-tauri/Cargo.toml、src-tauri/tauri.conf.json
 */
import { readFileSync, writeFileSync } from 'fs';
import { resolve } from 'path';

const root = resolve(import.meta.dirname, '..');

// 读取当前版本
const pkgPath = resolve(root, 'package.json');
const pkg = JSON.parse(readFileSync(pkgPath, 'utf-8'));
const oldVersion = pkg.version;

const arg = process.argv[2];

// 无参数时显示当前版本
if (!arg) {
  console.log(`当前版本: ${oldVersion}`);
  console.log('');
  console.log('用法:');
  console.log('  npm run version -- patch   递增补丁号');
  console.log('  npm run version -- minor   递增次版本号');
  console.log('  npm run version -- major   递增主版本号');
  console.log('  npm run version -- x.y.z   指定版本号');
  process.exit(0);
}

// 计算新版本号
let newVersion;
const [major, minor, patch] = oldVersion.split('.').map(Number);

switch (arg) {
  case 'patch':
    newVersion = `${major}.${minor}.${patch + 1}`;
    break;
  case 'minor':
    newVersion = `${major}.${minor + 1}.0`;
    break;
  case 'major':
    newVersion = `${major + 1}.0.0`;
    break;
  default:
    if (!/^\d+\.\d+\.\d+$/.test(arg)) {
      console.error(`错误: "${arg}" 不是有效的版本号或命令 (patch/minor/major/x.y.z)`);
      process.exit(1);
    }
    newVersion = arg;
}

if (newVersion === oldVersion) {
  console.log(`版本号未变化: ${oldVersion}`);
  process.exit(0);
}

// 更新 package.json
pkg.version = newVersion;
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
console.log(`  package.json:      ${oldVersion} -> ${newVersion}`);

// 更新 tauri.conf.json
const tauriConfPath = resolve(root, 'src-tauri/tauri.conf.json');
const tauriConf = JSON.parse(readFileSync(tauriConfPath, 'utf-8'));
tauriConf.version = newVersion;
writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n');
console.log(`  tauri.conf.json:   ${oldVersion} -> ${newVersion}`);

// 更新 Cargo.toml
const cargoPath = resolve(root, 'src-tauri/Cargo.toml');
let cargo = readFileSync(cargoPath, 'utf-8');
cargo = cargo.replace(/^version = ".*"$/m, `version = "${newVersion}"`);
writeFileSync(cargoPath, cargo);
console.log(`  Cargo.toml:        ${oldVersion} -> ${newVersion}`);

console.log(`\n✓ 版本已同步: ${oldVersion} -> ${newVersion}`);
console.log(`\n下一步:`);
console.log(`  git add -A && git commit -m "chore: bump version to ${newVersion}"`);
console.log(`  git tag v${newVersion} && git push origin main --tags`);
