# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

perfwin 是一个 Rust 编写的 Python 扩展模块，用于 Windows 平台系统性能监控。核心思路是强化版 psutil，提供进程级和系统级的 CPU、GPU、内存、网络等性能数据采集。

**平台限制**: 仅支持 Windows 平台。

## 构建与开发命令

```bash
# 开发模式构建（安装到当前 Python 环境）
maturin develop

# 发布模式构建
maturin build --release

# 运行 Python 测试
pytest tests/test_perfwin.py

# 运行 Rust 单元测试
cargo test

# 运行示例脚本
python examples/basic_usage.py
```

## 核心架构

```
Python 层 (lib.rs)
  └── Monitor (PyO3 绑定类)
        ↓
Rust 层
  └── MonitorCore (monitor.rs)
      ├─ 后台采集线程
      │   ├─ SysinfoCollector (进程级 CPU/内存)
      │   ├─ PdhCollector (进程级 GPU)
      │   ├─ HWiNFOCollector (系统级温度/功耗)
      │
      ├─ RingBuffer (环形缓冲区)
      ├─ HWiNFOManager (生命周期管理，隐藏窗口启动)
```

## 数据采集源

| 数据类型 | 来源 | 模块 |
|---------|------|------|
| 进程级 CPU/内存/句柄 | sysinfo crate | `collector/sysinfo.rs` |
| 进程级 GPU 使用率 | Windows PDH API | `collector/pdh.rs` |
| 系统级温度/功耗 | HWiNFO 共享内存 | `collector/hwinfo.rs` |

## 关键源文件

- `src/lib.rs` - PyO3 绑定入口，定义所有 Python 类（Monitor, ProcessFilter, Sample, AggregatedProcessInfo 等）
- `src/monitor.rs` - MonitorCore 核心实现，管理后台采集线程和汇总计算
- `src/data.rs` - 数据结构定义（CPUInfo, GPUInfo, ProcessInfo, AggregatedProcessInfo, Sample 等）
- `src/collector/mod.rs` - 采集器模块入口
- `src/ring_buffer.rs` - 环形缓冲区实现
- `src/hwinfo_manager.rs` - HWiNFO 进程启动/停止管理（隐藏窗口启动）

## Python API

### 基本使用

```python
import perfwin

# 获取系统所有进程列表
processes = perfwin.list_processes()
for pid, name in processes:
    print(f"{name}: PID={pid}")

# 上下文管理器模式
with perfwin.Monitor(
    interval=1.0,  # 最小值 1 秒
    duration=60,
    process_filter=perfwin.ProcessFilter(name="chrome.exe"),
    top_n_cpu=10,
) as monitor:
    time.sleep(10)
    result = monitor.get_result()

# 手动控制模式
monitor = perfwin.Monitor(interval=1.0)
monitor.start()
time.sleep(10)
monitor.stop()
result = monitor.get_result()
```

### ProcessFilter 筛选模式

- `ProcessFilter(pids=[1234, 5678])` - 按 PID 精确筛选，进程不存在返回占位数据
- `ProcessFilter(name="chrome.exe")` - 按进程名精确匹配，自动追踪新进程
- `ProcessFilter(names=["chrome.exe", "firefox.exe"])` - 多进程名筛选
- `ProcessFilter(name_regex=r"chrome.*")` - 按正则匹配进程名

### 返回数据结构

```python
Sample:
  timestamp: str              # 采样时间
  hwinfo_raw: Dict[str, Dict[str, Any]]  # HWiNFO 原始传感器数据（每次必须返回）
    key: 传感器名称（同名传感器自动编号 "#2", "#3" 等）
    value: {"value": float, "unit": str}  # 传感器值和单位
  processes: List[ProcessInfo]  # 进程明细（筛选时返回）
  aggregated: List[AggregatedProcessInfo]  # 汇总数据（进程名筛选时返回）
  top_n_cpu: List[ProcessInfo]  # Top N CPU（设置参数时返回）
  top_n_gpu: List[ProcessInfo]  # Top N GPU（设置参数时返回）

# 使用示例：
hwinfo = sample.hwinfo_raw
print(f"传感器总数: {len(hwinfo)}")

# 查找特定传感器
cpu_temp = hwinfo.get("CPU Package", {}).get("value")
if cpu_temp:
    print(f"CPU 温度: {cpu_temp:.1f}°C")

# 打印前10个传感器
for name, data in list(hwinfo.items())[:10]:
    print(f"{name}: {data['value']} {data['unit']}")
```

**v0.3.0 Breaking Change**: 删除了 `system` 字段，改用 `hwinfo_raw` 提供原始 HWiNFO 数据。

迁移指南：
- `sample.system.cpu.temperature` → `hwinfo.get("CPU Package", {}).get("value")`
- `sample.system.gpu.temperature` → `hwinfo.get("GPU Temperature", {}).get("value")`
- `sample.system.cpu.power` → `hwinfo.get("CPU Package Power", {}).get("value")`
- 传感器名称可能因 HWiNFO 版本不同而异，建议先打印 `hwinfo_raw.keys()` 查看实际名称

### 参数说明

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| interval | float | 1.0 | 采样间隔（秒），最小值 1.0 |
| duration | float | None | 监控时长（秒），None 表示无限 |
| enable_pdh | bool | True | 是否启用 GPU 采集 |
| enable_sysinfo | bool | True | 是否启用系统信息采集 |
| process_filter | ProcessFilter | None | 进程筛选器 |
| top_n_cpu | int | None | Top N CPU 进程数量 |
| top_n_gpu | int | None | Top N GPU 进程数量 |
| enable_aggregation | bool | True | 是否生成汇总数据 |

## 测试注意事项

- 所有测试仅在 Windows 上运行，非 Windows 平台自动跳过
- 测试前需先用 `maturin develop` 构建并安装模块
- interval 必须 >= 1.0，否则抛出 ValueError
- HWiNFO 强制启用，系统级数据每次必须返回

## HWiNFO 依赖

HWiNFO 用于获取系统级温度和功耗数据，强制启用：
- 模块启动时自动启动 HWiNFO（隐藏窗口，不会置顶）
- 用户需预先配置 HWiNFO 启用共享内存
- HWiNFO 免费版共享内存最长 12 小时失效，模块会自动重启恢复
- hwinfo_path 参数可指定 HWiNFO64.EXE 路径，默认自动搜索

## 版本号管理

**重要**: 修改 Rust 代码后必须更新版本号，否则 pip 安装时不会真正更新。

版本号位置：
- `pyproject.toml` 第 7 行：`version = "0.3.0"`
- `src/lib.rs` 第 953 行：`m.add("__version__", "0.3.0")?;`

更新规则：
- 小修改（bugfix、隐藏窗口等）：`0.1.x` → `0.1.x+1`
- 功能新增：`0.x.y` → `0.x+1.0`
- 重大更新：`x.y.z` → `x+1.0.0`

修改代码后：
1. 同时更新两个文件的版本号
2. `cargo clean && maturin build --release`
3. autotest 项目重新打包时自动使用新版本 wheel