// HWiNFO 采集器 - 系统级数据
// 从 HWiNFO 共享内存读取系统监控数据

use crate::data::{SystemInfo, CPUInfo, GPUInfo, MemoryInfo, NetworkInfo, BatteryInfo};
use serde::Deserialize;
use std::path::PathBuf;
use regex::Regex;

// ============================================================================
// Windows 平台实现
// ============================================================================

#[cfg(target_os = "windows")]
use windows::core::PCWSTR;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(target_os = "windows")]
use windows::Win32::System::Memory::{
    FILE_MAP_READ, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, MEMORY_MAPPED_VIEW_ADDRESS,
};

/// HWiNFO 共享内存名称 (需要 Global\ 前缀)
#[cfg(target_os = "windows")]
const HWINFO_SHARED_MEM_NAME: &str = "Global\\HWiNFO_SENS_SM2";

/// HWiNFO 魔数，用于验证共享内存有效性
#[cfg(target_os = "windows")]
const HWINFO_HEADER_MAGIC: u32 = 0x53695748;

/// 传感器类型枚举
#[cfg(target_os = "windows")]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorType {
    None = 0,
    Temp = 1,      // 温度
    Volt = 2,      // 电压
    Fan = 3,       // 风扇
    Current = 4,   // 电流
    Power = 5,     // 功率
    Clock = 6,     // 时钟频率
    Usage = 7,     // 使用率
    Other = 8,
}

/// HWiNFO 共享内存头部结构
#[cfg(target_os = "windows")]
#[repr(C, packed)]
struct HWiNFOHeader {
    magic: u32,
    version: u32,
    version2: u32,
    last_update: i64,
    sensor_section_offset: u32,
    sensor_element_size: u32,
    sensor_element_count: u32,
    entry_section_offset: u32,
    entry_element_size: u32,
    entry_element_count: u32,
}

/// HWiNFO 数据条目结构
#[cfg(target_os = "windows")]
#[repr(C, packed)]
struct HWiNFOEntry {
    sensor_type: u32,
    sensor_index: u32,
    id: u32,
    name_original: [u8; 128],
    name_user: [u8; 128],
    unit: [u8; 16],
    value_bytes: [u8; 8],
    value_min_bytes: [u8; 8],
    value_max_bytes: [u8; 8],
    value_avg_bytes: [u8; 8],
}

/// 解析固定长度字符串字段
#[cfg(target_os = "windows")]
fn parse_fixed_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// 从字节数组读取 f64 值
#[cfg(target_os = "windows")]
fn read_f64_from_bytes(bytes: &[u8; 8]) -> f64 {
    f64::from_bits(u64::from_ne_bytes(*bytes))
}

/// 传感器条目数据
#[derive(Debug, Clone)]
pub struct SensorEntry {
    pub sensor_type: SensorType,
    pub sensor_index: u32,
    pub id: u32,
    pub name_original: String,
    pub name_user: String,
    pub unit: String,
    pub value: f64,
    pub value_min: f64,
    pub value_max: f64,
    pub value_avg: f64,
}

impl SensorEntry {
    /// 获取显示名称（优先用户自定义名称）
    pub fn label(&self) -> &str {
        if !self.name_user.is_empty() {
            &self.name_user
        } else {
            &self.name_original
        }
    }
}

#[cfg(target_os = "windows")]
impl TryFrom<u32> for SensorType {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(SensorType::None),
            1 => Ok(SensorType::Temp),
            2 => Ok(SensorType::Volt),
            3 => Ok(SensorType::Fan),
            4 => Ok(SensorType::Current),
            5 => Ok(SensorType::Power),
            6 => Ok(SensorType::Clock),
            7 => Ok(SensorType::Usage),
            8 => Ok(SensorType::Other),
            _ => Err(()),
        }
    }
}

// ============================================================================
// 传感器配置
// ============================================================================

