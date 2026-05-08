# HWiNFO 原始数据扩展实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将HWiNFO数据采集改为返回所有传感器原始数据，删除预定义的系统级数据结构。

**Architecture:** 
- 删除配置文件和系统级数据结构（SystemInfo/CPUInfo/GPUInfo等）
- 新增SensorValue结构，通过hwinfo_raw字段返回所有传感器数据字典
- 使用同名编号规则处理重复传感器名称

**Tech Stack:** Rust, PyO3, HWiNFO共享内存, serde

---

## File Structure

**删除文件：**
- `hwinfo_sensors.toml` - 传感器配置文件（不再需要）
- `test_hwinfo_names.py` - 临时测试文件（删除）

**修改文件：**
- `src/data.rs` - 删除系统级结构，新增SensorValue，修改Sample
- `src/lib.rs` - 删除PySystemInfo等类，修改PySample的hwinfo_raw getter
- `src/collector/hwinfo.rs` - 删除配置逻辑，新增get_all_entries方法
- `src/monitor.rs` - 修改采集逻辑，删除SystemInfo相关代码
- `tests/test_perfwin.py` - 更新测试代码
- `examples/basic_usage.py` - 更新示例代码
- `pyproject.toml` - 版本号升级到0.3.0
- `CLAUDE.md` - 更新API文档

---

### Task 1: 清理测试代码和临时文件

**Files:**
- Delete: `test_hwinfo_names.py`
- Modify: `src/collector/hwinfo.rs`（删除test_sensor_name_duplicates测试）

- [ ] **Step 1: 删除临时测试文件**

```bash
rm test_hwinfo_names.py
```

- [ ] **Step 2: 删除hwinfo.rs中的测试代码**

在 `src/collector/hwinfo.rs` 中找到并删除：
- `use itertools::Itertools;` 导入行
- `#[test] #[cfg(target_os = "windows")] fn test_sensor_name_duplicates() { ... }` 整个测试函数

保留以下测试：
- `test_header_size`
- `test_entry_size`
- `test_config_default`（将在Task 2a删除）
- `test_sensor_entry_label`

- [ ] **Step 3: 运行编译验证**

```bash
cargo build
```

Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
git add test_hwinfo_names.py src/collector/hwinfo.rs
git commit -m "chore: 清理测试代码和临时文件"
```

---

### Task 2a: 删除配置文件和配置结构

**Files:**
- Delete: `hwinfo_sensors.toml`
- Modify: `src/collector/hwinfo.rs`（删除HWiNFOConfig等结构）

- [ ] **Step 1: 删除配置文件**

```bash
rm hwinfo_sensors.toml
```

- [ ] **Step 2: 删除配置结构和导入**

在 `src/collector/hwinfo.rs` 中找到并删除：
- `use serde::Deserialize;` 导入
- `use std::path::PathBuf;` 导入
- `#[derive(Debug, Clone, Deserialize, Default)] pub struct HWiNFOConfig { ... }` 及所有字段结构（CpuConfig, GpuConfig, SystemConfig, NetworkConfig, BatteryConfig）
- `impl HWiNFOConfig { ... }` 整个实现块（包含load, find_config_path, default_config方法）
- `#[test] fn test_config_default() { ... }` 测试函数

- [ ] **Step 3: 运行编译验证**

```bash
cargo build
```

Expected: 编译失败（HWiNFOCollector::new还在使用config）

- [ ] **Step 4: Commit**

暂不提交，继续Task 2b。

---

### Task 2b: 修改HWiNFOCollector删除配置加载

**Files:**
- Modify: `src/collector/hwinfo.rs`（修改HWiNFOCollector结构和方法）

- [ ] **Step 1: 删除config字段**

找到 `pub struct HWiNFOCollector { ... }`，删除 `config: HWiNFOConfig` 字段。

- [ ] **Step 2: 修改new方法**

找到 `pub fn new() -> anyhow::Result<Self> { ... }`，删除配置加载代码：

删除：
```rust
// 加载配置
let config = HWiNFOConfig::find_config_path()
    .and_then(|p| HWiNFOConfig::load(&p).ok())
    .unwrap_or_else(HWiNFOConfig::default_config);
```

修改返回：
```rust
Ok(Self {
    handle,
    mapped_ptr,
    header,
    // config,  ← 删除这行
})
```

