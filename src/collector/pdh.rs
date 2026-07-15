// GPU 采集器：使用 Windows PDH GPU Engine 计数器，支持核显、独显和混合显卡。

#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
#[cfg(target_os = "windows")]
use windows::Win32::System::Performance::{
    PdhAddCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE,
};

use crate::data::{GpuAdapterMetrics, ProcessInfo, SystemMetrics};

#[cfg(target_os = "windows")]
pub struct PdhCollector {
    query: isize,
    counter_handle: isize,
    process_engine_data: HashMap<(u32, String), f64>,
    adapter_engine_data: HashMap<(String, String), f64>,
    gpu_source: String,
    adapter_names: HashMap<String, String>,
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
            let path = "\\GPU Engine(*)\\Utilization Percentage\0"
                .encode_utf16()
                .collect::<Vec<u16>>();
            let mut counter_handle = 0isize;
            let result = PdhAddCounterW(query, PCWSTR(path.as_ptr()), 0, &mut counter_handle);
            if result != 0 {
                let _ = PdhCloseQuery(query);
                return Err(anyhow::anyhow!("PdhAddCounterW failed: {}", result));
            }
            Ok(Self {
                query,
                counter_handle,
                process_engine_data: HashMap::new(),
                adapter_engine_data: HashMap::new(),
                gpu_source: String::from("rust_pdh"),
                adapter_names: enumerate_adapter_names(),
            })
        }
    }

    /// 读取一轮快照。PDH 正常但没有活动实例时表示有效空闲，而不是失败。
    pub fn collect(&mut self) -> anyhow::Result<()> {
        unsafe {
            let result = PdhCollectQueryData(self.query);
            if result != 0 {
                return Err(anyhow::anyhow!("PdhCollectQueryData failed: {}", result));
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
            self.process_engine_data.clear();
            self.adapter_engine_data.clear();
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
                return Err(anyhow::anyhow!("PdhGetFormattedCounterArrayW failed: {}", result));
            }
            for item in buffer.iter().take(item_count as usize) {
                let instance_name = item.szName.to_string().unwrap_or_default();
                let value = item.FmtValue.Anonymous.doubleValue;
                if !value.is_finite() || value < 0.0 {
                    continue;
                }
                let Some(luid) = parse_luid(&instance_name) else { continue };
                let engine = parse_engine(&instance_name);
                if let Some(pid) = parse_pid(&instance_name) {
                    add_value(&mut self.process_engine_data, (pid, engine.clone()), value);
                }
                add_value(&mut self.adapter_engine_data, (luid, engine), value);
            }
        }
        Ok(())
    }

    pub fn system_metrics(&self) -> SystemMetrics {
        let mut adapters: HashMap<String, f64> = HashMap::new();
        for ((luid, _engine), value) in &self.adapter_engine_data {
            let entry = adapters.entry(luid.clone()).or_insert(0.0);
            *entry = entry.max((*value).clamp(0.0, 100.0));
        }
        let gpu_percent = adapters.values().copied().fold(0.0, f64::max).clamp(0.0, 100.0);
        let gpu_adapters = adapters.into_iter().map(|(luid, utilization)| GpuAdapterMetrics {
            name: self.adapter_names.get(&luid).cloned().unwrap_or_else(|| format!("GPU {}", luid)),
            luid,
            utilization_percent: utilization,
        }).collect();
        SystemMetrics { cpu_percent: Some(0.0), gpu_percent: Some(gpu_percent), gpu_adapters, gpu_source: self.gpu_source.clone() }
    }

    pub fn update_process_gpu(&mut self, processes: &mut [ProcessInfo]) -> anyhow::Result<()> {
        let mut process_values: HashMap<u32, f64> = HashMap::new();
        for ((pid, _engine), value) in &self.process_engine_data {
            let entry = process_values.entry(*pid).or_insert(0.0);
            *entry = entry.max((*value).clamp(0.0, 100.0));
        }
        for process in processes.iter_mut() {
            process.gpu_percent = process_values.get(&process.pid).copied().unwrap_or(0.0);
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn add_value<K: std::cmp::Eq + std::hash::Hash>(map: &mut HashMap<K, f64>, key: K, value: f64) {
    let entry = map.entry(key).or_insert(0.0);
    *entry = (*entry + value).clamp(0.0, 100.0);
}

#[cfg(target_os = "windows")]
fn parse_pid(instance_name: &str) -> Option<u32> {
    let start = instance_name.find("pid_")? + 4;
    let remaining = &instance_name[start..];
    let end = remaining.find('_')?;
    remaining[..end].parse().ok()
}

#[cfg(target_os = "windows")]
fn parse_luid(instance_name: &str) -> Option<String> {
    let start = instance_name.find("luid_")? + 5;
    let remaining = &instance_name[start..];
    let end = remaining.find("_phys_").or_else(|| remaining.find("_eng_")).unwrap_or(remaining.len());
    let luid = remaining[..end].trim_matches('_').trim_start_matches("0x");
    u64::from_str_radix(luid, 16).ok().map(|value| format!("0x{:x}", value))
}

#[cfg(target_os = "windows")]
fn parse_engine(instance_name: &str) -> String {
    instance_name.find("_engtype_")
        .map(|index| instance_name[index + 9..].split('_').next().unwrap_or("unknown").to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| String::from("unknown"))
}

#[cfg(target_os = "windows")]
fn enumerate_adapter_names() -> HashMap<String, String> {
    let mut names = HashMap::new();
    unsafe {
        let Ok(factory): windows::core::Result<IDXGIFactory1> = CreateDXGIFactory1() else { return names };
        let mut index = 0;
        loop {
            let Ok(adapter) = factory.EnumAdapters1(index) else { break };
            if let Ok(desc) = adapter.GetDesc1() {
                let luid = ((desc.AdapterLuid.HighPart as u32 as u64) << 32) | desc.AdapterLuid.LowPart as u64;
                let name = String::from_utf16_lossy(&desc.Description).trim_end_matches('\0').to_string();
                if !name.is_empty() { names.insert(format!("0x{:x}", luid), name); }
            }
            index += 1;
        }
    }
    names
}

#[cfg(target_os = "windows")]
impl Drop for PdhCollector {
    fn drop(&mut self) { if self.query != 0 { let _ = unsafe { PdhCloseQuery(self.query) }; } }
}

#[cfg(not(target_os = "windows"))]
pub struct PdhCollector;
#[cfg(not(target_os = "windows"))]
impl PdhCollector {
    pub fn new() -> anyhow::Result<Self> { anyhow::bail!("PDH collector is only available on Windows") }
    pub fn collect(&mut self) -> anyhow::Result<()> { anyhow::bail!("PDH collector is only available on Windows") }
    pub fn system_metrics(&self) -> SystemMetrics { SystemMetrics::default() }
    pub fn update_process_gpu(&mut self, _processes: &mut [ProcessInfo]) -> anyhow::Result<()> { anyhow::bail!("PDH collector is only available on Windows") }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::{parse_engine, parse_luid, parse_pid};
    #[test]
    fn parses_gpu_engine_instance() {
        let value = "pid_1234_luid_0x000000000000abcd_phys_0_eng_0_engtype_3D";
        assert_eq!(parse_pid(value), Some(1234));
        assert_eq!(parse_luid(value), Some("0xabcd".to_string()));
        assert_eq!(parse_engine(value), "3d");
    }

    #[test]
    fn parses_gpu_instance_without_pid() {
        let value = "luid_0x000000000000abcd_phys_0_eng_1_engtype_VideoDecode";
        assert_eq!(parse_pid(value), None);
        assert_eq!(parse_luid(value), Some("0xabcd".to_string()));
        assert_eq!(parse_engine(value), "videodecode");
    }
}