/// HWiNFO 传感器映射配置
#[derive(Debug, Clone, Deserialize, Default)]
pub struct HWiNFOConfig {
    pub cpu: CpuConfig,
    pub gpu: GpuConfig,
    pub system: SystemConfig,
    pub network: NetworkConfig,
    pub battery: BatteryConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CpuConfig {
    pub usage_name: String,
    pub usage_unit: String,
    pub temperature_name: String,
    pub temperature_unit: String,
    pub power_name: Option<String>,
    pub power_unit: Option<String>,
    pub clock_pattern: Option<String>,
    pub clock_unit: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GpuConfig {
    pub usage_name: String,
    pub usage_unit: String,
    pub temperature_name: String,
    pub temperature_unit: String,
    pub power_name: Option<String>,
    pub power_unit: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SystemConfig {
    pub power_name: Option<String>,
    pub power_unit: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct NetworkConfig {
    pub download_name: Option<String>,
    pub download_unit: Option<String>,
    pub upload_name: Option<String>,
    pub upload_unit: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BatteryConfig {
    pub charge_name: Option<String>,
    pub charge_unit: Option<String>,
}

impl HWiNFOConfig {
    /// 从文件加载配置
    pub fn load(path: &PathBuf) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: HWiNFOConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// 搜索配置文件路径
    pub fn find_config_path() -> Option<PathBuf> {
        let possible_paths: Vec<PathBuf> = vec![
            // 当前工作目录
            PathBuf::from("hwinfo_sensors.toml"),
            // 模块目录（打包后）
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("hwinfo_sensors.toml")))
                .unwrap_or_else(|| PathBuf::new()),
            // 项目根目录（开发模式）
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().and_then(|d| d.parent().map(|dd| dd.join("hwinfo_sensors.toml"))))
                .unwrap_or_else(|| PathBuf::new()),
        ];

        for path in possible_paths.iter() {
            if !path.as_os_str().is_empty() && path.exists() {
                return Some(path.clone());
            }
        }

        None
    }

    /// 获取默认配置（硬编码备份）
    pub fn default_config() -> Self {
        Self {
            cpu: CpuConfig {
                usage_name: "Total CPU Usage".to_string(),
                usage_unit: "%".to_string(),
                temperature_name: "CPU Package".to_string(),
                temperature_unit: "C".to_string(),
                power_name: Some("CPU Package Power".to_string()),
                power_unit: Some("W".to_string()),
                clock_pattern: Some("(Core [0-9]+ Clock|P-core [0-9]+ Clock|E-core [0-9]+ Clock)".to_string()),
                clock_unit: Some("MHz".to_string()),
            },
            gpu: GpuConfig {
                usage_name: "GPU D3D Usage".to_string(),
                usage_unit: "%".to_string(),
                temperature_name: "GPU Temperature".to_string(),
                temperature_unit: "C".to_string(),
                power_name: Some("GPU Power".to_string()),
                power_unit: Some("W".to_string()),
            },
            system: SystemConfig {
                power_name: Some("Total System Power".to_string()),
                power_unit: Some("W".to_string()),
            },
            network: NetworkConfig {
                download_name: Some("Current DL rate".to_string()),
                download_unit: Some("KB/s".to_string()),
                upload_name: Some("Current UP rate".to_string()),
                upload_unit: Some("KB/s".to_string()),
            },
            battery: BatteryConfig {
                charge_name: Some("Charge Level".to_string()),
                charge_unit: Some("%".to_string()),
            },
        }
    }
}

// ============================================================================
// HWiNFO 采集器
// ============================================================================

/// HWiNFO 共享内存采集器
#[cfg(target_os = "windows")]
pub struct HWiNFOCollector {
    handle: HANDLE,
    mapped_ptr: MEMORY_MAPPED_VIEW_ADDRESS,
    header: *const HWiNFOHeader,
    config: HWiNFOConfig,
}

#[cfg(target_os = "windows")]
impl std::fmt::Debug for HWiNFOCollector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HWiNFOCollector")
            .field("handle", &self.handle.0)
            .field("mapped_ptr", &self.mapped_ptr.Value)
            .field("header", &self.header)
            .finish()
    }
}

