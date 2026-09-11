//! 统一解码调度器（性能优化方案一，见 docs/architecture/performance-optimization-plan.md §2）。
//!
//! 所有"生成图像表示"（缩略图/预览/全图）的解码任务都经过同一个调度器获取许可：
//! 交互请求（正在看的内容）插队，预取请求（滚动方向预测）排队，全局并发有界。
//!
//! ## 与现有闸门的分工（刻意保持独立，请勿"统一"掉）
//!
//! - `video_thumbnail.rs` 的抽帧闸门（1）：mpv 单路解码的业务级限流；
//! - `review/assets.rs` 的证据哈希闸门（≤4）：CPU 密集哈希的业务级限流；
//! - `viewer-application::scheduler::DerivedWorkScheduler`：跨领域工作类
//!   （查询/发布/可见派生/索引/探测/指纹）之间的优先排序；
//! - 本调度器只关心**像素管线的全局并发上限与交互/预取优先级**，
//!   目标是滚动时集中解码不涌向主线程时间片（QoS 标注在 platform 层落地）。

use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
use viewer_application::ImagePriority;

/// 默认解码并发：不超过典型 M 系列芯片的性能核数量，settings 后续可调 1..=6。
pub const DEFAULT_DECODE_CONCURRENCY: usize = 3;

/// 解码调度器允许的最大并发上限（防御性上限，避免误配破坏整机响应）。
pub const MAX_DECODE_CONCURRENCY: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum DecodePriority {
    /// 预取（滚动方向预测的缩略图），只在无交互等待时被唤醒。
    Prefetch,
    /// 交互（正在查看的缩略图/预览/全图），到队首即被唤醒。
    Interactive,
}

impl From<ImagePriority> for DecodePriority {
    fn from(priority: ImagePriority) -> Self {
        match priority {
            ImagePriority::Interactive => Self::Interactive,
            ImagePriority::Prefetch => Self::Prefetch,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DecodeSchedulerStats {
    pub limit: usize,
    pub in_flight: usize,
    pub waiting_interactive: usize,
    pub waiting_prefetch: usize,
    /// 在队列中等待时被放弃（任务被 abort / 请求取消）的累计次数。
    pub cancelled_while_waiting: u64,
}

#[derive(Debug)]
struct QueuedTicket {
    priority: DecodePriority,
    seq: u64,
}

#[derive(Debug)]
struct DecodeSchedulerState {
    limit: usize,
    in_flight: usize,
    next_seq: u64,
    waiters: std::collections::VecDeque<QueuedTicket>,
    cancelled_while_waiting: u64,
}

impl DecodeSchedulerState {
    fn waiting(&self, priority: DecodePriority) -> usize {
        self.waiters
            .iter()
            .filter(|ticket| ticket.priority == priority)
            .count()
    }

    /// 下一个可放行的候选：优先级最高者优先；同优先级内按入队序（FCFS）。
    fn admission_candidate(&self) -> Option<u64> {
        let mut best: Option<&QueuedTicket> = None;
        for ticket in &self.waiters {
            let better = match best {
                None => true,
                Some(head) => {
                    ticket.priority > head.priority
                        || (ticket.priority == head.priority && ticket.seq < head.seq)
                }
            };
            if better {
                best = Some(ticket);
            }
        }
        best.map(|ticket| ticket.seq)
    }
}

#[derive(Debug)]
pub struct DecodeScheduler {
    state: Mutex<DecodeSchedulerState>,
    changed: Notify,
}

impl DecodeScheduler {
    pub fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(DecodeSchedulerState {
                limit: limit.min(MAX_DECODE_CONCURRENCY),
                in_flight: 0,
                next_seq: 0,
                waiters: std::collections::VecDeque::new(),
                cancelled_while_waiting: 0,
            }),
            changed: Notify::new(),
        })
    }

