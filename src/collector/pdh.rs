// PDH 采集器 - 进程级 GPU
// Windows PDH (Performance Data Helper) API 实现

#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use windows::core::PCSTR;
#[cfg(target_os = "windows")]
use windows::Win32::System::Performance::{
    PdhAddCounterA, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue, PdhOpenQueryA,
    PdhRemoveCounter, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
};

/// PDH 查询句柄（windows 0.58 使用 isize）
#[cfg(target_os = "windows")]
#[allow(non_camel_case_types)]
type PDH_HQUERY = isize;

/// PDH 计数器句柄（windows 0.58 使用 isize）
#[cfg(target_os = "windows")]
#[allow(non_camel_case_types)]
type PDH_HCOUNTER = isize;

#[cfg(target_os = "windows")]
use crate::data::ProcessInfo;

/// GPU 进程级采集器 (Windows PDH 实现)
#[cfg(target_os = "windows")]
pub struct PdhCollector {
    query: PDH_HQUERY,
    counters: HashMap<u32, PDH_HCOUNTER>, // pid -> counter handle
}

#[cfg(target_os = "windows")]
impl PdhCollector {
    /// 创建新的 PDH 采集器
    pub fn new() -> anyhow::Result<Self> {
        let mut query: PDH_HQUERY = 0;
        let result = unsafe { PdhOpenQueryA(None, 0, &mut query) };
        if result != 0 {
            return Err(anyhow::anyhow!("PdhOpenQuery failed with error code: {}", result));
        }

        Ok(Self {
            query,
            counters: HashMap::new(),
        })
    }

    /// 为指定 PID 添加 GPU counter
    /// GPU counter 路径: \GPU Engine(pid_{}*)\Utilization Percentage
    /// 如果该 PID 的 counter 已存在，则跳过
    pub fn add_process_counter(&mut self, pid: u32) -> anyhow::Result<()> {
        // 检查是否已存在，避免重复添加
        if self.counters.contains_key(&pid) {
            return Ok(());
        }

        let counter_path = format!("\\GPU Engine(pid_{}*)\\Utilization Percentage", pid);
        // PdhAddCounterA 使用 ANSI 字符串
        let path_bytes: Vec<u8> = counter_path.bytes().chain(std::iter::once(0)).collect();

        let mut counter: PDH_HCOUNTER = 0;
        let result = unsafe { PdhAddCounterA(self.query, PCSTR(path_bytes.as_ptr()), 0, &mut counter) };
        if result != 0 {
            return Err(anyhow::anyhow!("PdhAddCounter failed for pid {} with error code: {}", pid, result));
        }

        self.counters.insert(pid, counter);
        Ok(())
    }

    /// 批量为多个 PID 添加 GPU counter
    /// 会自动跳过已存在的 counter，失败时继续处理下一个
    pub fn add_process_counters(&mut self, pids: &[u32]) {
        for &pid in pids {
            if let Err(e) = self.add_process_counter(pid) {
                log::warn!("Failed to add counter for pid {}: {}", pid, e);
            }
        }
    }

    /// 收集 GPU 数据
    pub fn collect(&mut self) -> anyhow::Result<HashMap<u32, f64>> {
        let result = unsafe { PdhCollectQueryData(self.query) };
        if result != 0 {
            return Err(anyhow::anyhow!("PdhCollectQueryData failed with error code: {}", result));
        }

        let mut results = HashMap::new();
        for (pid, counter) in &self.counters {
            let mut value = PDH_FMT_COUNTERVALUE::default();
            unsafe {
                let _ = PdhGetFormattedCounterValue(*counter, PDH_FMT_DOUBLE, None, &mut value);
                results.insert(*pid, value.Anonymous.doubleValue);
            }
        }

        Ok(results)
    }

    /// 更新进程 GPU 信息
    pub fn update_process_gpu(&mut self, processes: &mut [ProcessInfo]) -> anyhow::Result<()> {
        let gpu_data = self.collect()?;

        for proc in processes.iter_mut() {
            if let Some(gpu_percent) = gpu_data.get(&proc.pid) {
                proc.gpu_percent = *gpu_percent;
            }
        }

        Ok(())
    }

    /// 移除指定 PID 的 counter
    pub fn remove_process_counter(&mut self, pid: u32) {
        if let Some(counter) = self.counters.remove(&pid) {
            unsafe {
                let _ = PdhRemoveCounter(counter);
            };
        }
    }

    /// 清除所有 counters
    pub fn clear_counters(&mut self) {
        for counter in self.counters.values() {
            unsafe {
                let _ = PdhRemoveCounter(*counter);
            };
        }
        self.counters.clear();
    }
}

#[cfg(target_os = "windows")]
impl Drop for PdhCollector {
    fn drop(&mut self) {
        // 正确关闭 PDH query，释放资源
        if self.query != 0 {
            let _ = unsafe { PdhCloseQuery(self.query) };
        }
    }
}

// ============================================================================
// 非 Windows 平台的占位实现
// ============================================================================

#[cfg(not(target_os = "windows"))]
pub struct PdhCollector;

#[cfg(not(target_os = "windows"))]
impl PdhCollector {
    pub fn new() -> anyhow::Result<Self> {
        anyhow::bail!("PDH collector is only available on Windows");
    }

    pub fn add_process_counter(&mut self, _pid: u32) -> anyhow::Result<()> {
        anyhow::bail!("PDH collector is only available on Windows");
    }

    pub fn add_process_counters(&mut self, _pids: &[u32]) {
        // 非 Windows 平台无操作
    }

    pub fn collect(&mut self) -> anyhow::Result<std::collections::HashMap<u32, f64>> {
        anyhow::bail!("PDH collector is only available on Windows");
    }

    pub fn update_process_gpu(
        &mut self,
        _processes: &mut [crate::data::ProcessInfo],
    ) -> anyhow::Result<()> {
        anyhow::bail!("PDH collector is only available on Windows");
    }

    pub fn remove_process_counter(&mut self, _pid: u32) {}

    pub fn clear_counters(&mut self) {}
}