- [ ] **Step 3: 运行编译验证**

```bash
cargo build
```

Expected: 编译失败（get_system_info方法还在使用config）

- [ ] **Step 4: Commit**

暂不提交，继续Task 2c。

---

### Task 2c: 删除get_system_info和相关方法

**Files:**
- Modify: `src/collector/hwinfo.rs`（删除find_by_name和get_system_info方法）

- [ ] **Step 1: 删除导入和方法**

找到并删除：
- `use regex::Regex;` 导入
- `fn find_by_name(&self, target_name: &str, target_unit: &str) -> f64 { ... }` 方法
- `fn find_by_name_opt(&self, target_name: &str, target_unit: &str) -> Option<f64> { ... }` 方法
- `fn find_by_pattern_avg(&self, pattern: &str, target_unit: &str) -> Option<f64> { ... }` 方法
- `pub fn get_system_info(&self) -> anyhow::Result<SystemInfo> { ... }` 方法

保留：
- `pub fn new() -> anyhow::Result<Self>`
- `pub fn is_valid(&self) -> bool`
- `pub fn iter_entries(&self) -> impl Iterator<Item = SensorEntry> + '_`

- [ ] **Step 2: 删除label方法**

找到 `impl SensorEntry { ... }`，删除 `pub fn label(&self) -> &str { ... }` 方法。

- [ ] **Step 3: 运行编译验证**

```bash
cargo build
```

Expected: 编译失败（lib.rs和data.rs还在使用SystemInfo）

- [ ] **Step 4: Commit（Task 2a-2c合并提交）**

```bash
git add hwinfo_sensors.toml src/collector/hwinfo.rs
git commit -m "refactor: 删除HWiNFO配置文件和配置加载逻辑"
```

---

### Task 3: 删除data.rs系统级结构并新增SensorValue

**Files:**
- Modify: `src/data.rs`

- [ ] **Step 1: 新增HashMap导入**

在文件顶部添加：
```rust
use std::collections::HashMap;
```

- [ ] **Step 2: 删除系统级结构**

找到并删除以下结构定义：
- `pub struct CPUInfo { ... }`
- `pub struct GPUInfo { ... }`
- `pub struct MemoryInfo { ... }`
- `pub struct NetworkInfo { ... }`
- `pub struct BatteryInfo { ... }`
- `pub struct SystemInfo { ... }`

保留：
- `pub struct ProcessInfo { ... }`
- `pub struct AggregatedProcessInfo { ... }`
- `pub struct ProcessFilter { ... }`
- `pub struct MonitorConfig { ... }`

- [ ] **Step 3: 新增SensorValue结构**

在删除位置添加：
```rust
/// HWiNFO 传感器数据值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorValue {
    pub value: f64,
    pub unit: String,
}
```

- [ ] **Step 4: 修改Sample结构**

