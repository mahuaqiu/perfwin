---
title: Perfdog - Rust Python 扩展模块设计
date: 2026-04-29
status: draft
---

# Perfdog - 系统性能监控扩展模块

## 项目概述

开发一个 Rust 编写的 Python 扩展模块，用于 Windows 平台系统性能监控。核心思路是强化版 psutil，提供进程级和系统级的 CPU、GPU、内存、网络等性能数据采集。

### 核心数据来源

1. **Windows PDH API**：获取进程级 GPU 使用率
2. **sysinfo crate**：获取进程级 CPU、内存、提交内存、句柄数
3. **HWiNFO 共享内存**：获取系统级 CPU/GPU/内存/温度/功耗/网络速度

### 平台限制

- 仅支持 Windows 平台
- 依赖 HWiNFO.exe（用户需放置在扩展同目录）
- HWiNFO 免费版共享内存最长 12 小时失效，需自动重启处理

---

## 整体架构

```
┌─────────────────────────────────────────────────────┐
│                    Python 层                         │
│  ┌─────────────────────────────────────────────┐    │
│  │  Monitor (PyO3 绑定)                         │    │
│  │  - 初始化参数配置                             │    │
│  │  - get_result() 获取增量数据                 │    │
│  │  - stop() 手动停止                           │    │
│  └─────────────────────────────────────────────┘    │
│                      ↓ pyo3                         │
├─────────────────────────────────────────────────────┤
│                    Rust 层                          │
│  ┌─────────────────────────────────────────────┐    │
│  │  MonitorCore                                 │    │
│  │  ├─ CollectorThread (后台采集线程)          │    │
│  │  │   ├─ PDHCollector (进程级 GPU)           │    │
│  │  │   ├─ SysinfoCollector (进程级 CPU/内存)   │    │
│  │  │   ├─ HWiNFOCollector (系统级数据)        │    │
│  │  │   └─────────────────────────────────────│    │
│  │  ├─ RingBuffer (环形缓冲区存储采样数据)      │    │
│  │  ├─ HWiNFOManager (生命周期管理)            │    │
│  │  │   ├─ 启动 HWiNFO.exe                     │    │
│  │  │   ├─ 检测共享内存失效                    │    │
│  │  │   ├─ 自动重启                            │    │
│  │  │   ├─ stop() 时杀掉进程                   │    │
│  │  └─────────────────────────────────────────│    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

---

## Python API 设计

### Monitor 类

```python
import perfdog

with perfdog.Monitor(
    interval: float = 1.0,              # 采集周期（秒）
    duration: Optional[float] = None,   # 最大采集时间（秒），None 表示无限
    enable_hwinfo: bool = True,         # 启用 HWiNFO 系统级数据
    enable_pdh: bool = True,            # 启用进程级 GPU（PDH）
    enable_sysinfo: bool = True,        # 启用进程级 CPU/内存
    hwinfo_path: Optional[str] = None,  # HWiNFO.exe 路径，None 使用同目录

    # 进程筛选
    process_filter: Optional[ProcessFilter] = None,

    # Top N 配置
    top_n_cpu: Optional[int] = None,    # 返回系统 CPU Top N 进程
    top_n_gpu: Optional[int] = None,    # 返回系统 GPU Top N 进程
) as monitor:

    # 获取增量数据（查询后清空缓存）
    result = monitor.get_result()

    # 手动停止（可选，with 结束会自动停止）
    monitor.stop()
```

### ProcessFilter 类

```python
from dataclasses import dataclass
from typing import Optional, List

@dataclass
class ProcessFilter:
    pids: Optional[List[int]] = None       # 固定 PID 列表
    name: Optional[str] = None             # 进程名（持续追踪新进程）
    name_regex: Optional[str] = None       # 正则匹配进程名
```

---

## 数据结构定义

### 系统级数据

```python
@dataclass
class CPUInfo:
    percent: float
    temperature: Optional[float] = None
    power: Optional[float] = None

@dataclass
class GPUInfo:
    percent: float
    temperature: Optional[float] = None
    power: Optional[float] = None
    memory_mb: Optional[float] = None

@dataclass
class MemoryInfo:
    percent: float
    used_mb: float
    total_mb: float
    committed_mb: float
    committed_limit_mb: float

@dataclass
class NetworkInfo:
    upload_speed: float      # bytes/s
    download_speed: float    # bytes/s

@dataclass
class SystemInfo:
    cpu: CPUInfo
    gpu: GPUInfo
    memory: MemoryInfo
    network: NetworkInfo
```

### 进程级数据

```python
@dataclass
class ProcessInfo:
    pid: int
    name: str
    cpu_percent: float

    # 内存（两种）
    working_set_mb: float           # 任务管理器显示的内存（工作集）
    committed_memory_mb: float      # 提交内存

    gpu_percent: float
    gpu_memory_mb: float

    handle_count: int               # 进程句柄数量
```

### 采样结果

```python
from datetime import datetime