#[cfg(target_os = "windows")]
impl HWiNFOCollector {
    /// 创建新的 HWiNFO 采集器
    pub fn new() -> anyhow::Result<Self> {
        // 加载配置
        let config = HWiNFOConfig::find_config_path()
            .and_then(|p| HWiNFOConfig::load(&p).ok())
            .unwrap_or_else(HWiNFOConfig::default_config);

        // 打开共享内存
        let name_wide: Vec<u16> = HWINFO_SHARED_MEM_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe { OpenFileMappingW(FILE_MAP_READ.0, false, PCWSTR(name_wide.as_ptr())) }
            .map_err(|e| anyhow::anyhow!(
                "无法打开 HWiNFO 共享内存: {}。请确保 HWiNFO 正在运行并启用了共享内存功能。",
                e
            ))?;

        if handle.is_invalid() {
            return Err(anyhow::anyhow!(
                "HWiNFO 共享内存未找到。请确保 HWiNFO 正在运行并启用了共享内存功能。"
            ));
        }

        // 映射共享内存
        let mapped_ptr = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0) };

        if mapped_ptr.Value.is_null() {
            unsafe { let _ = CloseHandle(handle); };
            return Err(anyhow::anyhow!("无法映射 HWiNFO 共享内存"));
        }

        // 验证魔数
        let header = unsafe { &*(mapped_ptr.Value as *const HWiNFOHeader) };
        let magic = header.magic;

        if magic != HWINFO_HEADER_MAGIC {
            unsafe {
                let _ = UnmapViewOfFile(mapped_ptr);
                let _ = CloseHandle(handle);
            };
            return Err(anyhow::anyhow!(
                "HWiNFO 共享内存魔数不匹配: 0x{:08X} (期望 0x{:08X})。",
                magic, HWINFO_HEADER_MAGIC
            ));
        }

        Ok(Self {
            handle,
            mapped_ptr,
            header,
            config,
        })
    }

    /// 检查共享内存是否有效
    pub fn is_valid(&self) -> bool {
        !self.handle.is_invalid() && !self.mapped_ptr.Value.is_null()
    }

    /// 遍历所有传感器条目
    pub fn iter_entries(&self) -> impl Iterator<Item = SensorEntry> + '_ {
        if !self.is_valid() {
            return Vec::new().into_iter();
        }

        let header = unsafe { &*self.header };
        let entry_base = unsafe {
            (self.mapped_ptr.Value as *const u8).add(header.entry_section_offset as usize)
        };

        let mut entries = Vec::with_capacity(header.entry_element_count as usize);

        for i in 0..header.entry_element_count as usize {
            let entry_addr = unsafe {
                entry_base.add(i * header.entry_element_size as usize)
            };
            let entry = unsafe { &*(entry_addr as *const HWiNFOEntry) };

            if entry.sensor_type == SensorType::None as u32 {
                continue;
            }

            entries.push(SensorEntry {
                sensor_type: SensorType::try_from(entry.sensor_type).unwrap_or(SensorType::Other),
                sensor_index: entry.sensor_index,
                id: entry.id,
                name_original: parse_fixed_string(&entry.name_original),
                name_user: parse_fixed_string(&entry.name_user),
                unit: parse_fixed_string(&entry.unit),
                value: read_f64_from_bytes(&entry.value_bytes),
                value_min: read_f64_from_bytes(&entry.value_min_bytes),
                value_max: read_f64_from_bytes(&entry.value_max_bytes),
                value_avg: read_f64_from_bytes(&entry.value_avg_bytes),
            });
        }

        entries.into_iter()
    }

    /// 按名称精确匹配传感器值
    /// 找不到时返回 0.0
    fn find_by_name(&self, target_name: &str, target_unit: &str) -> f64 {
        let target_name_lower = target_name.to_lowercase();
        let target_unit_lower = target_unit.to_lowercase();

        for entry in self.iter_entries() {
            let name_lower = entry.label().to_lowercase();
            let unit_lower = entry.unit.to_lowercase();

            // 名称包含目标名称，且单位包含目标单位
            if name_lower.contains(&target_name_lower) && unit_lower.contains(&target_unit_lower) {
                let value = entry.value;
                // 如果单位是 KB/s，转换为 B/s
                if unit_lower.contains("kb/s") || unit_lower.contains("kb") {
                    return value * 1024.0;
                }
                return value;
            }
        }

        0.0  // 找不到返回 0
    }

    /// 按名称精确匹配传感器值（可选）
    /// 找不到时返回 None
    fn find_by_name_opt(&self, target_name: &str, target_unit: &str) -> Option<f64> {
        let target_name_lower = target_name.to_lowercase();
        let target_unit_lower = target_unit.to_lowercase();

        for entry in self.iter_entries() {
            let name_lower = entry.label().to_lowercase();
            let unit_lower = entry.unit.to_lowercase();

            if name_lower.contains(&target_name_lower) && unit_lower.contains(&target_unit_lower) {
                let value = entry.value;
                // 如果单位是 KB/s，转换为 B/s
                if unit_lower.contains("kb/s") || unit_lower.contains("kb") {
                    return Some(value * 1024.0);
                }
                return Some(value);
            }
        }

        None
    }

    /// 按正则匹配传感器值并求平均（可选）
    /// 用于匹配多个核心时钟频率，如 "Core 0 Clock", "Core 1 Clock" 等
    /// 返回所有匹配项的平均值
    fn find_by_pattern_avg(&self, pattern: &str, target_unit: &str) -> Option<f64> {
        let re = Regex::new(pattern).ok()?;
        let target_unit_lower = target_unit.to_lowercase();

        let mut sum = 0.0;
        let mut count = 0;

        for entry in self.iter_entries() {
            let name = entry.label();
            let unit_lower = entry.unit.to_lowercase();

            if re.is_match(name) && unit_lower.contains(&target_unit_lower) {
                sum += entry.value;
                count += 1;
            }
        }

        if count > 0 {
            Some(sum / count as f64)
        } else {
            None
        }
    }

    /// 获取系统信息
    pub fn get_system_info(&self) -> anyhow::Result<SystemInfo> {
        if !self.is_valid() {
            return Err(anyhow::anyhow!("HWiNFO 共享内存无效"));
        }

        let config = &self.config;

        Ok(SystemInfo {
            cpu: CPUInfo {
                percent: self.find_by_name(&config.cpu.usage_name, &config.cpu.usage_unit),
                temperature: self.find_by_name_opt(&config.cpu.temperature_name, &config.cpu.temperature_unit),
                power: config.cpu.power_name.as_ref()
                    .zip(config.cpu.power_unit.as_ref())
                    .and_then(|(n, u)| self.find_by_name_opt(n, u)),
                clock_speed: config.cpu.clock_pattern.as_ref()
                    .zip(config.cpu.clock_unit.as_ref())
                    .and_then(|(p, u)| {
                        self.find_by_pattern_avg(p, u).map(|v| {
                            // MHz 转 GHz
                            if u.to_lowercase().contains("mhz") {
                                v / 1000.0
                            } else {
                                v
                            }
                        })
                    }),
            },
            gpu: GPUInfo {
                percent: self.find_by_name(&config.gpu.usage_name, &config.gpu.usage_unit),
                temperature: self.find_by_name_opt(&config.gpu.temperature_name, &config.gpu.temperature_unit),
                power: config.gpu.power_name.as_ref()
                    .zip(config.gpu.power_unit.as_ref())
                    .and_then(|(n, u)| self.find_by_name_opt(n, u)),
                memory_mb: None,
            },
            memory: MemoryInfo::default(),
            network: NetworkInfo {
                download_speed: config.network.download_name.as_ref()
                    .zip(config.network.download_unit.as_ref())
                    .map(|(n, u)| self.find_by_name(n, u))
                    .unwrap_or(0.0),
                upload_speed: config.network.upload_name.as_ref()
                    .zip(config.network.upload_unit.as_ref())
                    .map(|(n, u)| self.find_by_name(n, u))
                    .unwrap_or(0.0),
            },
            battery: BatteryInfo {
                charge_level: config.battery.charge_name.as_ref()
                    .zip(config.battery.charge_unit.as_ref())
                    .map(|(n, u)| self.find_by_name(n, u))
                    .unwrap_or(0.0),
            },
            // system_power: 优先 Total System Power，fallback 到 CPU Package Power
            system_power: {
                let total_power = config.system.power_name.as_ref()
                    .zip(config.system.power_unit.as_ref())
                    .map(|(n, u)| self.find_by_name(n, u))
                    .unwrap_or(0.0);

                // 如果 Total System Power 找不到（返回 0），fallback 到 CPU Package Power
                if total_power > 0.0 {
                    total_power
                } else {
                    config.cpu.power_name.as_ref()
                        .zip(config.cpu.power_unit.as_ref())
                        .map(|(n, u)| self.find_by_name(n, u))
                        .unwrap_or(0.0)
                }
            },
        })
    }
}

