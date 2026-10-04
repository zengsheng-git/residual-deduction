// 简单日志: 控制台 + 应用数据目录下的按日文件
use std::fs;
use std::path::Path;

use tracing_appender::non_blocking;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub fn init_tracer(level: tracing::Level, dir: &Path) -> WorkerGuard {
    let _ = fs::create_dir_all(dir.join("logs"));
    let appender = tracing_appender::rolling::daily(dir.join("logs"), "runtime.log");
    let (file_writer, guard) = non_blocking(appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level.as_str()));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_ansi(false).with_writer(std::io::stdout))
        .with(fmt::layer().with_ansi(false).with_writer(file_writer))
        .init();
    guard
}
