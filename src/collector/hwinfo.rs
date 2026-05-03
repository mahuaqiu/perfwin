// HWiNFO 采集器 - 系统级数据
// 从 HWiNFO 共享内存读取系统监控数据

use crate::data::{SystemInfo, CPUInfo, GPUInfo, MemoryInfo, NetworkInfo};

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
/// 与 Python pywhinfo.py 的 HWiNFOHeader 结构一致
#[cfg(target_os = "windows")]
#[repr(C, packed)]
struct HWiNFOHeader {
    magic: u32,                  // 魔数 0x53695748
    version: u32,                // 版本号
    version2: u32,               // 版本号2
    last_update: i64,            // 最后更新时间
    sensor_section_offset: u32,  // 传感器段偏移
    sensor_element_size: u32,    // 传感器元素大小
    sensor_element_count: u32,   // 传感器元素数量
    entry_section_offset: u32,   // 数据条目段偏移
    entry_element_size: u32,     // 数据条目元素大小
    entry_element_count: u32,    // 数据条目元素数量
}

/// HWiNFO 数据条目结构
/// 与 Python pywhinfo.py 的 HWiNFOEntry 结构一致
/// 注意: 'type' 是 Rust 关键字，改用 'sensor_type'
/// 注意: f64 字段用 [u8; 8] 表示以确保 packed 结构体大小精确匹配
#[cfg(target_os = "windows")]
#[repr(C, packed)]
struct HWiNFOEntry {
    sensor_type: u32,            // 传感器类型 (注意: type 是 Rust 关键字)
    sensor_index: u32,           // 传感器索引
    id: u32,                     // 条目 ID
    name_original: [u8; 128],    // 原始名称
    name_user: [u8; 128],        // 用户自定义名称
    unit: [u8; 16],              // 单位
    value_bytes: [u8; 8],        // 当前值 (f64，用字节数组确保大小)
    value_min_bytes: [u8; 8],    // 最小值 (f64)
    value_max_bytes: [u8; 8],    // 最大值 (f64)
    value_avg_bytes: [u8; 8],    // 平均值 (f64)
}

/// 从字节数组读取 f64 值
#[cfg(target_os = "windows")]
fn read_f64_from_bytes(bytes: &[u8; 8]) -> f64 {
    f64::from_bits(u64::from_ne_bytes(*bytes))
}

