# HWiNFO 原始数据扩展设计方案

**日期**: 2026-05-08
**版本**: v1.1
**作者**: Claude Code

## 概述

当前 perfwin 的 HWiNFO 数据采集使用配置文件硬编码提取特定传感器（如 "CPU Package"、"GPU Temperature"），扩展困难。本设计将改为返回 HWiNFO 共享内存中的所有传感器数据，按原始名称索引，支持灵活的系统数据分析。

**重要说明**：当前 `SystemInfo.memory` 数据来自 **sysinfo crate**（通过 Windows API `GlobalMemoryStatusEx`），不属于 HWiNFO。本设计将删除此数据，用户将失去内存使用率监控能力。

## 设计目标

1. **返回所有 HWiNFO 数据**：不再限制为预定义字段，用户可按需提取
2. **简化配置**：删除 `hwinfo_sensors.toml` 配置文件，降低维护成本
3. **保持进程级数据**：进程监控功能（sysinfo/PDH）不受影响
4. **处理同名传感器**：自动编号区分重复名称
5. **删除内存数据**：sysinfo 的内存监控数据将不再提供

## 数据来源对照表

| 当前字段 | 数据来源 | 设计后处理 | 可迁移性 |
|---------|---------|-----------|---------|
| `system.cpu.percent` | HWiNFO (`Total CPU Usage`) | `hwinfo_raw["Total CPU Usage"]` | ✓ |
| `system.cpu.temperature` | HWiNFO (`CPU Package`) | `hwinfo_raw["CPU Package"]` | ✓ |
| `system.cpu.power` | HWiNFO (`CPU Package Power`) | `hwinfo_raw["CPU Package Power"]` | ✓ |
| `system.cpu.clock_speed` | HWiNFO (`Core X Clock`) | `hwinfo_raw["Core X Clock"]` | ✓ |
| `system.gpu.percent` | HWiNFO (`GPU D3D Usage`) | `hwinfo_raw["GPU D3D Usage"]` | ✓ |
| `system.gpu.temperature` | HWiNFO (`GPU Temperature`) | `hwinfo_raw["GPU Temperature"]` | ✓ |
| `system.gpu.power` | HWiNFO (`GPU Power`) | `hwinfo_raw["GPU Power"]` | ✓ |
| **`system.memory`** | **sysinfo crate** | **删除** | **不可迁移** |
| `system.network.upload_speed` | HWiNFO (`Current UP rate`) | `hwinfo_raw["Current UP rate"]` | ✓ |
| `system.network.download_speed` | HWiNFO (`Current DL rate`) | `hwinfo_raw["Current DL rate"]` | ✓ |
| `system.battery.charge_level` | HWiNFO (`Charge Level`) | `hwinfo_raw["Charge Level"]` | ✓ |
| `system.system_power` | HWiNFO (`Total System Power`) | `hwinfo_raw["Total System Power"]` | ✓ |

**注意**：HWiNFO 没有"内存使用率"、"已用内存"、"提交内存"这类数据，只有硬件传感器数据（温度、功耗、电压、风扇转速、时钟频率等）。内存数据完全由 sysinfo crate 提供，不属于 HWiNFO。

## 数据结构设计

### Sample 结构变更

**旧版（删除）：**
```rust
struct Sample {
    timestamp: DateTime<Utc>,
    system: SystemInfo,  // 删除：预定义的系统级数据
    processes: Option<Vec<ProcessInfo>>,
    aggregated: Option<Vec<AggregatedProcessInfo>>,
    top_n_cpu: Option<Vec<ProcessInfo>>,
    top_n_gpu: Option<Vec<ProcessInfo>>,
}
```

**新版：**
```rust
struct Sample {
    timestamp: DateTime<Utc>,
    hwinfo_raw: HashMap<String, SensorValue>,  // 新增：所有HWiNFO原始数据
    processes: Option<Vec<ProcessInfo>>,       // 保持：进程明细
    aggregated: Option<Vec<AggregatedProcessInfo>>, // 保持：进程汇总
    top_n_cpu: Option<Vec<ProcessInfo>>,       // 保持：Top N CPU
    top_n_gpu: Option<Vec<ProcessInfo>>,       // 保持：Top N GPU
}
```

### SensorValue 结构

```rust
struct SensorValue {
    value: f64,
    unit: String,
}
```

**Python API 示例：**
```python
sample.hwinfo_raw = {
    "Virtual Memory Committed": {"value": 13215.0, "unit": "MB"},
    "CPU Package": {"value": 60.0, "unit": "°C"},
    "CPU Package #2": {"value": 55.0, "unit": "°C"},
    "Core 0 Clock": {"value": 4280.0, "unit": "MHz"},
    "GPU Temperature": {"value": 60.0, "unit": "°C"},
    "Drive Temperature": {"value": 40.0, "unit": "°C"},
    "Drive Temperature #2": {"value": 32.0, "unit": "°C"},
}
```

### 同名传感器编号规则

