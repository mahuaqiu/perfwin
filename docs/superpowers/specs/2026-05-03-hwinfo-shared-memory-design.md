# HWiNFO 共享内存数据对接设计

**日期**: 2026-05-03
**状态**: 待审核

## 概述

将Python参考实现 (`D:\code\hwinfo-oled-monitor-main`) 的HWiNFO共享内存读取逻辑移植到Rust项目，获取系统级性能数据：CPU/GPU使用率、温度、功耗，以及网络上传/下载速度。

## 背景

现有Rust项目的 `hwinfo.rs` 结构体定义不完整，无法正确解析HWiNFO共享内存。Python实现提供了完整的结构体布局和传感器匹配逻辑。

## 数据源

HWiNFO共享内存结构：
- 共享内存名称: `Global\HWiNFO_SENS_SM2` (Windows下需要 `Global\` 前缀)
- 魔数: `0x53695748`

### 结构体定义 (来自pywhinfo.py)

**重要**: 所有结构体使用 `#[repr(C, packed)]`，与Python的 `_pack_ = 1` 保持一致，确保内存布局正确。

**Header**:
```rust
#[repr(C, packed)]
struct HWiNFOHeader {
    magic: u32,              // 0x53695748，用于验证HWiNFO版本兼容性
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
```

**Sensor Entry** (注意: `type` 是Rust关键字，改用 `sensor_type`):
```rust
#[repr(C, packed)]
struct HWiNFOEntry {
    sensor_type: u32,        // 传感器类型 (注意: 不能用 'type', 是Rust关键字)
    sensor_index: u32,
    id: u32,
    name_original: [u8; 128],
    name_user: [u8; 128],
    unit: [u8; 16],
    value: f64,
    value_min: f64,
    value_max: f64,
    value_avg: f64,
}
```

### 传感器类型枚举

```rust
#[repr(u32)]
enum SensorType {
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
```

## 传感器匹配策略

通过关键词匹配传感器名称获取目标数据。采用优先级匹配：优先匹配更精确的名称。

| 数据项 | 优先关键词 | 次级关键词 | 单位过滤 |
|--------|-----------|-----------|---------|
| CPU使用率 | "total cpu usage", "cpu total" | "cpu usage", "cpu utilization" | `%` |
| CPU温度 | "cpu package", "cpu tctl", "cpu tdie" | "cpu", "processor" | `°C` 或 `C` |
| CPU功耗 | "cpu package power" | "cpu power" | `W` |
| GPU使用率 | "gpu core load", "gpu d3d usage" | "gpu utilization", "gpu activity" | `%` |
| GPU温度 | "gpu core", "gpu hotspot" | "gpu" | `°C` 或 `C` |
| GPU功耗 | "gpu power", "gpu total power" | - | `W` |
| 网络上传 | 需在HWiNFO环境确认实际传感器名称 | - | `B/s` 或 `KB/s` |
| 网络下载 | 需在HWiNFO环境确认实际传感器名称 | - | `B/s` 或 `KB/s` |

匹配逻辑：
1. 遍历所有传感器条目
2. 将名称转为小写
3. 按优先级检查关键词组合（先检查组合词如 "cpu package"，再检查单个词）
4. 检查单位是否符合预期
5. 返回第一个匹配的传感器值

**注意**: 网络速度传感器名称需要在实际HWiNFO环境中验证。如果HWiNFO不提供网络数据，可从sysinfo或其他来源获取。

## 实现方案

### 文件改动

**src/collector/hwinfo.rs**:
- 重写 `HWiNFOHeader` 结构体（修正字段，添加魔数校验）
- 新增 `HWiNFOEntry` 结构体（字段名避开Rust关键字）
- 新增 `SensorType` 枚举
- 修正共享内存名称为 `Global\HWiNFO_SENS_SM2`
- 实现 `iter_entries()` 遍历所有传感器
- 实现 `find_sensor()` 关键词匹配（返回 `Option<f64>`）
- 实现 `parse_string()` 解析固定长度字符串字段
- 重写 `get_system_info()` 获取CPU/GPU/网络数据

**src/hwinfo_manager.rs**:
- 修正默认路径为相对路径：`HWiNFO64/HWiNFO64.EXE`
- 路径计算：`std::env::current_exe().parent().join("HWiNFO64").join("HWiNFO64.EXE")`

### 代码结构

```rust
pub struct HWiNFOCollector {
    handle: HANDLE,
    mapped_ptr: MEMORY_MAPPED_VIEW_ADDRESS,
    header_ptr: *const HWiNFOHeader,
}

impl HWiNFOCollector {
    /// 连接共享内存，校验魔数
    pub fn new() -> Result<Self>;
    
    /// 遍历所有传感器条目
    pub fn iter_entries(&self) -> impl Iterator<Item = SensorEntry> + '_;
    
    /// 按关键词和单位查找传感器值
    /// 返回 Option<f64>，找不到时返回 None
    pub fn find_sensor(&self, keywords: &[&str], unit_hint: Option<&str>) -> Option<f64>;
    
    /// 获取系统信息
    pub fn get_system_info(&self) -> Result<SystemInfo>;
}
```

### 数据映射到 SystemInfo

```rust
fn get_system_info(&self) -> Result<SystemInfo> {
    Ok(SystemInfo {
        cpu: CPUInfo {
            // 优先匹配 "total" 关键词获取整体CPU使用率
            percent: self.find_sensor(&["total cpu", "cpu total"], Some("%"))
                .or_else(|| self.find_sensor(&["cpu usage", "cpu utilization"], Some("%")))
                .unwrap_or(0.0),
            temperature: self.find_sensor(&["cpu package", "cpu tctl"], Some("C")),
            power: self.find_sensor(&["cpu package power"], Some("W")),
        },
        gpu: GPUInfo {
            percent: self.find_sensor(&["gpu core load", "gpu d3d"], Some("%"))
                .or_else(|| self.find_sensor(&["gpu utilization"], Some("%")))
                .unwrap_or(0.0),
            temperature: self.find_sensor(&["gpu core", "gpu hotspot"], Some("C")),
            power: self.find_sensor(&["gpu power"], Some("W")),
            memory_mb: None,  // HWiNFO提供GPU显存使用率，不是MB值
        },
        network: NetworkInfo {
            // 网络传感器名称待确认，暂时返回0
            upload_speed: self.find_sensor(&["upload", "send"], Some("B/s"))
                .unwrap_or(0.0),
            download_speed: self.find_sensor(&["download", "receive"], Some("B/s"))
                .unwrap_or(0.0),
        },
        memory: MemoryInfo::default(),  // 内存从sysinfo获取
    })
}
```

## 错误处理

- **共享内存连接失败**: 返回错误信息提示用户检查HWiNFO是否运行并启用共享内存
- **魔数不匹配**: 返回错误提示HWiNFO版本可能不兼容
- **传感器未找到**: 对应字段返回 `None` 或默认值 `0.0`（不影响其他数据采集）

## 测试策略

1. **单元测试**: 验证结构体大小与Python ctypes结构体一致
2. **集成测试**: 需要HWiNFO运行环境，验证实际数据读取
3. **手动验证**: 运行Python脚本 `pywhinfo.py` 确认传感器关键词

## 依赖

- `windows` crate: Win32 API (OpenFileMappingW, MapViewOfFile)
- 无新增依赖