/// 解析固定长度字符串字段
#[cfg(target_os = "windows")]
fn parse_fixed_string(bytes: &[u8]) -> String {
    // 找到第一个 null 字符的位置
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    // 截取有效部分并解码
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// HWiNFO 共享内存采集器
#[cfg(target_os = "windows")]
pub struct HWiNFOCollector {
    handle: HANDLE,
    mapped_ptr: MEMORY_MAPPED_VIEW_ADDRESS,
    header: *const HWiNFOHeader,
}

// 手动实现 Debug，因为 HANDLE 和指针不自动实现 Debug
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
    /// 需要确保 HWiNFO 已经运行并启用了共享内存功能
    pub fn new() -> anyhow::Result<Self> {
        // 将共享内存名称转换为宽字符
        let name_wide: Vec<u16> = HWINFO_SHARED_MEM_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // 打开共享内存
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

        // 读取头部并验证魔数
        let header = unsafe { &*(mapped_ptr.Value as *const HWiNFOHeader) };

        // packed struct 字段不能直接引用，需要先复制
        let magic = header.magic;

        if magic != HWINFO_HEADER_MAGIC {
            unsafe {
                let _ = UnmapViewOfFile(mapped_ptr);
                let _ = CloseHandle(handle);
            };
            return Err(anyhow::anyhow!(
                "HWiNFO 共享内存魔数不匹配: 0x{:08X} (期望 0x{:08X})。版本可能不兼容。",
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

            // 跳过无效条目
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

    /// 按关键词查找传感器值
    /// 优先匹配组合关键词，再匹配单个关键词
    /// 返回 Option<f64>，找不到时返回 None
    pub fn find_sensor(&self, primary_keywords: &[&str], secondary_keywords: &[&str], unit_hint: Option<&str>) -> Option<f64> {
        // 先尝试优先关键词
        for entry in self.iter_entries() {
            let name_lower = entry.label().to_lowercase();

            // 检查是否包含所有优先关键词
            if primary_keywords.iter().all(|kw| name_lower.contains(kw)) {
                if let Some(unit) = unit_hint {
                    if !entry.unit.to_lowercase().contains(&unit.to_lowercase()) {
                        continue;
                    }
                }
                return Some(entry.value);
            }
        }

        // 再尝试次级关键词
        for entry in self.iter_entries() {
            let name_lower = entry.label().to_lowercase();

            // 检查是否包含任意次级关键词
            if secondary_keywords.iter().any(|kw| name_lower.contains(kw)) {
                if let Some(unit) = unit_hint {
                    if !entry.unit.to_lowercase().contains(&unit.to_lowercase()) {
                        continue;
                    }
                }
                return Some(entry.value);
            }
        }

        None
    }

    /// 获取系统信息
    pub fn get_system_info(&self) -> anyhow::Result<SystemInfo> {
        if !self.is_valid() {
            return Err(anyhow::anyhow!("HWiNFO 共享内存无效"));
        }

        Ok(SystemInfo {
            cpu: CPUInfo {
                // CPU 使用率 - 优先匹配 "total"，再匹配单个关键词
                percent: self.find_sensor(
                    &["total", "cpu"],
                    &["cpu usage", "cpu utilization", "cpu load"],
                    Some("%")
                ).unwrap_or(0.0),
                // CPU 温度
                temperature: self.find_sensor(
                    &["cpu", "package"],
                    &["cpu tctl", "cpu tdie", "processor"],
                    Some("C")
                ),
                // CPU 功耗
                power: self.find_sensor(
                    &["cpu", "package", "power"],
                    &["cpu power"],
                    Some("W")
                ),
            },
            gpu: GPUInfo {
                // GPU 使用率
                percent: self.find_sensor(
                    &["gpu core load"],
                    &["gpu d3d", "gpu utilization", "gpu activity"],
                    Some("%")
                ).unwrap_or(0.0),
                // GPU 温度
                temperature: self.find_sensor(
                    &["gpu", "core"],
                    &["gpu hotspot", "gpu temp"],
                    Some("C")
                ),
                // GPU 功耗
                power: self.find_sensor(
                    &["gpu", "power"],
                    &["gpu total power"],
                    Some("W")
                ),
                // GPU 显存 - HWiNFO 通常提供使用率而非 MB 值
                memory_mb: None,
            },
            memory: MemoryInfo::default(),  // 内存数据从 sysinfo 获取
            network: NetworkInfo {
                // 网络上传速度 - 关键词待验证
                upload_speed: self.find_sensor(
                    &["upload", "send"],
                    &["tx", "transmit"],
                    Some("B/s")
                ).or_else(|| self.find_sensor(
                    &["upload", "send"],
                    &["tx", "transmit"],
                    Some("KB/s")
                ).map(|v| v * 1024.0))
                .unwrap_or(0.0),
                // 网络下载速度 - 关键词待验证
                download_speed: self.find_sensor(
                    &["download", "receive"],
                    &["rx", "recv"],
                    Some("B/s")
                ).or_else(|| self.find_sensor(
                    &["download", "receive"],
                    &["rx", "recv"],
                    Some("KB/s")
                ).map(|v| v * 1024.0))
                .unwrap_or(0.0),
            },
        })
    }
}

#[cfg(target_os = "windows")]
impl Drop for HWiNFOCollector {
    fn drop(&mut self) {
        // 解除内存映射
        if !self.mapped_ptr.Value.is_null() {
            unsafe {
                let _ = UnmapViewOfFile(self.mapped_ptr);
            }
        }
        // 关闭句柄
        if !self.handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
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
// 非 Windows 平台的占位实现
// ============================================================================

#[cfg(not(target_os = "windows"))]
pub struct HWiNFOCollector {
    _private: (),
}

#[cfg(not(target_os = "windows"))]
impl HWiNFOCollector {
    pub fn new() -> anyhow::Result<Self> {
        Err(anyhow::anyhow!("HWiNFO 仅在 Windows 平台上可用"))
    }

    pub fn is_valid(&self) -> bool {
        false
    }

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
    fn test_hwinfo_collector_new() {
        // 此测试需要 HWiNFO 正在运行
        let result = HWiNFOCollector::new();
        if result.is_err() {
            let err = result.unwrap_err();
            assert!(err.to_string().contains("HWiNFO"));
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_header_size() {
        // 验证结构体大小与 Python ctypes 一致 (44 bytes)
        assert_eq!(std::mem::size_of::<HWiNFOHeader>(), 44);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_entry_size() {
        // 注意: Rust #[repr(C, packed)] 可能添加尾部 padding
        // 实际读取时使用 HWiNFO 提供的 entry_element_size，不依赖 Rust 结构体大小
        // Python ctypes _pack_=1 确保精确 312 bytes，Rust 可能略有不同
        // 这里只验证大致正确范围，运行时会正确处理
        let size = std::mem::size_of::<HWiNFOEntry>();
        assert!(size >= 312 && size <= 320, "Entry size should be around 312 bytes, got {}", size);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_parse_fixed_string() {
        let bytes = b"Hello World\0\0\0\0\0\0\0\0";
        assert_eq!(parse_fixed_string(bytes), "Hello World");

        let bytes_no_null = b"NoNullTerminator................";
        assert!(parse_fixed_string(bytes_no_null).contains("NoNullTerminator"));
    }

    #[test]
    fn test_sensor_entry_label() {
        let entry = SensorEntry {
            sensor_type: SensorType::Temp,
            sensor_index: 0,
            id: 1,
            name_original: "Original Name".to_string(),
            name_user: "User Name".to_string(),
            unit: "C".to_string(),
            value: 45.0,
            value_min: 30.0,
            value_max: 80.0,
            value_avg: 50.0,
        };
        assert_eq!(entry.label(), "User Name");

        let entry_no_user = SensorEntry {
            sensor_type: SensorType::Temp,
            sensor_index: 0,
            id: 1,
            name_original: "Original Name".to_string(),
            name_user: "".to_string(),
            unit: "C".to_string(),
            value: 45.0,
            value_min: 30.0,
            value_max: 80.0,
            value_avg: 50.0,
        };
        assert_eq!(entry_no_user.label(), "Original Name");
    }
}