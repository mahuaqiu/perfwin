use std::collections::VecDeque;
use std::sync::Arc;
use parking_lot::Mutex;
use crate::data::Sample;

/// 环形缓冲区，用于存储采样数据
/// 查询时返回增量数据并清空
/// 使用 Arc 实现跨线程共享
pub struct RingBuffer {
    buffer: Arc<Mutex<VecDeque<Sample>>>,
}

impl RingBuffer {
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// 添加采样数据
    pub fn push(&self, sample: Sample) {
        let mut buf = self.buffer.lock();
        buf.push_back(sample);
    }

    /// 获取所有增量数据并清空
    pub fn drain(&self) -> Vec<Sample> {
        let mut buf = self.buffer.lock();
        buf.drain(..).collect()
    }

    /// 获取当前数据数量
    pub fn len(&self) -> usize {
        self.buffer.lock().len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 克隆共享引用（用于跨线程传递）
    pub fn clone_arc(&self) -> Self {
        Self {
            buffer: Arc::clone(&self.buffer),
        }
    }
}

impl Clone for RingBuffer {
    fn clone(&self) -> Self {
        self.clone_arc()
    }
}

impl Default for RingBuffer {
    fn default() -> Self {
        Self::new()
    }
}