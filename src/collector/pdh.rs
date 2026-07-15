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

/// 最近一次 PDH 采集诊断信息，便于定位“为何回落到 HWiNFO”。
#[derive(Debug, Clone, Default)]
pub struct PdhCollectDiagnostics {
    pub raw_item_count: u32,
    pub parsed_item_count: u32,
    pub parse_fail_count: u32,
    pub process_engine_count: usize,
    pub adapter_engine_count: usize,
    pub sample_unparsed: Vec<String>,
}

#[cfg(target_os = "windows")]
pub struct PdhCollector {
    query: isize,
    counter_handle: isize,
    process_engine_data: HashMap<(u32, String), f64>,
    adapter_engine_data: HashMap<(String, String), f64>,
    gpu_source: String,
    adapter_names: HashMap<String, String>,
    last_diagnostics: PdhCollectDiagnostics,
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
            let adapter_names = enumerate_adapter_names();
            log::info!(
                "PDH GPU collector ready, DXGI adapters={}",
                adapter_names.len()
            );
            for (luid, name) in &adapter_names {
                log::info!("DXGI adapter: luid={} name={}", luid, name);
            }
            Ok(Self {
                query,
                counter_handle,
                process_engine_data: HashMap::new(),
                adapter_engine_data: HashMap::new(),
                gpu_source: String::from("rust_pdh"),
                adapter_names,
                last_diagnostics: PdhCollectDiagnostics::default(),
            })
        }
    }

    pub fn last_diagnostics(&self) -> &PdhCollectDiagnostics {
        &self.last_diagnostics
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
            let first = PdhGetFormattedCounterArrayW(
                self.counter_handle,
                PDH_FMT_DOUBLE,
                &mut buffer_size,
                &mut item_count,
                None,
            );
            self.process_engine_data.clear();
            self.adapter_engine_data.clear();
            self.last_diagnostics = PdhCollectDiagnostics::default();

            // 无数据：真·空闲，或计数器尚无实例
            if buffer_size == 0 {
                log::debug!(
                    "PDH GPU array empty (status={}, item_count={})",
                    first,
                    item_count
                );
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
                return Err(anyhow::anyhow!(
                    "PdhGetFormattedCounterArrayW failed: {} (buffer_size={}, item_count={})",
                    result,
                    buffer_size,
                    item_count
                ));
            }

            let mut parsed = 0u32;
            let mut failed = 0u32;
            let mut sample_unparsed: Vec<String> = Vec::new();
            for item in buffer.iter().take(item_count as usize) {
                let instance_name = item.szName.to_string().unwrap_or_default();
                let value = item.FmtValue.Anonymous.doubleValue;
                if !value.is_finite() || value < 0.0 {
                    failed += 1;
                    if sample_unparsed.len() < 3 {
                        sample_unparsed.push(format!("{}(bad_value={})", instance_name, value));
                    }
                    continue;
                }
                let Some(luid) = parse_luid(&instance_name) else {
                    failed += 1;
                    if sample_unparsed.len() < 3 {
                        sample_unparsed.push(instance_name);
                    }
                    continue;
                };
                let engine = parse_engine(&instance_name);
                if let Some(pid) = parse_pid(&instance_name) {
                    add_value(&mut self.process_engine_data, (pid, engine.clone()), value);
                }
                add_value(&mut self.adapter_engine_data, (luid, engine), value);
                parsed += 1;
            }

            self.last_diagnostics = PdhCollectDiagnostics {
                raw_item_count: item_count,
                parsed_item_count: parsed,
                parse_fail_count: failed,
                process_engine_count: self.process_engine_data.len(),
                adapter_engine_count: self.adapter_engine_data.len(),
                sample_unparsed,
            };

            if item_count > 0 && parsed == 0 {
                log::error!(
                    "PDH GPU 有 {} 个实例但全部解析失败，样本: {:?}",
                    item_count,
                    self.last_diagnostics.sample_unparsed
                );
            } else if failed > 0 {
                log::warn!(
                    "PDH GPU 解析部分失败: raw={}, parsed={}, failed={}, samples={:?}",
                    item_count,
                    parsed,
                    failed,
                    self.last_diagnostics.sample_unparsed
                );
            } else {
                log::debug!(
                    "PDH GPU 采集: raw={}, parsed={}, adapters={}, processes={}",
                    item_count,
                    parsed,
                    self.adapter_engine_data.len(),
                    self.process_engine_data.len()
                );
            }
        }
        Ok(())
    }

    pub fn system_metrics(&self) -> SystemMetrics {
        // 任务管理器整体 GPU：各引擎利用率的最大值（按适配器再取 max）
        let mut adapters: HashMap<String, f64> = HashMap::new();
        for ((luid, _engine), value) in &self.adapter_engine_data {
            let entry = adapters.entry(luid.clone()).or_insert(0.0);
            *entry = entry.max((*value).clamp(0.0, 100.0));
        }
        let gpu_percent = adapters
            .values()
            .copied()
            .fold(0.0, f64::max)
            .clamp(0.0, 100.0);
        let gpu_adapters = adapters
            .into_iter()
            .map(|(luid, utilization)| GpuAdapterMetrics {
                name: self
                    .adapter_names
                    .get(&luid)
                    .cloned()
                    .unwrap_or_else(|| format!("GPU {}", luid)),
                luid,
                utilization_percent: utilization,
            })
            .collect();
        SystemMetrics {
            cpu_percent: Some(0.0),
            gpu_percent: Some(gpu_percent),
            gpu_adapters,
            gpu_source: self.gpu_source.clone(),
        }
    }

    /// 单个 PID 的 GPU：该进程各引擎利用率之和（接近任务管理器进程列），限制到 0-100。
    pub fn gpu_for_pid(&self, pid: u32) -> f64 {
        self.process_engine_data
            .iter()
            .filter(|((p, _), _)| *p == pid)
            .map(|((_, _), value)| (*value).clamp(0.0, 100.0))
            .sum::<f64>()
            .clamp(0.0, 100.0)
    }

    /// 多个 PID（同名多实例/目标进程组）的 GPU，接近任务管理器语义：
    /// 1) 同一引擎上对各 PID 利用率求和
    /// 2) 再对所有引擎取 max
    /// 这样不会把“每进程先 max 再 sum”抬到明显高于系统整体。
    pub fn gpu_for_pids(&self, pids: &[u32]) -> f64 {
        if pids.is_empty() {
            return 0.0;
        }
        let pid_set: std::collections::HashSet<u32> = pids.iter().copied().collect();
        let mut by_engine: HashMap<String, f64> = HashMap::new();
        for ((pid, engine), value) in &self.process_engine_data {
            if !pid_set.contains(pid) {
                continue;
            }
            let entry = by_engine.entry(engine.clone()).or_insert(0.0);
            *entry = (*entry + (*value).clamp(0.0, 100.0)).clamp(0.0, 100.0);
        }
        by_engine
            .values()
            .copied()
            .fold(0.0, f64::max)
            .clamp(0.0, 100.0)
    }

    pub fn update_process_gpu(&mut self, processes: &mut [ProcessInfo]) -> anyhow::Result<()> {
        for process in processes.iter_mut() {
            process.gpu_percent = self.gpu_for_pid(process.pid);
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
fn parse_hex_u32(value: &str) -> Option<u32> {
    let hex = value
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    u32::from_str_radix(hex, 16).ok()
}

#[cfg(target_os = "windows")]
fn parse_hex_u64(value: &str) -> Option<u64> {
    let hex = value
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    u64::from_str_radix(hex, 16).ok()
}

#[cfg(target_os = "windows")]
fn parse_pid(instance_name: &str) -> Option<u32> {
    let start = instance_name.find("pid_")? + 4;
    let remaining = &instance_name[start..];
    let end = remaining.find('_')?;
    remaining[..end].parse().ok()
}

/// 解析 PDH GPU Engine 实例中的 LUID。
///
/// 真实 Windows 实例名常见格式：
/// `pid_1234_luid_0x00000000_0x0000ABCD_phys_0_eng_0_engtype_3D`
/// 即 high/low 两段 32-bit，而不是单个 64-bit hex。
#[cfg(target_os = "windows")]
fn parse_luid(instance_name: &str) -> Option<String> {
    let start = instance_name.find("luid_")? + 5;
    let remaining = &instance_name[start..];
    let end = remaining
        .find("_phys_")
        .or_else(|| remaining.find("_eng_"))
        .unwrap_or(remaining.len());
    let luid_raw = remaining[..end].trim_matches('_');
    if luid_raw.is_empty() {
        return None;
    }

    // 优先按 high_low 两段解析：0xHIGH_0xLOW
    let parts: Vec<&str> = luid_raw
        .split('_')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() >= 2 {
        let high = parse_hex_u32(parts[0])?;
        let low = parse_hex_u32(parts[1])?;
        let value = ((high as u64) << 32) | (low as u64);
        return Some(format!("0x{:x}", value));
    }

    // 兼容测试/旧格式：单个 64-bit hex
    if parts.len() == 1 {
        return parse_hex_u64(parts[0]).map(|value| format!("0x{:x}", value));
    }
    None
}

#[cfg(target_os = "windows")]
fn parse_engine(instance_name: &str) -> String {
    instance_name
        .find("_engtype_")
        .map(|index| {
            instance_name[index + 9..]
                .split('_')
                .next()
                .unwrap_or("unknown")
                .to_ascii_lowercase()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| String::from("unknown"))
}

#[cfg(target_os = "windows")]
fn enumerate_adapter_names() -> HashMap<String, String> {
    let mut names = HashMap::new();
    unsafe {
        let Ok(factory): windows::core::Result<IDXGIFactory1> = CreateDXGIFactory1() else {
            return names;
        };
        let mut index = 0;
        loop {
            let Ok(adapter) = factory.EnumAdapters1(index) else {
                break;
            };
            if let Ok(desc) = adapter.GetDesc1() {
                // 与 PDH high/low 组合方式保持一致
                let luid = ((desc.AdapterLuid.HighPart as u32 as u64) << 32)
                    | (desc.AdapterLuid.LowPart as u64);
                let name = String::from_utf16_lossy(&desc.Description)
                    .trim_end_matches('\0')
                    .to_string();
                if !name.is_empty() {
                    names.insert(format!("0x{:x}", luid), name);
                }
            }
            index += 1;
        }
    }
    names
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
        anyhow::bail!("PDH collector is only available on Windows")
    }
    pub fn collect(&mut self) -> anyhow::Result<()> {
        anyhow::bail!("PDH collector is only available on Windows")
    }
    pub fn system_metrics(&self) -> SystemMetrics { SystemMetrics::default() }
    pub fn gpu_for_pid(&self, _pid: u32) -> f64 { 0.0 }
    pub fn gpu_for_pids(&self, _pids: &[u32]) -> f64 { 0.0 }
    pub fn update_process_gpu(&mut self, _processes: &mut [ProcessInfo]) -> anyhow::Result<()> { anyhow::bail!("PDH collector is only available on Windows") }
    pub fn last_diagnostics(&self) -> PdhCollectDiagnostics {
        PdhCollectDiagnostics::default()
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::{parse_engine, parse_luid, parse_pid};

    #[test]
    fn parses_gpu_engine_instance_two_part_luid() {
        // 真实 Windows PDH 实例名：high/low 两段
        let value = "pid_1234_luid_0x00000000_0x0000abcd_phys_0_eng_0_engtype_3D";
        assert_eq!(parse_pid(value), Some(1234));
        assert_eq!(parse_luid(value), Some("0xabcd".to_string()));
        assert_eq!(parse_engine(value), "3d");
    }

    #[test]
    fn parses_gpu_engine_instance_nonzero_high() {
        let value = "pid_2188_luid_0x00000001_0x0001193e_phys_0_eng_0_engtype_3D";
        assert_eq!(parse_pid(value), Some(2188));
        // high=1, low=0x1193e => 0x10001193e
        assert_eq!(parse_luid(value), Some("0x10001193e".to_string()));
        assert_eq!(parse_engine(value), "3d");
    }

    #[test]
    fn parses_gpu_instance_without_pid() {
        let value = "luid_0x00000000_0x0000abcd_phys_0_eng_1_engtype_VideoDecode";
        assert_eq!(parse_pid(value), None);
        assert_eq!(parse_luid(value), Some("0xabcd".to_string()));
        assert_eq!(parse_engine(value), "videodecode");
    }

    #[test]
    fn parses_legacy_single_hex_luid() {
        // 兼容旧测试/简化格式
        let value = "pid_1234_luid_0x000000000000abcd_phys_0_eng_0_engtype_3D";
        assert_eq!(parse_luid(value), Some("0xabcd".to_string()));
    }
}