HWiNFO 可能存在同名传感器（如多个硬盘的 "Drive Temperature"），编号规则：
- 第一次出现：使用原名称（如 "CPU Package"）
- 第二次出现：原名称 + "#2"（如 "CPU Package #2"）
- 第N次出现：原名称 + "#N"

实现逻辑：
```rust
let mut name_counter: HashMap<String, usize> = HashMap::new();
let mut hwinfo_raw: HashMap<String, SensorValue> = HashMap::new();

for entry in collector.iter_entries() {
    let base_name = entry.name_original.clone();
    let count = name_counter.get(&base_name).unwrap_or(0);

    let final_name = if count == 0 {
        base_name.clone()
    } else {
        format!("{} #{}", base_name, count + 1)
    };

    name_counter.insert(base_name, count + 1);
    hwinfo_raw.insert(final_name, SensorValue {
        value: entry.value,
        unit: entry.unit,
    });
}
```

## 代码变更范围

### 删除文件

- `hwinfo_sensors.toml`：传感器配置文件

### 删除结构

**data.rs**：
- `SystemInfo`, `CPUInfo`, `GPUInfo`, `MemoryInfo`, `NetworkInfo`, `BatteryInfo`

**lib.rs**：
- `PyCPUInfo`, `PyGPUInfo`, `PyMemoryInfo`, `PyNetworkInfo`, `PyBatteryInfo`, `PySystemInfo` 类

**collector/hwinfo.rs**：
- `HWiNFOConfig`, `CpuConfig`, `GpuConfig`, `SystemConfig`, `NetworkConfig`, `BatteryConfig`
- 配置加载逻辑（`load()`, `find_config_path()`, `default_config()`）
- `get_system_info()` 方法
- `find_by_name()`, `find_by_name_opt()`, `find_by_pattern_avg()` 方法

### 新增代码

**collector/hwinfo.rs**：
```rust
impl HWiNFOCollector {
    /// 获取所有传感器数据（按原始名称索引）
    pub fn get_all_entries(&self) -> HashMap<String, SensorValue> {
        let mut name_counter: HashMap<String, usize> = HashMap::new();
        let mut result: HashMap<String, SensorValue> = HashMap::new();

        for entry in self.iter_entries() {
            let base_name = entry.name_original.clone();
            let count = name_counter.get(&base_name).unwrap_or(0);

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
```

**lib.rs**：
```rust
#[pyclass(name = "Sample")]
pub struct PySample {
    inner: Sample,
}

#[pymethods]
impl PySample {
    #[getter]
    fn hwinfo_raw(&self) -> PyResult<PyObject> {
        Python::with_gil(|py| {
            let dict = PyDict::new(py);
            for (name, value) in &self.inner.hwinfo_raw {
                let value_dict = PyDict::new(py);
                value_dict.set_item("value", value.value)?;
                value_dict.set_item("unit", value.unit)?;
                dict.set_item(name, value_dict)?;
            }
            Ok(dict.into())
        })
    }

    // 其他getter保持不变（processes, aggregated, top_n_cpu, top_n_gpu）
}
```

### 修改文件

**monitor.rs**：
- 修改 `Sample` 结构定义
- 修改采集线程逻辑：调用 `get_all_entries()` 填充 `hwinfo_raw`
- 删除 `SystemInfo` 相关代码

**data.rs**：
- 删除系统级结构定义
- 新增 `SensorValue` 结构
- 修改 `Sample` 结构

## 数据采集流程

```
MonitorCore::collect_sample()
  ├─ HWiNFOCollector::get_all_entries()
  │   ├─ iter_entries()：遍历所有传感器
  │   ├─ 处理同名编号
  │   └─ 构建 HashMap<String, SensorValue>
  │
  ├─ SysinfoCollector::collect()（进程级数据，保持不变）
  ├─ PdhCollector::collect()（进程GPU数据，保持不变）
  │
  └─ 构建 Sample {
      timestamp: now(),
      hwinfo_raw: hwinfo_data,
      processes: ...,
      aggregated: ...,
      top_n_cpu: ...,
      top_n_gpu: ...,
  }
```

## API 使用示例

### 基本使用

```python
import perfwin

with perfwin.Monitor(interval=1.0, duration=10) as monitor:
    time.sleep(11)
    result = monitor.get_result()

# 获取第一个样本
sample = result.samples[0]
hwinfo = sample.hwinfo_raw

# 查找特定传感器
cpu_temp = hwinfo.get("CPU Package", {}).get("value")
gpu_temp = hwinfo.get("GPU Temperature", {}).get("value")

# 遍历所有传感器
for name, data in hwinfo.items():
    print(f"{name}: {data['value']} {data['unit']}")
```

### 数据分析示例

```python
# 提取所有温度传感器
temps = {k: v for k, v in hwinfo.items() if v['unit'] == '°C'}

# 提取所有功耗传感器
powers = {k: v for k, v in hwinfo.items() if 'W' in v['unit']}

# 分析CPU核心温度
core_temps = {k: v for k, v in hwinfo.items() if k.startswith('Core') and v['unit'] == '°C'}
avg_core_temp = sum(v['value'] for v in core_temps.values()) / len(core_temps)
```