找到 `pub struct Sample { ... }`，修改：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub timestamp: DateTime<Utc>,
    /// HWiNFO 原始数据 - 所有传感器数据（按原始名称索引）
    pub hwinfo_raw: HashMap<String, SensorValue>,
    /// 进程明细数据 - 仅在有筛选条件时返回
    pub processes: Option<Vec<ProcessInfo>>,
    /// 进程汇总数据 - 仅在按进程名筛选时返回（多个同名PID聚合）
    pub aggregated: Option<Vec<AggregatedProcessInfo>>,
    /// Top N CPU 进程 - 仅在设置 top_n_cpu 参数时返回
    pub top_n_cpu: Option<Vec<ProcessInfo>>,
    /// Top N GPU 进程 - 仅在设置 top_n_gpu 参数时返回
    pub top_n_gpu: Option<Vec<ProcessInfo>>,
}
```

删除 `pub system: SystemInfo` 字段。

- [ ] **Step 5: 运行编译验证**

```bash
cargo build
```

Expected: 编译失败（lib.rs和monitor.rs还在使用旧结构）

- [ ] **Step 6: Commit**

暂不提交，继续Task 4。

---

### Task 4: 新增get_all_entries方法

**Files:**
- Modify: `src/collector/hwinfo.rs`

- [ ] **Step 1: 新增导入**

在文件顶部添加：
```rust
use std::collections::HashMap;
use crate::data::SensorValue;
```

- [ ] **Step 2: 新增方法**

找到 `pub fn iter_entries(&self) -> ...` 方法后，添加：

```rust
/// 获取所有传感器数据（按原始名称索引，同名传感器自动编号）
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
```

- [ ] **Step 3: 运行编译验证**

```bash
cargo build
```

Expected: 编译失败（lib.rs和monitor.rs还在使用旧结构）

- [ ] **Step 4: Commit（Task 3-4合并提交）**

```bash
git add src/data.rs src/collector/hwinfo.rs
git commit -m "refactor: 删除系统级数据结构，新增SensorValue和get_all_entries"
```

---

### Task 5a: 删除lib.rs的导入和PyCPUInfo等类

**Files:**
- Modify: `src/lib.rs`

- [ ] **Step 1: 修改导入**

找到 `use crate::data::{ ... }`，修改为：

```rust
use crate::data::{
    SensorValue, ProcessInfo, AggregatedProcessInfo, Sample, MonitorConfig, ProcessFilter,
};
```

删除：`CPUInfo, GPUInfo, MemoryInfo, NetworkInfo, SystemInfo, BatteryInfo`

- [ ] **Step 2: 删除PyCPUInfo类**

找到 `#[pyclass(name = "CPUInfo")] pub struct PyCPUInfo { ... }` 和对应的 `#[pymethods] impl PyCPUInfo { ... }` 以及 `impl From<CPUInfo> for PyCPUInfo { ... }`，全部删除。

- [ ] **Step 3: 删除PyGPUInfo类**

找到 `#[pyclass(name = "GPUInfo")] pub struct PyGPUInfo { ... }` 和对应的实现块，全部删除。

- [ ] **Step 4: 删除PyMemoryInfo类**

找到 `#[pyclass(name = "MemoryInfo")] pub struct PyMemoryInfo { ... }` 和对应的实现块，全部删除。

- [ ] **Step 5: 运行编译验证**

```bash
cargo build
```

Expected: 编译失败（还有其他类未删除）

- [ ] **Step 6: Commit**

暂不提交，继续Task 5b。

---

### Task 5b: 删除PyNetworkInfo和PySystemInfo类

**Files:**
- Modify: `src/lib.rs`

- [ ] **Step 1: 删除PyNetworkInfo类**

找到 `#[pyclass(name = "NetworkInfo")] pub struct PyNetworkInfo { ... }` 和对应的实现块，全部删除。

- [ ] **Step 2: 删除PyBatteryInfo类**

找到 `#[pyclass(name = "BatteryInfo")] pub struct PyBatteryInfo { ... }` 和对应的实现块，全部删除。

- [ ] **Step 3: 删除PySystemInfo类**

找到 `#[pyclass(name = "SystemInfo")] pub struct PySystemInfo { ... }` 和对应的实现块（包括 `impl From<SystemInfo> for PySystemInfo { ... }`），全部删除。

- [ ] **Step 4: 运行编译验证**

```bash
cargo build
```

Expected: 编译失败（PySample的system getter还在）

- [ ] **Step 5: Commit**

暂不提交，继续Task 5c。

---

### Task 5c: 修改PySample类新增hwinfo_raw getter

**Files:**
- Modify: `src/lib.rs`

- [ ] **Step 1: 删除system getter**

找到 `#[pymethods] impl PySample { ... }`，删除：

```rust
#[getter]
fn system(&self) -> PyResult<PySystemInfo> {
    Ok(PySystemInfo::from(self.inner.system.clone()))
}
```

- [ ] **Step 2: 新增hwinfo_raw getter**

添加：

```rust
#[getter]
fn hwinfo_raw(&self, py: Python<'_>) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    for (name, value) in &self.inner.hwinfo_raw {
        let value_dict = PyDict::new(py);
        value_dict.set_item("value", value.value)?;
        value_dict.set_item("unit", &value.unit)?;
        dict.set_item(name, value_dict)?;
    }
    Ok(dict.into())
}
```

- [ ] **Step 3: 运行编译验证**

```bash
cargo build
```

Expected: 编译失败（monitor.rs还在使用SystemInfo）

- [ ] **Step 4: Commit（Task 5a-5c合并提交）**

```bash
git add src/lib.rs
git commit -m "refactor: 删除PySystemInfo等类，修改PySample新增hwinfo_raw getter"
```

