# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

perfdog 是一个 Rust 编写的 Python 扩展模块，用于 Windows 平台系统性能监控。核心思路是强化版 psutil，提供进程级和系统级的 CPU、GPU、内存、网络等性能数据采集。

**平台限制**: 仅支持 Windows 平台。

## 构建与开发命令

```bash
# 开发模式构建（安装到当前 Python 环境）
maturin develop

# 发布模式构建
maturin build --release

# 运行 Python 测试
pytest

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
      ├─ HWiNFOManager (生命周期管理)
```

## 数据采集源

| 数据类型 | 来源 | 模块 |
|---------|------|------|
| 进程级 CPU/内存/句柄 | sysinfo crate | `collector/sysinfo.rs` |
| 进程级 GPU 使用率 | Windows PDH API | `collector/pdh.rs` |
| 系统级温度/功耗 | HWiNFO 共享内存 | `collector/hwinfo.rs` |

## 关键源文件

- `src/lib.rs` - PyO3 绑定入口，定义所有 Python 类（Monitor, ProcessFilter, Sample 等）
- `src/monitor.rs` - MonitorCore 核心实现，管理后台采集线程
- `src/data.rs` - 数据结构定义（CPUInfo, GPUInfo, ProcessInfo, Sample 等）
- `src/collector/mod.rs` - 采集器模块入口
- `src/ring_buffer.rs` - 环形缓冲区实现
- `src/hwinfo_manager.rs` - HWiNFO 进程启动/停止管理

## Python API 使用模式

```python
import perfdog

# 上下文管理器模式
with perfdog.Monitor(
    interval=1.0,
    duration=60,
    process_filter=perfdog.ProcessFilter(name="chrome.exe"),
    top_n_cpu=10,
) as monitor:
    time.sleep(10)
    result = monitor.get_result()

# 手动控制模式
monitor = perfdog.Monitor(interval=0.5)
monitor.start()
time.sleep(10)
monitor.stop()
result = monitor.get_result()
```

## ProcessFilter 筛选模式

- `ProcessFilter(pids=[1234, 5678])` - 按 PID 精确筛选，进程不存在返回占位数据
- `ProcessFilter(name="chrome.exe")` - 按进程名精确匹配，自动追踪新进程
- `ProcessFilter(name_regex=r"chrome.*")` - 按正则匹配进程名

## 测试注意事项

- 所有测试仅在 Windows 上运行，非 Windows 平台自动跳过
- 测试前需先用 `maturin develop` 构建并安装模块
- `enable_hwinfo=False` 用于测试环境（可能没有 HWiNFO）

## HWiNFO 依赖

HWiNFO 是可选依赖，用于获取系统级温度和功耗数据：
- 用户需预先配置 HWiNFO 启用共享内存
- HWiNFO 免费版共享内存最长 12 小时失效，模块会自动重启恢复
- `enable_hwinfo=False` 时模块仍可正常工作（不提供温度/功耗数据）