//! 日志便利工具。
//!
//! 提供 `log_error!` 宏和 `log_and_return` 辅助函数，用于非关键路径上的
//! "best-effort" 操作——记录错误并返回默认值，替代 `let _ =` / `.ok()` /
//! `unwrap_or_default()` 等静默吞没错误的模式。
//!
//! # 使用原则
//! - 关键路径（数据持久化、配置保存）必须用 `?` 向上传播
//! - 非关键路径（UI 刷新、缓存写入、事件通知）可用 `log_error!`
//! - 需要同时给 UI 反馈 + 日志时用 `log_and_return`

/// 记录错误并返回默认值。
///
/// 替代 `let _ =` / `.ok()` / `unwrap_or_default()` 等静默吞没错误的模式。
///
/// # 用法
///
/// ```ignore
/// use qingqi_plugin::{log_error, log_and_return};
///
/// // 返回 T::default()
/// let value = log_error!(fallible_op(), warn, "操作失败");
///
/// // 返回指定的 fallback
/// let value = log_error!(fallible_op(), error, "关键操作失败", some_fallback);
/// ```
///
/// 支持的日志级别: `error`, `warn`, `info`, `debug`, `trace`
#[macro_export]
macro_rules! log_error {
    ($result:expr, $level:ident, $context:literal $(,)?) => {{
        match $result {
            Ok(v) => v,
            Err(e) => {
                ::tracing::$level!(error = %e, $context);
                ::std::default::Default::default()
            }
        }
    }};
    ($result:expr, $level:ident, $context:literal, $fallback:expr $(,)?) => {{
        match $result {
            Ok(v) => v,
            Err(e) => {
                ::tracing::$level!(error = %e, $context);
                $fallback
            }
        }
    }};
}

/// 记录错误并返回错误消息字符串给 UI 层。
///
/// 用于需要同时记录日志和向用户展示错误消息的场景。
///
/// # 用法
///
/// ```ignore
/// use qingqi_plugin::log_and_return;
///
/// let result = log_and_return(service.pause_job(id), "暂停下载失败");
/// if let Err(msg) = result {
///     self.message = msg;  // UI 提示
/// }
/// ```
pub fn log_and_return<T, E: std::fmt::Display>(
    result: Result<T, E>,
    context: &str,
) -> Result<T, String> {
    match result {
        Ok(v) => Ok(v),
        Err(e) => {
            tracing::error!(error = %e, "{context}");
            Err(format!("{context}: {e}"))
        }
    }
}
