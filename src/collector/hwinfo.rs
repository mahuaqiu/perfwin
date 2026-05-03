// HWiNFO 采集器 - 系统级数据
// 读取 HWiNFO 共享内存获取系统监控数据

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

/// HWiNFO 共享内存名称
#[cfg(target_os = "windows")]
const HWINFO_SHARED_MEM_NAME: &str = "HWiNFO_SENS_SM2";

/// HWiNFO 共享内存头结构（需要根据 HWiNFO 文档调整）
/// 参考: https://www.hwinfo.com/forum/threads/introducing-hwinfo-gadget.1139/
#[cfg(target_os = "windows")]
#[repr(C)]
struct HWiNFOSharedMemHeader {
    signature: u32,      // 签名，用于验证
    version: u32,       // 版本号
    size: u32,          // 共享内存大小
    num_sensors: u32,   // 传感器数量
    // TODO: 根据实际 HWiNFO 共享内存格式添加其他字段
    // 例如: sensor_offset, name_offset 等
}

/// HWiNFO 传感器数据结构（需要根据实际格式调整）
#[cfg(target_os = "windows")]
#[repr(C)]
struct HWiNFOSensorEntry {
    sensor_id: u64,     // 传感器 ID
    value: f64,         // 传感器值
    unit: u32,          // 单位类型
    name_offset: u32,   // 名称在内存中的偏移量
    // TODO: 根据实际 HWiNFO 共享内存格式添加其他字段
}

/// HWiNFO 共享内存采集器
#[cfg(target_os = "windows")]
pub struct HWiNFOCollector {
    handle: HANDLE,         // 共享内存句柄
    mapped_ptr: MEMORY_MAPPED_VIEW_ADDRESS,    // 映射的内存指针
    size: usize,           // 映射内存大小
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
            .map_err(|e| anyhow::anyhow!("OpenFileMappingW failed: {}", e))?;

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

        // 读取头部信息获取大小
        let header = unsafe { &*(mapped_ptr.Value as *const HWiNFOSharedMemHeader) };
        let size = header.size as usize;

        Ok(Self {
            handle,
            mapped_ptr,
            size,
        })
    }

    /// 检查共享内存是否有效
    pub fn is_valid(&self) -> bool {
        !self.handle.is_invalid() && !self.mapped_ptr.Value.is_null()
    }

    /// 获取共享内存大小
    pub fn size(&self) -> usize {
        self.size
    }

    /// 解析系统信息
    /// TODO: 需要根据 HWiNFO 共享内存格式详细实现
    /// 参考 HWiNFO 官方文档获取准确的内存布局
    pub fn get_system_info(&self) -> anyhow::Result<SystemInfo> {
        if !self.is_valid() {
            return Err(anyhow::anyhow!("HWiNFO 共享内存无效"));
        }

        // 读取头部
        let _header = unsafe { &*(self.mapped_ptr.Value as *const HWiNFOSharedMemHeader) };

        // TODO: 根据实际 HWiNFO 共享内存格式解析传感器数据
        // 1. 遍历传感器列表
        // 2. 根据传感器 ID 或名称匹配 CPU/GPU/内存/网络数据
        // 3. 提取温度、功率、使用率等信息

        // 框架实现 - 返回默认值
        Ok(SystemInfo {
            cpu: CPUInfo {
                percent: 0.0,
                temperature: None,
                power: None,
            },
            gpu: GPUInfo {
                percent: 0.0,
                temperature: None,
                power: None,
                memory_mb: None,
            },
            memory: MemoryInfo {
                percent: 0.0,
                used_mb: 0.0,
                total_mb: 0.0,
                committed_mb: 0.0,
                committed_limit_mb: 0.0,
            },
            network: NetworkInfo {
                upload_speed: 0.0,
                download_speed: 0.0,
            },
        })
    }

    /// 读取字符串（从共享内存中的偏移量）
    /// TODO: 实现字符串读取逻辑
    #[allow(dead_code)]
    fn read_string(&self, offset: u32) -> String {
        if offset as usize >= self.size {
            return String::new();
        }

        // 从偏移量位置读取 null-terminated 字符串
        let start = unsafe { (self.mapped_ptr.Value as *const u8).add(offset as usize) };
        let mut end = start;
        let mut len = 0;

        // 查找 null 终止符
        unsafe {
            while len < self.size - offset as usize && *end != 0 {
                end = end.add(1);
                len += 1;
            }
        }

        // 构建字符串（假设是 ASCII 或 UTF-8）
        let slice = unsafe { std::slice::from_raw_parts(start, len) };
        String::from_utf8_lossy(slice).into_owned()
    }

    /// 根据传感器名称查找值
    /// TODO: 实现传感器查找逻辑
    #[allow(dead_code)]
    fn find_sensor_value(&self, _name: &str) -> Option<f64> {
        // TODO: 遍历传感器列表，匹配名称，返回值
        None
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

    pub fn size(&self) -> usize {
        0
    }

    pub fn get_system_info(&self) -> anyhow::Result<SystemInfo> {
        Err(anyhow::anyhow!("HWiNFO 仅在 Windows 平台上可用"))
    }
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
        // 如果 HWiNFO 未运行，应该返回错误
        if result.is_err() {
            let err = result.unwrap_err();
            assert!(err.to_string().contains("HWiNFO"));
        }
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_hwinfo_collector_not_windows() {
        let result = HWiNFOCollector::new();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Windows"));
    }
}