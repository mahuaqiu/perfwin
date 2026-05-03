// GPU 进程级采集器 - 使用 Windows PDH API
// 支持所有 GPU 类型（NVIDIA、Intel、AMD 等）

#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;
#[cfg(target_os = "windows")]
use windows::Win32::System::Performance::{
    PdhOpenQueryW, PdhAddCounterW, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhCloseQuery, PDH_FMT_DOUBLE, PDH_FMT_COUNTERVALUE_ITEM_W,
};

#[cfg(target_os = "windows")]
use crate::data::ProcessInfo;

/// GPU 进程级采集器 (Windows PDH 实现)
pub struct PdhCollector {
    query: isize,
    counter_handle: isize,
    last_gpu_data: HashMap<u32, f64>,
}

#[cfg(target_os = "windows")]
impl PdhCollector {
    pub fn new() -> anyhow::Result<Self> {
        unsafe {
            let mut query = 0isize;
            let result = PdhOpenQueryW(PCWSTR::null(), 0, &mut query);
            if result != 0 {
                return Err(anyhow::anyhow!("PdhOpenQueryW failed: {}", result));
            }

            let path = "\\GPU Engine(*)\\Utilization Percentage\0".encode_utf16().collect::<Vec<u16>>();
            let mut counter_handle = 0isize;
            let result = PdhAddCounterW(query, PCWSTR(path.as_ptr()), 0, &mut counter_handle);
            if result != 0 {
                let _ = PdhCloseQuery(query);
                return Err(anyhow::anyhow!("PdhAddCounterW failed: {}", result));
            }

            Ok(Self {
                query,
                counter_handle,
                last_gpu_data: HashMap::new(),
            })
        }
    }

    pub fn collect(&mut self) -> anyhow::Result<()> {
        unsafe {
            let result = PdhCollectQueryData(self.query);
            if result != 0 {
                return Ok(());
            }

            let mut buffer_size = 0u32;
            let mut item_count = 0u32;

            let _ = PdhGetFormattedCounterArrayW(
                self.counter_handle,
                PDH_FMT_DOUBLE,
                &mut buffer_size,
                &mut item_count,
                None,
            );

            if item_count == 0 || buffer_size == 0 {
                return Ok(());
            }

            let item_size = std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            let needed_items = (buffer_size as usize + item_size - 1) / item_size;
            let mut buffer: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> = Vec::with_capacity(needed_items);
            buffer.resize(needed_items, std::mem::zeroed());

            let result = PdhGetFormattedCounterArrayW(
                self.counter_handle,
                PDH_FMT_DOUBLE,
                &mut buffer_size,
                &mut item_count,
                Some(buffer.as_mut_ptr()),
            );

            if result != 0 {
                return Ok(());
            }

            // 不清空上一轮数据，而是累加（因为两次 collect 之间的数据需要合并）
            for i in 0..item_count as usize {
                if i >= buffer.len() {
                    break;
                }
                let item = &buffer[i];
                let instance_name = item.szName.to_string().unwrap_or_default();
                let gpu_percent = item.FmtValue.Anonymous.doubleValue;

                let pid = parse_pid(&instance_name);
                if pid > 0 && gpu_percent > 0.0 {
                    let current = self.last_gpu_data.get(&pid).copied().unwrap_or(0.0);
                    self.last_gpu_data.insert(pid, current + gpu_percent);
                }
            }
        }

        Ok(())
    }

    pub fn update_process_gpu(&mut self, processes: &mut [ProcessInfo]) -> anyhow::Result<()> {
        for proc in processes.iter_mut() {
            if let Some(gpu_percent) = self.last_gpu_data.get(&proc.pid) {
                proc.gpu_percent = *gpu_percent;
            }
        }
        // 更新后清空数据，准备下一次采集
        self.last_gpu_data.clear();
        Ok(())
    }
}

fn parse_pid(instance_name: &str) -> u32 {
    if let Some(start) = instance_name.find("pid_") {
        let remaining = &instance_name[start + 4..];
        if let Some(end) = remaining.find('_') {
            if let Ok(pid) = remaining[..end].parse::<u32>() {
                return pid;
            }
        }
    }
    0
}

#[cfg(target_os = "windows")]
impl Drop for PdhCollector {
    fn drop(&mut self) {
        if self.query != 0 {
            let _ = unsafe { PdhCloseQuery(self.query) };
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub struct PdhCollector;

#[cfg(not(target_os = "windows"))]
impl PdhCollector {
    pub fn new() -> anyhow::Result<Self> {
        anyhow::bail!("PDH collector is only available on Windows");
    }
    pub fn collect(&mut self) -> anyhow::Result<()> {
        anyhow::bail!("PDH collector is only available on Windows");
    }
    pub fn update_process_gpu(&mut self, _processes: &mut [ProcessInfo]) -> anyhow::Result<()> {
        anyhow::bail!("PDH collector is only available on Windows");
    }
}