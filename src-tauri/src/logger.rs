use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// 轻量文件日志：单文件上限 512KB，超限时轮转为 .old（仅保留一份），
/// 总磁盘占用封顶约 1MB。日志频率与用户操作同量级（唤起/粘贴），无性能压力。
static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

const MAX_LOG_BYTES: u64 = 512 * 1024;

/// 初始化日志路径（app_data_dir/app.log），应用启动时调用一次
pub fn init(app_data_dir: &Path) {
    let _ = LOG_PATH.set(app_data_dir.join("app.log"));
}

/// 追加一行带时间戳的日志；dev 模式同步输出到控制台
pub fn log(msg: &str) {
    let ts = chrono::Local::now().format("%m-%d %H:%M:%S");
    let line = format!("[{}] {}\n", ts, msg);

    // dev 控制台可见（release 下 stderr 无人读取，代价可忽略）
    eprint!("{}", line);

    let Some(path) = LOG_PATH.get() else { return };

    // 超限轮转：app.log → app.log.old（覆盖旧档）
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = std::fs::rename(path, path.with_extension("log.old"));
        }
    }

    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}
