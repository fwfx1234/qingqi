#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod features;

use anyhow::Result;

fn main() -> Result<()> {
    // 创建 tokio 多线程运行时：SSH 插件通过 tokio runtime 执行异步 IO
    // （SSH 连接、文件传输等）。不调用 rt.enter() 以避免干扰 GPUI 的事件循环。
    // runtime 在后台线程上运行，worker 线程处理所有 spawned tasks。
    let rt = tokio::runtime::Runtime::new()?;
    qingqi_feature_ssh::init_tokio_runtime(rt.handle().clone());

    // 后台线程保持 runtime 存活（GPUI 的 app.run() 会阻塞主线程）
    std::thread::spawn(move || {
        rt.block_on(std::future::pending::<()>());
    });

    let mut host = qingqi_app::app::runtime::bootstrap()?;
    features::registry::register_builtin_plugins(&mut host)?;
    qingqi_app::app::runtime::run(host)
}
