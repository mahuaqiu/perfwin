// PDH 采集器 - 进程级 GPU
// Windows PDH (Performance Data Helper) API 实现

#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;
#[cfg(target_os = "windows")]
use windows::Win32::System::Performance::{
    PdhAddCounter, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue, PdhOpenQuery,
    PDH_FMT_COUNTER_VALUE, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
};

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
        let mut query = PDH_HQUERY::default();
        unsafe { PdhOpenQuery(None, 0, &mut query) }
            .map_err(|e| anyhow::anyhow!("PdhOpenQuery failed: {}", e))?;

        Ok(Self {
            query,
            counters: HashMap::new(),
        })
    }

    /// 为指定 PID 添加 GPU counter
    /// GPU counter 路径: \GPU Engine(pid_{}*)\Utilization Percentage
    pub fn add_process_counter(&mut self, pid: u32) -> anyhow::Result<()> {
        let counter_path = format!("\\GPU Engine(pid_{}*)\\Utilization Percentage", pid);
        let path_wide: Vec<u16> = counter_path.encode_utf16().chain(std::iter::once(0)).collect();

        let mut counter = PDH_HCOUNTER::default();
        unsafe { PdhAddCounter(self.query, PCWSTR(path_wide.as_ptr()), 0, &mut counter) }
            .map_err(|e| anyhow::anyhow!("PdhAddCounter failed for pid {}: {}", pid, e))?;

        self.counters.insert(pid, counter);
        Ok(())
    }

    /// 收集 GPU 数据
    pub fn collect(&mut self) -> anyhow::Result<HashMap<u32, f64>> {
        unsafe { PdhCollectQueryData(self.query) }
            .map_err(|e| anyhow::anyhow!("PdhCollectQueryData failed: {}", e))?;

        let mut results = HashMap::new();
        for (pid, counter) in &self.counters {
            let mut value = PDH_FMT_COUNTER_VALUE::default();
            unsafe { PdhGetFormattedCounterValue(*counter, PDH_FMT_DOUBLE, None, &mut value) }
                .ok();

            results.insert(*pid, value.doubleValue);
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
        self.counters.remove(&pid);
    }

    /// 清除所有 counters
    pub fn clear_counters(&mut self) {
        self.counters.clear();
    }
}

#[cfg(target_os = "windows")]
impl Drop for PdhCollector {
    fn drop(&mut self) {
        // 正确关闭 PDH query，释放资源
        unsafe { PdhCloseQuery(self.query) };
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