// HWiNFO 采集器 - 系统级数据
// 从 HWiNFO 共享内存读取系统监控数据

use std::collections::HashMap;
use crate::data::SensorValue;

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
// HWiNFO 采集器
// ============================================================================

/// HWiNFO 共享内存采集器
#[cfg(target_os = "windows")]
pub struct HWiNFOCollector {
    handle: HANDLE,
    mapped_ptr: MEMORY_MAPPED_VIEW_ADDRESS,
    header: *const HWiNFOHeader,
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

    /// 获取所有传感器数据（按原始名称索引，同名传感器自动编号）
    pub fn get_all_entries(&self) -> HashMap<String, SensorValue> {
        let mut name_counter: HashMap<String, usize> = HashMap::new();
        let mut result: HashMap<String, SensorValue> = HashMap::new();

        for entry in self.iter_entries() {
            let base_name = entry.name_original.clone();
            let count = name_counter.get(&base_name).copied().unwrap_or(0);

            let final_name = if count == 0 {
                base_name.clone()
            } else {
                format!("{} #{}", base_name, count + 1)
            };

            name_counter.insert(base_name, count + 1);
            result.insert(final_name, SensorValue {
                value: entry.value,
                unit: entry.unit,
            });
        }

        result
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

    #[test]
    #[cfg(target_os = "windows")]
    fn test_duplicate_sensor_names() {
        // 测试同名传感器编号逻辑
        // 注意：需要HWiNFO正在运行
        if let Ok(collector) = HWiNFOCollector::new() {
            let entries = collector.get_all_entries();

            // 验证至少有100个传感器
            assert!(entries.len() > 100, "应该有至少100个传感器");

            // 验证同名传感器编号格式
            let duplicate_names: Vec<_> = entries.keys()
                .filter(|k| k.contains(" #"))
                .collect();

            for name in &duplicate_names {
                // 编号格式应该是 "#2", "#3" 等
                assert!(name.contains(" #2") || name.contains(" #3"),
                    "同名编号格式正确: {}", name);
            }

            println!("测试通过：{}个传感器，{}个同名传感器",
                entries.len(), duplicate_names.len());
        }
    }
}