#[cfg(target_os = "windows")]
impl Drop for HWiNFOCollector {
    fn drop(&mut self) {
        if !self.mapped_ptr.Value.is_null() {
            unsafe { let _ = UnmapViewOfFile(self.mapped_ptr); }
        }
        if !self.handle.is_invalid() {
            unsafe { let _ = CloseHandle(self.handle); }
        }
    }
}

// ============================================================================
// 非 Windows 平台占位实现
// ============================================================================

#[cfg(not(target_os = "windows"))]
pub struct HWiNFOCollector { _private: () }

#[cfg(not(target_os = "windows"))]
impl HWiNFOCollector {
    pub fn new() -> anyhow::Result<Self> {
        Err(anyhow::anyhow!("HWiNFO 仅在 Windows 平台上可用"))
    }
    pub fn is_valid(&self) -> bool { false }
    pub fn get_system_info(&self) -> anyhow::Result<SystemInfo> {
        Err(anyhow::anyhow!("HWiNFO 仅在 Windows 平台上可用"))
    }
}

#[cfg(not(target_os = "windows"))]
pub struct SensorEntry {
    pub sensor_type: u32,
    pub sensor_index: u32,
    pub id: u32,
    pub name_original: String,
    pub name_user: String,
    pub unit: String,
    pub value: f64,
    pub value_min: f64,
    pub value_max: f64,
    pub value_avg: f64,
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "windows")]
    fn test_header_size() {
        assert_eq!(std::mem::size_of::<HWiNFOHeader>(), 44);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_entry_size() {
        let size = std::mem::size_of::<HWiNFOEntry>();
        assert!(size >= 312 && size <= 320, "Entry size should be around 312 bytes, got {}", size);
    }

    #[test]
    fn test_config_default() {
        let config = HWiNFOConfig::default_config();
        assert_eq!(config.cpu.usage_name, "Total CPU Usage");
        assert_eq!(config.gpu.usage_name, "GPU D3D Usage");
        assert_eq!(config.battery.charge_name, Some("Charge Level".to_string()));
    }

    #[test]
    fn test_sensor_entry_label() {
        let entry = SensorEntry {
            sensor_type: SensorType::Temp,
            sensor_index: 0,
            id: 1,
            name_original: "Original".to_string(),
            name_user: "User".to_string(),
            unit: "C".to_string(),
            value: 45.0,
            value_min: 30.0,
            value_max: 80.0,
            value_avg: 50.0,
        };
        assert_eq!(entry.label(), "User");
    }
}