@dataclass
class Sample:
    timestamp: datetime
    system: Optional[SystemInfo] = None            # 有 enable_hwinfo 时才有

    processes: Optional[List[ProcessInfo]] = None  # 有 process_filter 时才有
    top_n_cpu: Optional[List[ProcessInfo]] = None  # 有 top_n_cpu 时才有
    top_n_gpu: Optional[List[ProcessInfo]] = None  # 有 top_n_gpu 时才有

@dataclass
class MonitorResult:
    samples: List[Sample]      # 增量采样数据，查询后清空
```

---

## 核心功能行为

### HWiNFO 生命周期管理

| 操作 | 触发条件 | 行为 |
|------|---------|------|
| 启动 | 初始化时 enable_hwinfo=True | 启动同目录 HWiNFO.exe |
| 检测失效 | 共享内存读取失败 | 自动重启 HWiNFO |
| 停止 | stop() 调用或 with 退出 | 杀掉 HWiNFO 进程 |

**注意事项**：
- 用户需预先配置 HWiNFO 启用共享内存、最小化托盘启动
- HWiNFO 免费版共享内存最长 12 小时，失效后自动重启恢复

### 进程筛选行为

| 筛选方式 | 进程不存在 | 进程退出 | 新进程启动 |
|---------|-----------|---------|-----------|
| `pids=[1234]` | 返回占位数据 (cpu=0, memory=0) | 返回占位数据 | 不追踪 |
| `name="chrome"` | 返回空列表 [] | 返回空列表 | 自动追踪新 PID |
| `name_regex` | 返回空列表 [] | 返回空列表 | 自动追踪匹配的新进程 |

### 数据采集模式

- **后台定时采集**：Rust 后台线程按 interval 参数定时采集
- **增量返回**：Python 调用 get_result() 返回缓存数据，查询后清空
- **时间戳**：每个 Sample 带采集时间戳
- **最大时长**：duration 到期自动停止采集

---

## Rust 技术选型

| 功能 | 技术方案 |
|------|---------|
| Python 绑定 | PyO3 crate |
| 进程级 CPU/内存/句柄 | sysinfo crate + Windows API |
| 进程级 GPU | Windows PDH API (windows-rs) |
| 系统级数据 | HWiNFO 共享内存解析 |
| 后台线程 | std::thread + crossbeam channel |
| 数据缓存 | 环形缓冲区 (VecDeque 或自定义) |

### Windows API 调用

- `GetProcessMemoryInfo`：获取 WorkingSetSize（工作集内存）和 PrivateUsage（提交内存）
- `GetProcessHandleCount`：获取句柄数
- `PdhCollectQueryData` + `PdhGetFormattedCounterValue`：获取进程 GPU 使用率

### HWiNFO 共享内存

- 需解析 HWiNFO 共享内存格式（结构定义需参考 HWiNFO 文档）
- 内存映射文件读取方式访问

---

## 使用示例

### 基本使用

```python
import perfdog

# 监控特定进程 + 系统 Top 10
with perfdog.Monitor(
    interval=1.0,
    duration=3600,
    process_filter=perfdog.ProcessFilter(name="chrome"),
    top_n_cpu=10,
    top_n_gpu=10,
) as monitor:
    while True:
        result = monitor.get_result()

        for sample in result.samples:
            # Chrome 进程数据
            for proc in sample.processes:
                print(f"Chrome PID {proc.pid}: CPU={proc.cpu_percent}%")

            # 系统 CPU 前 10
            for proc in sample.top_n_cpu:
                print(f"Top CPU: {proc.name}: {proc.cpu_percent}%")

        time.sleep(5)
```

### 仅监控系统状态

```python
with perfdog.Monitor(
    interval=0.5,
    top_n_cpu=10,
    top_n_gpu=10,
) as monitor:
    result = monitor.get_result()

    for sample in result.samples:
        print(f"CPU: {sample.system.cpu.percent}%")
        print(f"GPU Temp: {sample.system.gpu.temperature}°C")
```

---

## 文件结构

```
perfdog/
├── src/
│   ├── lib.rs                 # PyO3 入口
│   ├── monitor.rs             # Monitor 类实现
│   ├── collector/
│   │   ├── mod.rs
│   │   ├── pdh.rs             # PDH GPU 采集
│   │   ├── sysinfo.rs         # sysinfo 采集
│   │   └── hwinfo.rs          # HWiNFO 共享内存
│   ├── hwinfo_manager.rs      # HWiNFO 进程管理
│   ├── data.rs                # 数据结构定义
│   └── ring_buffer.rs         # 环形缓冲区
├── Cargo.toml
├── pyproject.toml             # Python 打包配置
└── tests/
    └── test_monitor.py
```

---

## 待确认事项

1. HWiNFO 共享内存格式需要详细文档或逆向分析
2. GPU 进程级数据的具体 PDH counter 名称
3. Python 包名确认（perfdog 或其他）