---

### Task 6: 修改monitor.rs采集逻辑

**Files:**
- Modify: `src/monitor.rs`

- [ ] **Step 1: 删除SystemInfo导入**

找到 `use crate::data::{ ... }`，删除 `SystemInfo`。

- [ ] **Step 2: 修改采集逻辑**

找到创建Sample的代码，修改：

```rust
// 旧代码（删除）
let system_info = hwinfo_collector.get_system_info()?;

// 新代码（添加）
let hwinfo_raw = hwinfo_collector.get_all_entries();
```

修改Sample创建：
```rust
Sample {
    timestamp: Utc::now(),
    hwinfo_raw,  // 新增
    // system: system_info,  ← 删除这行
    processes,
    aggregated,
    top_n_cpu,
    top_n_gpu,
}
```

- [ ] **Step 3: 运行编译验证**

```bash
cargo build
```

Expected: 编译成功

- [ ] **Step 4: 运行测试验证**

```bash
cargo test
```

Expected: Rust单元测试通过

- [ ] **Step 5: Commit**

```bash
git add src/monitor.rs
git commit -m "refactor: 修改monitor采集逻辑使用hwinfo_raw"
```

---

### Task 7: 编写单元测试验证同名传感器编号

**Files:**
- Modify: `src/collector/hwinfo.rs`

- [ ] **Step 1: 新增同名传感器编号测试**

在测试模块中添加：

```rust
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
        
        for name in duplicate_names {
            // 编号格式应该是 "#2", "#3" 等
            assert!(name.contains(" #2") || name.contains(" #3"), 
                "同名编号格式正确: {}", name);
        }
        
        println!("测试通过：{}个传感器，{}个同名传感器", 
            entries.len(), duplicate_names.len());
    }
}
```

- [ ] **Step 2: 运行测试验证**

```bash
cargo test test_duplicate_sensor_names -- --nocapture
```

Expected: 测试通过，打印传感器数量

- [ ] **Step 3: Commit**

```bash
git add src/collector/hwinfo.rs
git commit -m "test: 新增同名传感器编号单元测试"
```

---

### Task 8: 更新Python测试代码

**Files:**
- Modify: `tests/test_perfwin.py`

- [ ] **Step 1: 构建并安装模块**

```bash
maturin develop
```

Expected: 构建成功

- [ ] **Step 2: 修改测试代码**

找到测试中的system字段访问，删除或修改：

删除：
```python
assert sample.system is not None
assert sample.system.cpu is not None
```

新增：
```python
# 验证hwinfo_raw字段
assert sample.hwinfo_raw is not None
assert len(sample.hwinfo_raw) > 100  # 至少100个传感器

# 验证数据结构
for name, data in sample.hwinfo_raw.items():
    assert "value" in data
    assert "unit" in data
    assert isinstance(data["value"], (int, float))
    assert isinstance(data["unit"], str)
```

- [ ] **Step 3: 运行测试验证**

```bash
pytest tests/test_perfwin.py -v
```

Expected: 测试通过

- [ ] **Step 4: Commit**

```bash
git add tests/test_perfwin.py
git commit -m "test: 更新Python测试代码验证hwinfo_raw"
```

---

### Task 9: 更新示例代码

**Files:**
- Modify: `examples/basic_usage.py`

- [ ] **Step 1: 修改示例6的system访问**

找到 `example_full_monitoring()` 函数，修改：

删除：
```python
s = sample.system
print(f"\n【系统级数据】")
print(f"  CPU 使用率: {s.cpu.percent:.1f}%")
```

新增：
```python
hwinfo = sample.hwinfo_raw
print(f"\n【HWiNFO 传感器数据】")
print(f"  传感器总数: {len(hwinfo)}")

# 示例：查找特定传感器
cpu_temp = hwinfo.get("CPU Package", {}).get("value")
if cpu_temp:
    print(f"  CPU 温度: {cpu_temp:.1f}°C")

gpu_temp = hwinfo.get("GPU Temperature", {}).get("value")
if gpu_temp:
    print(f"  GPU 温度: {gpu_temp:.1f}°C")

# 打印前20个传感器作为示例
print(f"\n前20个传感器:")
for i, (name, data) in enumerate(hwinfo.items()[:20]):
    print(f"  {name}: {data['value']} {data['unit']}")
```