    /// 获取一个解码许可。队列语义：严格 (priority, 入队序) 排序，只有队首且
    /// `in_flight < limit` 时才被放行。返回的 ticket 被 drop 即释放槽位并唤醒队首。
    pub async fn acquire(self: &Arc<Self>, priority: DecodePriority) -> DecodeTicket {
        let seq = {
            let mut state = self.lock_state();
            state.next_seq = state.next_seq.wrapping_add(1);
            let seq = state.next_seq;
            state.waiters.push_back(QueuedTicket { priority, seq });
            seq
        };
        let mut registration = DecodeWaitRegistration {
            scheduler: Arc::clone(self),
            seq,
            registered: true,
        };
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            {
                let mut state = self.lock_state();
                if state.in_flight < state.limit && state.admission_candidate() == Some(seq) {
                    if let Some(position) =
                        state.waiters.iter().position(|ticket| ticket.seq == seq)
                    {
                        state.waiters.remove(position);
                    }
                    state.in_flight += 1;
                    registration.registered = false;
                    return DecodeTicket {
                        scheduler: Arc::clone(self),
                    };
                }
            }
            changed.await;
        }
    }

    /// 收缩或放大并发上限（内存压力联动入口）。收缩时在途任务不被打断，
    /// 只拒绝超出新上限的新任务。
    pub fn set_limit(&self, limit: usize) {
        {
            let mut state = self.lock_state();
            state.limit = limit.min(MAX_DECODE_CONCURRENCY);
        }
        self.changed.notify_waiters();
    }

    /// 遥测快照：暴露 in_flight / waiting / cancelled，用于发现许可卡死。
    pub fn snapshot(&self) -> DecodeSchedulerStats {
        let state = self.lock_state();
        DecodeSchedulerStats {
            limit: state.limit,
            in_flight: state.in_flight,
            waiting_interactive: state.waiting(DecodePriority::Interactive),
            waiting_prefetch: state.waiting(DecodePriority::Prefetch),
            cancelled_while_waiting: state.cancelled_while_waiting,
        }
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, DecodeSchedulerState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

struct DecodeWaitRegistration {
    scheduler: Arc<DecodeScheduler>,
    seq: u64,
    registered: bool,
}

impl Drop for DecodeWaitRegistration {
    fn drop(&mut self) {
        if !self.registered {
            return;
        }
        {
            let mut state = self.scheduler.lock_state();
            if let Some(position) = state
                .waiters
                .iter()
                .position(|ticket| ticket.seq == self.seq)
            {
                state.waiters.remove(position);
            }
            state.cancelled_while_waiting += 1;
        }
        self.scheduler.changed.notify_waiters();
    }
}

#[derive(Debug)]
pub struct DecodeTicket {
    scheduler: Arc<DecodeScheduler>,
}

impl Drop for DecodeTicket {
    fn drop(&mut self) {
        {
            let mut state = self.scheduler.lock_state();
            debug_assert!(state.in_flight > 0);
            state.in_flight = state.in_flight.saturating_sub(1);
        }
        self.scheduler.changed.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_DECODE_CONCURRENCY, DecodePriority, DecodeScheduler};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn concurrent_admission_never_exceeds_the_limit() {
        let scheduler = DecodeScheduler::new(DEFAULT_DECODE_CONCURRENCY);
        let in_flight = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let peak = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut holders = Vec::new();
        for _ in 0..200 {
            let scheduler = Arc::clone(&scheduler);
            let in_flight = Arc::clone(&in_flight);
            let peak = Arc::clone(&peak);
            holders.push(tokio::spawn(async move {
                let _ticket = scheduler.acquire(DecodePriority::Interactive).await;
                let current = in_flight.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                peak.fetch_max(current, std::sync::atomic::Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(1)).await;
                in_flight.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            }));
        }
        for holder in holders {
            holder.await.unwrap();
        }
        assert_eq!(peak.load(std::sync::atomic::Ordering::SeqCst), 3);
        let stats = scheduler.snapshot();
        assert_eq!(stats.in_flight, 0);
        assert_eq!(stats.waiting_interactive + stats.waiting_prefetch, 0);
    }

    #[tokio::test]
    async fn interactive_waiter_is_admitted_before_queued_prefetch() {
        let scheduler = DecodeScheduler::new(1);
        let first = scheduler.acquire(DecodePriority::Interactive).await;
        let prefetch = {
            let scheduler = Arc::clone(&scheduler);
            tokio::spawn(async move { scheduler.acquire(DecodePriority::Prefetch).await })
        };
        let interactive = {
            let scheduler = Arc::clone(&scheduler);
            tokio::spawn(async move { scheduler.acquire(DecodePriority::Interactive).await })
        };
        tokio::task::yield_now().await;

        drop(first);
        let ticket = tokio::time::timeout(Duration::from_millis(200), interactive)
            .await
            .expect("interactive waiter must jump the queued prefetch")
            .unwrap();
        assert!(!prefetch.is_finished(), "prefetch must keep waiting");
        drop(ticket);

        tokio::time::timeout(Duration::from_millis(200), prefetch)
            .await
            .expect("prefetch resumes after the interactive slot is released")
            .unwrap();
    }

    #[tokio::test]
    async fn abandoning_a_waiter_does_not_stall_the_queue() {
        let scheduler = DecodeScheduler::new(1);
        let first = scheduler.acquire(DecodePriority::Prefetch).await;
        let abandoned = {
            let scheduler = Arc::clone(&scheduler);
            tokio::spawn(async move { scheduler.acquire(DecodePriority::Prefetch).await })
        };
        tokio::task::yield_now().await;
        abandoned.abort();
        assert!(abandoned.await.unwrap_err().is_cancelled());

        drop(first);
        tokio::time::timeout(
            Duration::from_millis(200),
            scheduler.acquire(DecodePriority::Prefetch),
        )
        .await
        .expect("cancelled waiter must be removed from the queue");
        assert_eq!(scheduler.snapshot().cancelled_while_waiting, 1);
    }

    #[tokio::test]
    async fn shrinking_the_limit_does_not_cancel_in_flight_work() {
        let scheduler = DecodeScheduler::new(3);
        let first = scheduler.acquire(DecodePriority::Interactive).await;
        let second = scheduler.acquire(DecodePriority::Interactive).await;
        let third = scheduler.acquire(DecodePriority::Interactive).await;

        scheduler.set_limit(1);
        assert_eq!(scheduler.snapshot().in_flight, 3);

        let newcomer = {
            let scheduler = Arc::clone(&scheduler);
            tokio::spawn(async move { scheduler.acquire(DecodePriority::Interactive).await })
        };
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !newcomer.is_finished(),
            "shrunken limit must refuse new work"
        );

        drop(first);
        drop(second);
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !newcomer.is_finished(),
            "in-flight equal to the new limit still blocks newcomers"
        );

        drop(third);
        tokio::time::timeout(Duration::from_millis(200), newcomer)
            .await
            .expect("newcomer is admitted once in-flight drops below the new limit")
            .unwrap();
    }
}
