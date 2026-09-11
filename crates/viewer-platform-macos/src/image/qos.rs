//! 解码工作线程的 macOS QoS 标注（性能优化方案一，见
//! docs/architecture/performance-optimization-plan.md §2.1）。
//!
//! FFI 刻意收敛在这一个函数里：只在解码线程的阻塞闭包开头按请求优先级
//! 设置一次 QoS —— 交互请求 user-initiated、预取 utility。设置失败只影响
//! 系统调度器的线程级提示，正确性仍由 `DecodeScheduler` 的并发闸门与优先级
//! 队列保证，因此 release 构建静默忽略、debug 构建断言。
//! 如需整体停用，把 `DECODE_QOS_ENABLED` 置为 false 即可（回滚开关）。

use viewer_application::ImagePriority;

const DECODE_QOS_ENABLED: bool = true;

/// 在当前线程上标注解码工作的 QoS 类别。仅供解码阻塞闭包/专用解码线程调用。
pub fn annotate_decode_thread(priority: ImagePriority) {
    if !DECODE_QOS_ENABLED {
        return;
    }
    let class = match priority {
        ImagePriority::Interactive => libc::qos_class_t::QOS_CLASS_USER_INITIATED,
        ImagePriority::Prefetch => libc::qos_class_t::QOS_CLASS_UTILITY,
    };
    // SAFETY: pthread_set_qos_class_self_np 只影响当前线程的调度提示，
    // 参数为合法的 qos_class_t 常量，无未定义行为。
    let result = unsafe { libc::pthread_set_qos_class_self_np(class, 0) };
    debug_assert_eq!(result, 0, "pthread_set_qos_class_self_np failed");
}

#[cfg(test)]
mod tests {
    use super::annotate_decode_thread;
    use viewer_application::ImagePriority;

    #[test]
    fn annotation_is_safe_on_the_current_thread_for_both_priorities() {
        annotate_decode_thread(ImagePriority::Interactive);
        annotate_decode_thread(ImagePriority::Prefetch);
    }
}