- [ ] **Step 2: 运行示例验证**

```bash
python examples/basic_usage.py
```

Expected: 示例正常运行，打印所有传感器数据

- [ ] **Step 3: Commit**

```bash
git add examples/basic_usage.py
git commit -m "docs: 更新示例代码展示hwinfo_raw使用"
```

---

### Task 10: 更新版本号和文档

**Files:**
- Modify: `pyproject.toml`
- Modify: `src/lib.rs`
- Modify: `CLAUDE.md`

- [ ] **Step 1: 更新pyproject.toml版本号**

找到 `version = "0.2.2"`，修改为 `version = "0.3.0"`。

- [ ] **Step 2: 更新lib.rs版本号**

找到 `m.add("__version__", "0.2.2")?;`，修改为 `m.add("__version__", "0.3.0")?;`。

- [ ] **Step 3: 更新CLAUDE.md API说明**

修改返回数据结构部分：

删除：
```markdown
### 返回数据结构

Sample:
  timestamp: str
  system: SystemInfo
    cpu: {percent, temperature, power}
    ...
```

新增：
```markdown
### 返回数据结构

Sample:
  timestamp: str              # 采样时间
  hwinfo_raw: Dict[str, Dict] # HWiNFO所有传感器数据
    {"CPU Package": {"value": 60.0, "unit": "°C"}, ...}
  processes: List[ProcessInfo]  # 进程明细（筛选时返回）
  aggregated: List[AggregatedProcessInfo]  # 汇总数据
  top_n_cpu: List[ProcessInfo]  # Top N CPU
  top_n_gpu: List[ProcessInfo]  # Top N GPU

**使用示例**:
```python
# 查找特定传感器
hwinfo = sample.hwinfo_raw
cpu_temp = hwinfo.get("CPU Package", {}).get("value")

# 遍历所有传感器
for name, data in hwinfo.items():
    print(f"{name}: {data['value']} {data['unit']}")
```
```

在文档末尾添加迁移说明：
```markdown
## 版本升级说明 (0.2.x → 0.3.0)

**Breaking Changes**:
- `sample.system` 字段删除，改用 `sample.hwinfo_raw`
- 内存数据（使用率、已用/总量）不再提供，需用psutil获取
- 用户需知道HWiNFO传感器的原始名称

**迁移示例**:
```python
# 旧版（0.2.x）
cpu_temp = sample.system.cpu.temperature

# 新版（0.3.0）
hwinfo = sample.hwinfo_raw
cpu_temp = hwinfo.get("CPU Package", {}).get("value")

# 内存数据替代方案
import psutil
memory_percent = psutil.virtual_memory().percent
```
```

- [ ] **Step 4: 运行完整构建验证**

```bash
cargo clean
maturin build --release
```

Expected: 构建成功

- [ ] **Step 5: Commit**

```bash
git add pyproject.toml src/lib.rs CLAUDE.md
git commit -m "chore: 版本号升级到0.3.0，更新API文档"

git tag v0.3.0
```

---

## 完整性检查

- [ ] **Step 1: 运行完整测试套件**

```bash
cargo test
pytest tests/test_perfwin.py -v
python examples/basic_usage.py
```

Expected: 所有测试通过，示例正常运行

- [ ] **Step 2: 性能验证**

验证：
1. 内存占用：300+ 传感器的内存开销 < 1MB
2. 采集性能：1秒间隔采集的 CPU 占用 < 5%

```bash
python examples/basic_usage.py
```

观察输出，确认传感器数量（预期 > 300）。

- [ ] **Step 3: 检查git状态**

```bash
git status
git log --oneline -15
```

Expected: 无未提交文件，10个新commit

- [ ] **Step 4: 推送到远程（可选）**

```bash
git push origin main --tags
```

---

## 总结

本计划实现HWiNFO原始数据扩展功能，包括：
- 删除配置文件和系统级数据结构
- 新增SensorValue和hwinfo_raw字段
- 实现同名传感器自动编号
- 更新所有测试和文档

每个任务完成后立即commit，遵循TDD和DRY原则。

**Breaking Changes**: sample.system字段删除，改用sample.hwinfo_raw

**预计耗时**: 2-3小时（包含测试和调试）