## 测试计划

### 单元测试

1. **同名传感器编号测试**：验证多个同名传感器正确编号
2. **数据结构测试**：验证 `SensorValue` 序列化/反序列化
3. **Python绑定测试**：验证 `hwinfo_raw` getter 正确返回字典

### 集成测试

1. **真实HWiNFO数据测试**：启动HWiNFO，验证能获取所有传感器（300+）
2. **进程数据兼容测试**：验证进程监控功能不受影响
3. **API兼容测试**：验证 `to_dicts()` 方法正确输出新格式

### 性能测试

1. **内存占用**：验证 300+ 传感器的内存开销可控
2. **采集性能**：验证 1秒间隔采集的性能无明显下降

## 版本兼容性

**版本号升级**：从 0.2.x 升级到 0.3.0（重大API变更）

**Breaking Changes**：
1. `sample.system` 字段删除，用户需改用 `sample.hwinfo_raw`
2. **内存监控数据丢失**：`system.memory`（使用率、已用/总量、提交内存）不再提供，该数据来自 sysinfo crate，不属于 HWiNFO
3. 用户需知道HWiNFO传感器的原始名称才能访问数据

**迁移指南**：
```python
# ===== CPU 数据 =====
# 旧版代码
cpu_temp = sample.system.cpu.temperature
cpu_power = sample.system.cpu.power
cpu_usage = sample.system.cpu.percent

# 新版代码
hwinfo = sample.hwinfo_raw
cpu_temp = hwinfo.get("CPU Package", {}).get("value")
cpu_power = hwinfo.get("CPU Package Power", {}).get("value")
cpu_usage = hwinfo.get("Total CPU Usage", {}).get("value")

# ===== GPU 数据 =====
# 旧版代码
gpu_temp = sample.system.gpu.temperature
gpu_usage = sample.system.gpu.percent

# 新版代码
gpu_temp = hwinfo.get("GPU Temperature", {}).get("value")
gpu_usage = hwinfo.get("GPU D3D Usage", {}).get("value")

# ===== Network 数据 =====
# 旧版代码
upload_speed = sample.system.network.upload_speed
download_speed = sample.system.network.download_speed

# 新版代码
upload_speed = hwinfo.get("Current UP rate", {}).get("value") * 1024  # 注意单位转换
download_speed = hwinfo.get("Current DL rate", {}).get("value") * 1024

# ===== Memory 数据 =====
# 旧版代码
memory_percent = sample.system.memory.percent
memory_used = sample.system.memory.used_mb
memory_total = sample.system.memory.total_mb

# 新版代码
# ⚠️ 数据不再提供！需自行调用其他API获取内存数据
# 或使用psutil库：
import psutil
memory_percent = psutil.virtual_memory().percent
memory_used = psutil.virtual_memory().used / 1024 / 1024  # MB
memory_total = psutil.virtual_memory().total / 1024 / 1024  # MB

# ===== 查看所有可用传感器 =====
# 打印所有传感器名称和值
for name, data in sample.hwinfo_raw.items():
    print(f"{name}: {data['value']} {data['unit']}")
```

## 风险与限制

### 已知限制

1. **内存数据丢失**：sysinfo 的内存使用率、已用/总量、提交内存数据将不再提供。用户需自行使用 `psutil` 或其他库获取内存数据。
2. **名称依赖**：用户需知道HWiNFO的传感器原始名称，可能需要文档或示例
3. **同名编号**：编号顺序依赖遍历顺序，可能不固定（但通常稳定）
4. **传感器名称差异**：不同硬件的传感器名称可能不同（如 "CPU Package" vs "CPU Temperature"）

### 风险缓解

1. **内存替代方案**：推荐用户使用 `psutil.virtual_memory()` 获取内存数据
2. **文档补充**：提供常见传感器名称列表和查找示例
3. **示例代码**：提供数据分析示例脚本，包含遍历所有传感器的代码
4. **名称稳定性**：HWiNFO的传感器顺序通常稳定，编号应保持一致
5. **打印辅助**：建议用户先运行一次遍历，查看所有传感器名称

## 实现计划

1. **删除旧代码**：删除配置文件、系统级结构、配置加载逻辑
2. **新增方法**：实现 `get_all_entries()` 和同名编号逻辑
3. **修改结构**：修改 `Sample` 结构，更新 Python 绑定
4. **更新序列化**：修改 `to_dicts()` 方法，返回新格式的字典
5. **测试验证**：运行单元测试和集成测试
6. **版本更新**：更新 `pyproject.toml` 和 `lib.rs` 版本号到 0.3.0
7. **文档更新**：更新 CLAUDE.md API 说明，添加迁移指南
8. **示例更新**：更新 `examples/basic_usage.py`，展示新的数据访问方式

## 参考

- 测试结果：335个传感器，15个同名传感器（如 "CPU Package", "Drive Temperature"）
- sensor_index 能区分不同设备组，但用户更倾向于简单的编号方案
- HWiNFO 共享内存结构：Entry section 包含所有传感器数据