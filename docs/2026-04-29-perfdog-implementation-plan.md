# Perfdog Rust Python 扩展模块实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 开发一个 Rust Python 扩展模块，提供 Windows 平台系统性能监控（进程级 CPU/GPU/内存 + 系统级 HWiNFO 数据）

**Architecture:** Rust 后台线程定时采集数据，存入环形缓冲区；PyO3 绑定暴露 Python API；HWiNFO 进程自动管理生命周期

**Tech Stack:** PyO3, sysinfo, windows-rs (PDH API), HWiNFO 共享内存

---

## 文件结构概览

```
perfdog/
├── Cargo.toml
├── pyproject.toml
├── src/
│   ├── lib.rs                 # PyO3 入口 + Python 类定义
│   ├── data.rs                # 数据结构定义
│   ├── ring_buffer.rs         # 环形缓冲区
│   ├── collector/
│   │   ├── mod.rs
│   │   ├── sysinfo.rs         # 进程级 CPU/内存/句柄采集
│   │   ├── pdh.rs             # 进程级 GPU 采集 (PDH API)
│   │   └── hwinfo.rs          # HWiNFO 共享内存解析
│   ├── hwinfo_manager.rs      # HWiNFO 进程生命周期管理
│   └── monitor.rs             # Monitor 核心逻辑（后台线程）
├── tests/
│   └── test_perfdog.py        # Python 功能测试
└── examples/
    └── basic_usage.py         # 使用示例
```

---

## Task 1: 项目初始化

**Files:**
- Create: `Cargo.toml`
- Create: `pyproject.toml`
- Create: `src/lib.rs` (空骨架)

- [ ] **Step 1: 创建 Cargo.toml**

```toml
[package]
name = "perfdog"
version = "0.1.0"
edition = "2021"

[lib]
name = "perfdog"
crate-type = ["cdylib"]

[dependencies]
pyo3 = { version = "0.22", features = ["extension-module"] }
sysinfo = "0.31"
windows = { version = "0.58", features = [
    "Win32_Foundation",
    "Win32_System_Performance",
    "Win32_System_ProcessStatus",
    "Win32_System_Threading",
    "Win32_System_Memory",
    "Win32_Storage_FileSystem",
]}
chrono = "0.4"
crossbeam = "0.8"
parking_lot = "0.12"
anyhow = "1.0"
thiserror = "1.0"
regex = "1.10"
itertools = "0.13"  # 用于 sorted_by 等迭代器方法
serde = { version = "1.0", features = ["derive"] }  # 用于数据结构序列化

[build-dependencies]
pyo3-build-config = "0.22"
```

- [ ] **Step 2: 创建 pyproject.toml**

```toml
[build-system]
requires = ["maturin>=1.0"]
build-backend = "maturin"

[project]
name = "perfdog"
version = "0.1.0"
description = "Windows system performance monitoring extension"
requires-python = ">=3.8"
classifiers = [
    "Programming Language :: Rust",
    "Programming Language :: Python :: Implementation :: CPython",
    "Operating System :: Microsoft :: Windows",
]

[tool.maturin]
features = ["pyo3/extension-module"]
```

- [ ] **Step 3: 创建 src/lib.rs 骨架**

```rust
use pyo3::prelude::*;

/// Perfdog Python module
#[pymodule]
fn perfdog(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // 模块初始化 - 后续添加类
    Ok(())
}
```

- [ ] **Step 4: 创建 src 目录结构**

```bash
mkdir -p src/collector tests examples
touch src/data.rs src/ring_buffer.rs src/monitor.rs src/hwinfo_manager.rs
touch src/collector/mod.rs src/collector/sysinfo.rs src/collector/pdh.rs src/collector/hwinfo.rs
```

- [ ] **Step 5: Commit 初始化**

```bash
git add Cargo.toml pyproject.toml src/
git commit -m "init: project structure setup"
```

---

## Task 2: 数据结构定义

**Files:**
- Create: `src/data.rs`

**Dependencies:** Task 1 (需要项目结构)

- [ ] **Step 1: 定义 Rust 数据结构**

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// CPU 系统信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CPUInfo {
    pub percent: f64,
    pub temperature: Option<f64>,
    pub power: Option<f64>,
}

/// GPU 系统信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUInfo {
    pub percent: f64,
    pub temperature: Option<f64>,
    pub power: Option<f64>,
    pub memory_mb: Option<f64>,
}

/// 内存系统信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub percent: f64,
    pub used_mb: f64,
    pub total_mb: f64,
    pub committed_mb: f64,
    pub committed_limit_mb: f64,
}

/// 网络系统信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub upload_speed: f64,      // bytes/s
    pub download_speed: f64,    // bytes/s
}

/// 系统级信息汇总
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub cpu: CPUInfo,
    pub gpu: GPUInfo,
    pub memory: MemoryInfo,
    pub network: NetworkInfo,
}

/// 进程级信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f64,
    pub working_set_mb: f64,        // 工作集内存
    pub committed_memory_mb: f64,   // 提交内存
    pub gpu_percent: f64,
    pub gpu_memory_mb: f64,
    pub handle_count: u32,
}

/// 单次采样数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub timestamp: DateTime<Utc>,
    pub system: Option<SystemInfo>,
    pub processes: Option<Vec<ProcessInfo>>,
    pub top_n_cpu: Option<Vec<ProcessInfo>>,
    pub top_n_gpu: Option<Vec<ProcessInfo>>,
}

/// 进程筛选配置
#[derive(Debug, Clone)]
pub enum ProcessFilter {
    Pids(Vec<u32>),
    Name(String),
    NameRegex(String),
}

/// Monitor 配置参数
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub interval: f64,                  // 秒
    pub duration: Option<f64>,          // 秒，None 表示无限
    pub enable_hwinfo: bool,
    pub enable_pdh: bool,
    pub enable_sysinfo: bool,
    pub hwinfo_path: Option<String>,
    pub process_filter: Option<ProcessFilter>,
    pub top_n_cpu: Option<usize>,
    pub top_n_gpu: Option<usize>,
}
```

- [ ] **Step 2: Commit 数据结构**

```bash
git add src/data.rs
git commit -m "feat: define data structures for system and process info"
```

---

## Task 3: 环形缓冲区实现

**Files:**
- Create: `src/ring_buffer.rs`

**Dependencies:** Task 2 (需要 Sample 数据结构)

- [ ] **Step 1: 实现环形缓冲区**

```rust
use std::collections::VecDeque;
use std::sync::Arc;
use parking_lot::Mutex;
use crate::data::Sample;

/// 环形缓冲区，用于存储采样数据
/// 查询时返回增量数据并清空
/// 使用 Arc 实现跨线程共享
pub struct RingBuffer {
    buffer: Arc<Mutex<VecDeque<Sample>>>,
}

impl RingBuffer {
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// 添加采样数据
    pub fn push(&self, sample: Sample) {
        let mut buf = self.buffer.lock();
        buf.push_back(sample);
    }

    /// 获取所有增量数据并清空
    pub fn drain(&self) -> Vec<Sample> {
        let mut buf = self.buffer.lock();
        buf.drain(..).collect()
    }

    /// 获取当前数据数量
    pub fn len(&self) -> usize {
        self.buffer.lock().len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 克隆共享引用（用于跨线程传递）
    pub fn clone_arc(&self) -> Self {
        Self {
            buffer: Arc::clone(&self.buffer),
        }
    }
}

impl Clone for RingBuffer {
    fn clone(&self) -> Self {
        self.clone_arc()
    }
}

impl Default for RingBuffer {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: Commit 环形缓冲区**

```bash
git add src/ring_buffer.rs
git commit -m "feat: implement ring buffer for sample storage"
```

---

## Task 4: 进程级 CPU/内存/句柄采集

**Files:**
- Create: `src/collector/sysinfo.rs`
- Modify: `src/collector/mod.rs`

**Dependencies:** Task 1 (项目结构), Task 2 (ProcessInfo 数据结构)

- [ ] **Step 1: 创建 sysinfo 采集器**

```rust
use sysinfo::System;
use crate::data::ProcessInfo;

/// 进程级数据采集器
pub struct SysinfoCollector {
    sys: System,
}

impl SysinfoCollector {
    pub fn new() -> Self {
        Self {
            sys: System::new_all(),
        }
    }

    /// 刷新进程信息
    pub fn refresh(&mut self) {
        self.sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            sysinfo::ProcessRefreshKind::everything()
        );
    }

    /// 获取所有进程信息
    pub fn get_all_processes(&mut self) -> Vec<ProcessInfo> {
        self.refresh();
        self.sys.processes()
            .iter()
            .map(|(pid, proc)| ProcessInfo {
                pid: pid.as_u32(),
                name: proc.name().to_string(),
                cpu_percent: proc.cpu_usage() as f64,
                working_set_mb: proc.memory() as f64 / 1024.0 / 1024.0,
                committed_memory_mb: 0.0,  // 需要单独 Windows API
                gpu_percent: 0.0,          // PDH 采集
                gpu_memory_mb: 0.0,        // PDH 采集
                handle_count: 0,           // 需要单独 Windows API
            })
            .collect()
    }

    /// 获取指定 PID 进程信息
    pub fn get_process_by_pid(&mut self, pid: u32) -> Option<ProcessInfo> {
        self.refresh();
        let sysinfo_pid = sysinfo::Pid::from_u32(pid);
        self.sys.process(sysinfo_pid).map(|proc| ProcessInfo {
            pid,
            name: proc.name().to_string(),
            cpu_percent: proc.cpu_usage() as f64,
            working_set_mb: proc.memory() as f64 / 1024.0 / 1024.0,
            committed_memory_mb: 0.0,
            gpu_percent: 0.0,
            gpu_memory_mb: 0.0,
            handle_count: 0,
        })
    }

    /// 按进程名筛选
    pub fn get_processes_by_name(&mut self, name: &str) -> Vec<ProcessInfo> {
        self.refresh();
        self.sys.processes()
            .iter()
            .filter(|(_, proc)| proc.name() == name)
            .map(|(pid, proc)| ProcessInfo {
                pid: pid.as_u32(),
                name: proc.name().to_string(),
                cpu_percent: proc.cpu_usage() as f64,
                working_set_mb: proc.memory() as f64 / 1024.0 / 1024.0,
                committed_memory_mb: 0.0,
                gpu_percent: 0.0,
                gpu_memory_mb: 0.0,
                handle_count: 0,
            })
            .collect()
    }
}

impl Default for SysinfoCollector {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: 添加 Windows API 补充采集（提交内存和句柄数）**

```rust
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::Threading::{OpenProcess, GetProcessHandleCount, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::Foundation::{CloseHandle, BOOL, HANDLE};

/// 获取进程提交内存和句柄数（Windows API）
pub fn get_process_memory_and_handles(pid: u32) -> Option<(f64, u32)> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, BOOL::from(false), pid) };
    if handle.is_invalid() {
        return None;
    }

    // 获取内存信息
    let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
    counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    let memory_result = unsafe { GetProcessMemoryInfo(handle, &mut counters as *mut _ as *mut _, counters.cb) };

    // 获取句柄数
    let mut handle_count = 0u32;
    let handle_result = unsafe { GetProcessHandleCount(handle, &mut handle_count) };

    unsafe { CloseHandle(handle) };

    if memory_result.is_ok() && handle_result.is_ok() {
        let committed_mb = counters.PrivateUsage as f64 / 1024.0 / 1024.0;
        Some((committed_mb, handle_count))
    } else {
        None
    }
}
```

- [ ] **Step 3: 创建 collector/mod.rs**

```rust
pub mod sysinfo;
pub mod pdh;
pub mod hwinfo;
```

- [ ] **Step 4: Commit sysinfo 采集器**

```bash
git add src/collector/
git commit -m "feat: implement sysinfo collector for process CPU/memory"
```

---

## Task 5: 进程级 GPU 采集 (PDH API)

**Files:**
- Create: `src/collector/pdh.rs`

**Dependencies:** Task 1 (项目结构), Task 2 (ProcessInfo 数据结构), Task 4 (collector/mod.rs)

- [ ] **Step 1: 实现 PDH GPU 采集器**

```rust
use windows::Win32::System::Performance::{
    PdhOpenQuery,
    PdhAddCounter,
    PdhCollectQueryData,
    PdhGetFormattedCounterValue,
    PdhCloseQuery,
    PDH_HQUERY,
    PDH_HCOUNTER,
    PDH_FMT_DOUBLE,
    PDH_FMT_COUNTER_VALUE,
};
use windows::core::PCWSTR;
use crate::data::ProcessInfo;
use std::collections::HashMap;

/// GPU 进程级采集器
pub struct PdhCollector {
    query: PDH_HQUERY,
    counters: HashMap<u32, PDH_HCOUNTER>,  // pid -> counter handle
}

impl PdhCollector {
    pub fn new() -> anyhow::Result<Self> {
        let mut query = PDH_HQUERY::default();
        unsafe { PdhOpenQuery(None, 0, &mut query) }
            .map_err(|e| anyhow::anyhow!("PdhOpenQuery failed: {}", e))?;

        Ok(Self {
            query,
            counters: HashMap::new(),
        })
    }

    /// 为指定 PID 添加 GPU counter
    /// GPU counter 路径: \GPU Engine(*engtype_3D)\Utilization Percentage
    /// 需要根据进程 PID 构建正确的 counter 路径
    pub fn add_process_counter(&mut self, pid: u32) -> anyhow::Result<()> {
        let counter_path = format!("\\GPU Engine(pid_{}*)\\Utilization Percentage", pid);
        let path_wide: Vec<u16> = counter_path.encode_utf16().chain(std::iter::once(0)).collect();

        let mut counter = PDH_HCOUNTER::default();
        unsafe { PdhAddCounter(self.query, PCWSTR(path_wide.as_ptr()), 0, &mut counter) }
            .map_err(|e| anyhow::anyhow!("PdhAddCounter failed for pid {}: {}", pid, e))?;

        self.counters.insert(pid, counter);
        Ok(())
    }

    /// 收集 GPU 数据
    pub fn collect(&mut self) -> anyhow::Result<HashMap<u32, f64>> {
        unsafe { PdhCollectQueryData(self.query) }
            .map_err(|e| anyhow::anyhow!("PdhCollectQueryData failed: {}", e))?;

        let mut results = HashMap::new();
        for (pid, counter) in &self.counters {
            let mut value = PDH_FMT_COUNTER_VALUE::default();
            unsafe {
                PdhGetFormattedCounterValue(*counter, PDH_FMT_DOUBLE, None, &mut value)
            }.ok();

            results.insert(*pid, value.doubleValue);
        }

        Ok(results)
    }

    /// 更新进程 GPU 信息
    pub fn update_process_gpu(&mut self, processes: &mut [ProcessInfo]) -> anyhow::Result<()> {
        let gpu_data = self.collect()?;

        for proc in processes.iter_mut() {
            if let Some(gpu_percent) = gpu_data.get(&proc.pid) {
                proc.gpu_percent = *gpu_percent;
            }
        }

        Ok(())
    }
}

impl Drop for PdhCollector {
    fn drop(&mut self) {
        // 正确关闭 PDH query，释放资源
        unsafe { PdhCloseQuery(self.query) };
    }
}
```

- [ ] **Step 2: Commit PDH 采集器**

```bash
git add src/collector/pdh.rs
git commit -m "feat: implement PDH collector for process GPU usage"
```

---

## Task 6: HWiNFO 共享内存解析

**Files:**
- Create: `src/collector/hwinfo.rs`

**Dependencies:** Task 1 (项目结构), Task 2 (SystemInfo 数据结构), Task 4 (collector/mod.rs)

**注意：HWiNFO 共享内存格式需要参考官方文档或逆向分析，以下是框架实现**

- [ ] **Step 1: 实现 HWiNFO 共享内存读取框架**

```rust
use crate::data::{SystemInfo, CPUInfo, GPUInfo, MemoryInfo, NetworkInfo};
use std::ptr;
use windows::Win32::Storage::FileSystem::{
    CreateFileMappingW,
    OpenFileMappingW,
    MapViewOfFile,
    UnmapViewOfFile,
    CloseHandle,
    FILE_MAP_READ,
    PAGE_READONLY,
};
use windows::Win32::Foundation::HANDLE;
use windows::core::PCWSTR;

/// HWiNFO 共享内存名称
const HWINFO_SHARED_MEM_NAME: &str = "HWiNFO_SENS_SM2";

/// HWiNFO 共享内存头结构（需要根据 HWiNFO 文档调整）
#[repr(C)]
struct HWiNFOSharedMemHeader {
    signature: u32,
    version: u32,
    size: u32,
    num_sensors: u32,
    // ... 其他字段
}

/// HWiNFO 传感器数据结构（需要根据实际格式调整）
#[repr(C)]
struct HWiNFOSensorEntry {
    sensor_id: u64,
    value: f64,
    unit: u32,
    name_offset: u32,
}

/// HWiNFO 共享内存采集器
pub struct HWiNFOCollector {
    handle: HANDLE,
    mapped_ptr: *mut u8,
}

impl HWiNFOCollector {
    pub fn new() -> anyhow::Result<Self> {
        let name_wide: Vec<u16> = HWINFO_SHARED_MEM_NAME.encode_utf16().chain(std::iter::once(0)).collect();

        let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, false, PCWSTR(name_wide.as_ptr())) };
        if handle.is_invalid() {
            return Err(anyhow::anyhow!("HWiNFO shared memory not found. Is HWiNFO running?"));
        }

        let mapped_ptr = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0) };
        if mapped_ptr.is_null() {
            unsafe { CloseHandle(handle) };
            return Err(anyhow::anyhow!("Failed to map HWiNFO shared memory"));
        }

        Ok(Self {
            handle,
            mapped_ptr: mapped_ptr as *mut u8,
        })
    }

    /// 检查共享内存是否有效
    pub fn is_valid(&self) -> bool {
        !self.handle.is_invalid() && !self.mapped_ptr.is_null()
    }

    /// 解析系统信息
    /// TODO: 要根据 HWiNFO 共享内存格式详细实现
    pub fn get_system_info(&self) -> anyhow::Result<SystemInfo> {
        if !self.is_valid() {
            return Err(anyhow::anyhow!("HWiNFO shared memory invalid"));
        }

        // 读取共享内存头
        let header = unsafe { *(self.mapped_ptr as *const HWiNFOSharedMemHeader) };

        // 解析传感器数据
        // 需要根据 HWiNFO 文档实现具体解析逻辑
        // 这里返回默认值，实际实现时需要解析

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
}

impl Drop for HWiNFOCollector {
    fn drop(&mut self) {
        if !self.mapped_ptr.is_null() {
            unsafe { UnmapViewOfFile(self.mapped_ptr as *const _) };
        }
        if !self.handle.is_invalid() {
            unsafe { CloseHandle(self.handle) };
        }
    }
}
```

- [ ] **Step 2: Commit HWiNFO 框架**

```bash
git add src/collector/hwinfo.rs
git commit -m "feat: implement HWiNFO shared memory reader framework"
```

---

## Task 7: HWiNFO 进程管理

**Files:**
- Create: `src/hwinfo_manager.rs`

**Dependencies:** Task 1 (项目结构)

- [ ] **Step 1: 实现 HWiNFO 进程生命周期管理**

```rust
use std::process::{Command, Child};
use std::path::PathBuf;
use std::time::Duration;
use anyhow::Result;

/// HWiNFO 进程管理器
pub struct HWiNFOManager {
    process: Option<Child>,
    path: PathBuf,
}

impl HWiNFOManager {
    pub fn new(hwinfo_path: Option<&str>) -> Result<Self> {
        // 默认使用扩展同目录下的 HWiNFO.exe
        let path = if let Some(p) = hwinfo_path {
            PathBuf::from(p)
        } else {
            // 获取扩展模块所在目录
            let exe_dir = std::env::current_exe()?
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Cannot get exe directory"))?
                .to_path_buf();
            exe_dir.join("HWiNFO.exe")
        };

        Ok(Self {
            process: None,
            path,
        })
    }

    /// 启动 HWiNFO（最小化托盘，启用共享内存）
    pub fn start(&mut self) -> Result<()> {
        if self.process.is_some() {
            return Ok(());  // 已经启动
        }

        // HWiNFO 启动参数需要预先配置好
        // 这里假设用户已经配置了共享内存和最小化托盘
        let child = Command::new(&self.path)
            .arg("/minimize")  // 最小化启动
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start HWiNFO: {}", e))?;

        self.process = Some(child);

        // 等待共享内存生效（大约需要几秒）
        std::thread::sleep(Duration::from_secs(3));

        Ok(())
    }

    /// 停止 HWiNFO
    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            process.kill()
                .map_err(|e| anyhow::anyhow!("Failed to kill HWiNFO: {}", e))?;
        }
        Ok(())
    }

    /// 检查 HWiNFO 是否运行
    pub fn is_running(&self) -> bool {
        self.process.as_ref()
            .map(|p| p.try_wait().map(|w| w.is_none()).unwrap_or(false))
            .unwrap_or(false)
    }

    /// 重启 HWiNFO（用于处理 12 小时失效）
    pub fn restart(&mut self) -> Result<()> {
        self.stop()?;
        std::thread::sleep(Duration::from_secs(1));
        self.start()?;
        Ok(())
    }
}

impl Drop for HWiNFOManager {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            eprintln!("Failed to stop HWiNFO on drop: {}", e);
        }
    }
}
```

- [ ] **Step 2: Commit HWiNFO 管理器**

```bash
git add src/hwinfo_manager.rs
git commit -m "feat: implement HWiNFO process lifecycle manager"
```

---

## Task 8: Monitor 核心逻辑

**Files:**
- Create: `src/monitor.rs`

**Dependencies:** Task 2 (数据结构), Task 3 (RingBuffer), Task 4 (sysinfo 采集), Task 5 (PDH 采集), Task 6 (HWiNFO 采集), Task 7 (HWiNFO 管理器)

- [ ] **Step 1: 实现 Monitor 后台采集线程**

```rust
use std::thread::{self, JoinHandle};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use chrono::Utc;
use parking_lot::Mutex;
use itertools::Itertools;  // 用于 sorted_by
use crate::data::{Sample, MonitorConfig, ProcessFilter, ProcessInfo, SystemInfo};
use crate::ring_buffer::RingBuffer;
use crate::collector::{sysinfo::SysinfoCollector, pdh::PdhCollector, hwinfo::HWiNFOCollector};
use crate::hwinfo_manager::HWiNFOManager;
use crate::collector::sysinfo::get_process_memory_and_handles;
use regex::Regex;

/// Monitor 核心
pub struct MonitorCore {
    config: MonitorConfig,
    buffer: RingBuffer,
    running: Arc<AtomicBool>,  // 使用 Arc 支持跨线程共享
    thread: Option<JoinHandle<()>>,
    hwinfo_manager: Mutex<Option<HWiNFOManager>>,
}

impl MonitorCore {
    pub fn new(config: MonitorConfig) -> anyhow::Result<Self> {
        // 启动 HWiNFO（如果需要）
        let hwinfo_manager = if config.enable_hwinfo {
            let mut manager = HWiNFOManager::new(config.hwinfo_path.as_deref())?;
            manager.start()?;
            Some(manager)
        } else {
            None
        };

        Ok(Self {
            config,
            buffer: RingBuffer::new(),
            running: Arc::new(AtomicBool::new(false)),
            thread: None,
            hwinfo_manager: Mutex::new(hwinfo_manager),
        })
    }

    /// 启动采集
    pub fn start(&mut self) -> anyhow::Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());  // 已经运行
        }

        self.running.store(true, Ordering::SeqCst);
        let config = self.config.clone();
        let buffer = self.buffer.clone();
        let running = Arc::clone(&self.running);  // 正确克隆 Arc

        let thread = thread::spawn(move || {
            let mut sysinfo_collector = SysinfoCollector::new();
            let mut pdh_collector = PdhCollector::new().ok();
            let hwinfo_collector = HWiNFOCollector::new().ok();

            let start_time = Instant::now();
            let interval = Duration::from_secs_f64(config.interval);

            while running.load(Ordering::SeqCst) {
                // 检查是否超时
                if let Some(duration) = config.duration {
                    if start_time.elapsed().as_secs_f64() >= duration {
                        running.store(false, Ordering::SeqCst);
                        break;
                    }
                }

                // 采集数据
                let sample = collect_sample(
                    &config,
                    &mut sysinfo_collector,
                    &mut pdh_collector,
                    &hwinfo_collector,
                );

                buffer.push(sample);

                // 等待下一个周期
                thread::sleep(interval);
            }
        });

        self.thread = Some(thread);
        Ok(())
    }

    /// 停止采集
    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.running.store(false, Ordering::SeqCst);

        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }

        // 停止 HWiNFO
        if let Some(manager) = self.hwinfo_manager.lock().take() {
            manager.stop()?;
        }

        Ok(())
    }

    /// 获取增量数据
    pub fn get_result(&self) -> Vec<Sample> {
        self.buffer.drain()
    }
}

/// 采集单次数据
fn collect_sample(
    config: &MonitorConfig,
    sysinfo_collector: &mut SysinfoCollector,
    pdh_collector: &mut Option<PdhCollector>,
    hwinfo_collector: &Option<HWiNFOCollector>,
) -> Sample {
    let timestamp = Utc::now();

    // 系统级数据
    let system = if config.enable_hwinfo {
        hwinfo_collector.as_ref().and_then(|c| c.get_system_info().ok())
    } else {
        None
    };

    // 进程数据
    let processes = if let Some(filter) = &config.process_filter {
        Some(get_filtered_processes(sysinfo_collector, pdh_collector, filter))
    } else {
        None
    };

    // Top N 数据
    let (top_n_cpu, top_n_gpu) = get_top_n_processes(sysinfo_collector, pdh_collector, config);

    Sample {
        timestamp,
        system,
        processes,
        top_n_cpu,
        top_n_gpu,
    }
}

/// 获取筛选后的进程
fn get_filtered_processes(
    sysinfo_collector: &mut SysinfoCollector,
    pdh_collector: &mut Option<PdhCollector>,
    filter: &ProcessFilter,
) -> Vec<ProcessInfo> {
    let mut processes = match filter {
        ProcessFilter::Pids(pids) => {
            pids.iter()
                .map(|pid| {
                    sysinfo_collector.get_process_by_pid(*pid)
                        .unwrap_or_else(|| create_placeholder_process(*pid))
                })
                .collect()
        }
        ProcessFilter::Name(name) => {
            sysinfo_collector.get_processes_by_name(name)
        }
        ProcessFilter::NameRegex(pattern) => {
            let regex = Regex::new(pattern).unwrap();
            sysinfo_collector.get_all_processes()
                .into_iter()
                .filter(|p| regex.is_match(&p.name))
                .collect()
        }
    };

    // 补充提交内存和句柄数
    for proc in &mut processes {
        if let Some((committed, handles)) = get_process_memory_and_handles(proc.pid) {
            proc.committed_memory_mb = committed;
            proc.handle_count = handles;
        }
    }

    // 补充 GPU 数据
    if let Some(pdh) = pdh_collector {
        pdh.update_process_gpu(&mut processes).ok();
    }

    processes
}

/// 获取 Top N 进程
fn get_top_n_processes(
    sysinfo_collector: &mut SysinfoCollector,
    pdh_collector: &mut Option<PdhCollector>,
    config: &MonitorConfig,
) -> (Option<Vec<ProcessInfo>>, Option<Vec<ProcessInfo>>) {
    if config.top_n_cpu.is_none() && config.top_n_gpu.is_none() {
        return (None, None);
    }

    let mut all_processes = sysinfo_collector.get_all_processes();

    // 补充提交内存和句柄数
    for proc in &mut all_processes {
        if let Some((committed, handles)) = get_process_memory_and_handles(proc.pid) {
            proc.committed_memory_mb = committed;
            proc.handle_count = handles;
        }
    }

    // 补充 GPU 数据
    if let Some(pdh) = pdh_collector {
        pdh.update_process_gpu(&mut all_processes).ok();
    }

    let top_n_cpu = config.top_n_cpu.map(|n| {
        all_processes.iter()
            .filter(|p| p.cpu_percent > 0.0)
            .sorted_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap())
            .take(n)
            .cloned()
            .collect()
    });

    let top_n_gpu = config.top_n_gpu.map(|n| {
        all_processes.iter()
            .filter(|p| p.gpu_percent > 0.0)
            .sorted_by(|a, b| b.gpu_percent.partial_cmp(&a.gpu_percent).unwrap())
            .take(n)
            .cloned()
            .collect()
    });

    (top_n_cpu, top_n_gpu)
}

/// 创建占位进程数据（PID 不存在时）
fn create_placeholder_process(pid: u32) -> ProcessInfo {
    ProcessInfo {
        pid,
        name: String::new(),
        cpu_percent: 0.0,
        working_set_mb: 0.0,
        committed_memory_mb: 0.0,
        gpu_percent: 0.0,
        gpu_memory_mb: 0.0,
        handle_count: 0,
    }
}
```

- [ ] **Step 2: Commit Monitor 核心逻辑**

```bash
git add src/monitor.rs
git commit -m "feat: implement Monitor core with background collection thread"
```

---

## Task 9: PyO3 Python 绑定

**Files:**
- Modify: `src/lib.rs`

**Dependencies:** Task 2 (数据结构), Task 8 (MonitorCore)

- [ ] **Step 1: 实现 PyO3 绑定**

```rust
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use chrono::{DateTime, Utc};
use crate::data::{MonitorConfig, ProcessFilter};
use crate::monitor::MonitorCore;

/// ProcessFilter Python 类
#[pyclass]
#[derive(Clone)]
pub struct PyProcessFilter {
    inner: ProcessFilter,
}

#[pymethods]
impl PyProcessFilter {
    #[new]
    #[pyo3(signature = (pids=None, name=None, name_regex=None))]
    fn new(pids: Option<Vec<u32>>, name: Option<String>, name_regex: Option<String>) -> PyResult<Self> {
        let inner = if let Some(pids) = pids {
            ProcessFilter::Pids(pids)
        } else if let Some(name) = name {
            ProcessFilter::Name(name)
        } else if let Some(pattern) = name_regex {
            ProcessFilter::NameRegex(pattern)
        } else {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "Must specify one of: pids, name, or name_regex"
            ));
        };

        Ok(Self { inner })
    }
}

/// Monitor Python 类
#[pyclass]
pub struct PyMonitor {
    core: MonitorCore,  // 直接持有，不再使用 Option
    started: bool,      // 标记是否已启动
}

#[pymethods]
impl PyMonitor {
    #[new]
    #[pyo3(signature = (
        interval=1.0,
        duration=None,
        enable_hwinfo=true,
        enable_pdh=true,
        enable_sysinfo=true,
        hwinfo_path=None,
        process_filter=None,
        top_n_cpu=None,
        top_n_gpu=None,
    ))]
    fn new(
        interval: f64,
        duration: Option<f64>,
        enable_hwinfo: bool,
        enable_pdh: bool,
        enable_sysinfo: bool,
        hwinfo_path: Option<String>,
        process_filter: Option<PyProcessFilter>,
        top_n_cpu: Option<usize>,
        top_n_gpu: Option<usize>,
    ) -> PyResult<Self> {
        let config = MonitorConfig {
            interval,
            duration,
            enable_hwinfo,
            enable_pdh,
            enable_sysinfo,
            hwinfo_path,
            process_filter: process_filter.map(|f| f.inner),
            top_n_cpu,
            top_n_gpu,
        };

        let core = MonitorCore::new(config)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(Self { core, started: false })
    }

    /// 进入上下文管理器 - 返回 self 使 with 语句正确绑定
    fn __enter__(slf: Py<Self>) -> PyResult<Py<Self>> {
        let py = slf.py();
        slf.borrow_mut(py).core.start()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        slf.borrow_mut(py).started = true;
        Ok(slf)
    }

    /// 退出上下文管理器
    fn __exit__(
        slf: Py<Self>,
        _exc_type: &Bound<'_, PyAny>,
        _exc_value: &Bound<'_, PyAny>,
        _traceback: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        let py = slf.py();
        slf.borrow_mut(py).core.stop()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        slf.borrow_mut(py).started = false;
        Ok(false)  // 不抑制异常
    }

    /// 获取增量数据
    fn get_result(slf: Py<Self>) -> PyResult<PyMonitorResult> {
        let py = slf.py();
        let samples = slf.borrow(py).core.get_result();
        Ok(PyMonitorResult { samples })
    }

    /// 手动停止
    fn stop(slf: Py<Self>) -> PyResult<()> {
        let py = slf.py();
        slf.borrow_mut(py).core.stop()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        slf.borrow_mut(py).started = false;
        Ok(())
    }
}

/// MonitorResult Python 类
#[pyclass]
pub struct PyMonitorResult {
    samples: Vec<crate::data::Sample>,
}

#[pymethods]
impl PyMonitorResult {
    /// 获取采样列表
    #[getter]
    fn samples(&self, py: Python<'_>) -> PyResult<Bound<'_, PyList>> {
        let list = PyList::new(py, self.samples.iter().map(|s| PySample::from_sample(s, py)));
        Ok(list)
    }
}

/// Sample Python 类
#[pyclass]
pub struct PySample {
    timestamp: DateTime<Utc>,
    system: Option<PySystemInfo>,
    processes: Option<Vec<PyProcessInfo>>,
    top_n_cpu: Option<Vec<PyProcessInfo>>,
    top_n_gpu: Option<Vec<PyProcessInfo>>,
}

impl PySample {
    fn from_sample(sample: &crate::data::Sample, py: Python<'_>) -> Self {
        Self {
            timestamp: sample.timestamp,
            system: sample.system.as_ref().map(|s| PySystemInfo::from_info(s)),
            processes: sample.processes.as_ref().map(|p| p.iter().map(PyProcessInfo::from_info).collect()),
            top_n_cpu: sample.top_n_cpu.as_ref().map(|p| p.iter().map(PyProcessInfo::from_info).collect()),
            top_n_gpu: sample.top_n_gpu.as_ref().map(|p| p.iter().map(PyProcessInfo::from_info).collect()),
        }
    }
}

#[pymethods]
impl PySample {
    #[getter]
    fn timestamp(&self) -> String {
        self.timestamp.to_rfc3339()
    }

    #[getter]
    fn system(&self) -> Option<PySystemInfo> {
        self.system.clone()
    }

    #[getter]
    fn processes(&self, py: Python<'_>) -> Option<Bound<'_, PyList>> {
        self.processes.as_ref().map(|p| PyList::new(py, p.iter().cloned()))
    }

    #[getter]
    fn top_n_cpu(&self, py: Python<'_>) -> Option<Bound<'_, PyList>> {
        self.top_n_cpu.as_ref().map(|p| PyList::new(py, p.iter().cloned()))
    }

    #[getter]
    fn top_n_gpu(&self, py: Python<'_>) -> Option<Bound<'_, PyList>> {
        self.top_n_gpu.as_ref().map(|p| PyList::new(py, p.iter().cloned()))
    }
}

/// SystemInfo Python 类
#[pyclass]
#[derive(Clone)]
pub struct PySystemInfo {
    cpu: PyCPUInfo,
    gpu: PyGPUInfo,
    memory: PyMemoryInfo,
    network: PyNetworkInfo,
}

impl PySystemInfo {
    fn from_info(info: &crate::data::SystemInfo) -> Self {
        Self {
            cpu: PyCPUInfo::from_info(&info.cpu),
            gpu: PyGPUInfo::from_info(&info.gpu),
            memory: PyMemoryInfo::from_info(&info.memory),
            network: PyNetworkInfo::from_info(&info.network),
        }
    }
}

#[pymethods]
impl PySystemInfo {
    #[getter]
    fn cpu(&self) -> PyCPUInfo { self.cpu.clone() }

    #[getter]
    fn gpu(&self) -> PyGPUInfo { self.gpu.clone() }

    #[getter]
    fn memory(&self) -> PyMemoryInfo { self.memory.clone() }

    #[getter]
    fn network(&self) -> PyNetworkInfo { self.network.clone() }
}

/// CPUInfo Python 类
#[pyclass]
#[derive(Clone)]
pub struct PyCPUInfo {
    percent: f64,
    temperature: Option<f64>,
    power: Option<f64>,
}

impl PyCPUInfo {
    fn from_info(info: &crate::data::CPUInfo) -> Self {
        Self {
            percent: info.percent,
            temperature: info.temperature,
            power: info.power,
        }
    }
}

#[pymethods]
impl PyCPUInfo {
    #[getter]
    fn percent(&self) -> f64 { self.percent }

    #[getter]
    fn temperature(&self) -> Option<f64> { self.temperature }

    #[getter]
    fn power(&self) -> Option<f64> { self.power }
}

/// GPUInfo Python 类
#[pyclass]
#[derive(Clone)]
pub struct PyGPUInfo {
    percent: f64,
    temperature: Option<f64>,
    power: Option<f64>,
    memory_mb: Option<f64>,
}

impl PyGPUInfo {
    fn from_info(info: &crate::data::GPUInfo) -> Self {
        Self {
            percent: info.percent,
            temperature: info.temperature,
            power: info.power,
            memory_mb: info.memory_mb,
        }
    }
}

#[pymethods]
impl PyGPUInfo {
    #[getter]
    fn percent(&self) -> f64 { self.percent }

    #[getter]
    fn temperature(&self) -> Option<f64> { self.temperature }

    #[getter]
    fn power(&self) -> Option<f64> { self.power }

    #[getter]
    fn memory_mb(&self) -> Option<f64> { self.memory_mb }
}

/// MemoryInfo Python 类
#[pyclass]
#[derive(Clone)]
pub struct PyMemoryInfo {
    percent: f64,
    used_mb: f64,
    total_mb: f64,
    committed_mb: f64,
    committed_limit_mb: f64,
}

impl PyMemoryInfo {
    fn from_info(info: &crate::data::MemoryInfo) -> Self {
        Self {
            percent: info.percent,
            used_mb: info.used_mb,
            total_mb: info.total_mb,
            committed_mb: info.committed_mb,
            committed_limit_mb: info.committed_limit_mb,
        }
    }
}

#[pymethods]
impl PyMemoryInfo {
    #[getter]
    fn percent(&self) -> f64 { self.percent }

    #[getter]
    fn used_mb(&self) -> f64 { self.used_mb }

    #[getter]
    fn total_mb(&self) -> f64 { self.total_mb }

    #[getter]
    fn committed_mb(&self) -> f64 { self.committed_mb }

    #[getter]
    fn committed_limit_mb(&self) -> f64 { self.committed_limit_mb }
}

/// NetworkInfo Python 类
#[pyclass]
#[derive(Clone)]
pub struct PyNetworkInfo {
    upload_speed: f64,
    download_speed: f64,
}

impl PyNetworkInfo {
    fn from_info(info: &crate::data::NetworkInfo) -> Self {
        Self {
            upload_speed: info.upload_speed,
            download_speed: info.download_speed,
        }
    }
}

#[pymethods]
impl PyNetworkInfo {
    #[getter]
    fn upload_speed(&self) -> f64 { self.upload_speed }

    #[getter]
    fn download_speed(&self) -> f64 { self.download_speed }
}

/// ProcessInfo Python 类
#[pyclass]
#[derive(Clone)]
pub struct PyProcessInfo {
    pid: u32,
    name: String,
    cpu_percent: f64,
    working_set_mb: f64,
    committed_memory_mb: f64,
    gpu_percent: f64,
    gpu_memory_mb: f64,
    handle_count: u32,
}

impl PyProcessInfo {
    fn from_info(info: &crate::data::ProcessInfo) -> Self {
        Self {
            pid: info.pid,
            name: info.name.clone(),
            cpu_percent: info.cpu_percent,
            working_set_mb: info.working_set_mb,
            committed_memory_mb: info.committed_memory_mb,
            gpu_percent: info.gpu_percent,
            gpu_memory_mb: info.gpu_memory_mb,
            handle_count: info.handle_count,
        }
    }
}

#[pymethods]
impl PyProcessInfo {
    #[getter]
    fn pid(&self) -> u32 { self.pid }

    #[getter]
    fn name(&self) -> &str { &self.name }

    #[getter]
    fn cpu_percent(&self) -> f64 { self.cpu_percent }

    #[getter]
    fn working_set_mb(&self) -> f64 { self.working_set_mb }

    #[getter]
    fn committed_memory_mb(&self) -> f64 { self.committed_memory_mb }

    #[getter]
    fn gpu_percent(&self) -> f64 { self.gpu_percent }

    #[getter]
    fn gpu_memory_mb(&self) -> f64 { self.gpu_memory_mb }

    #[getter]
    fn handle_count(&self) -> u32 { self.handle_count }
}

/// Python 模块入口
#[pymodule]
fn perfdog(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyProcessFilter>()?;
    m.add_class::<PyMonitor>()?;
    m.add_class::<PyMonitorResult>()?;
    m.add_class::<PySample>()?;
    m.add_class::<PySystemInfo>()?;
    m.add_class::<PyCPUInfo>()?;
    m.add_class::<PyGPUInfo>()?;
    m.add_class::<PyMemoryInfo>()?;
    m.add_class::<PyNetworkInfo>()?;
    m.add_class::<PyProcessInfo>()?;
    Ok(())
}
```

- [ ] **Step 2: Commit PyO3 绑定**

```bash
git add src/lib.rs
git commit -m "feat: implement PyO3 bindings for Python API"
```

---

## Task 10: Python 测试

**Files:**
- Create: `tests/test_perfdog.py`

**Dependencies:** Task 9 (PyO3 绑定完成，可构建后测试)

- [ ] **Step 1: 创建基本测试**

```python
import pytest
import time
import sys
import os
import perfdog

def test_monitor_basic():
    """测试基本监控功能"""
    with perfdog.Monitor(
        interval=0.5,
        duration=10,
        enable_hwinfo=False,  # 测试环境可能没有 HWiNFO
        enable_pdh=True,
        enable_sysinfo=True,
        top_n_cpu=5,
    ) as monitor:
        time.sleep(2)  # 等待采集

        result = monitor.get_result()
        assert len(result.samples) > 0

        for sample in result.samples:
            assert sample.timestamp is not None
            assert sample.top_n_cpu is not None
            assert len(sample.top_n_cpu) <= 5

            for proc in sample.top_n_cpu:
                assert proc.pid > 0
                assert proc.name
                assert proc.cpu_percent >= 0

def test_process_filter_by_name():
    """测试按进程名筛选"""
    # 找一个肯定存在的进程
    current_name = os.path.basename(sys.executable).lower()

    with perfdog.Monitor(
        interval=0.5,
        duration=5,
        process_filter=perfdog.ProcessFilter(name=current_name),
    ) as monitor:
        time.sleep(1)

        result = monitor.get_result()
        for sample in result.samples:
            if sample.processes:
                for proc in sample.processes:
                    assert proc.name.lower() == current_name

def test_process_filter_by_pid():
    """测试按 PID 筛选"""
    my_pid = os.getpid()

    with perfdog.Monitor(
        interval=0.5,
        duration=5,
        process_filter=perfdog.ProcessFilter(pids=[my_pid]),
    ) as monitor:
        time.sleep(1)

        result = monitor.get_result()
        for sample in result.samples:
            assert sample.processes is not None
            assert len(sample.processes) == 1
            assert sample.processes[0].pid == my_pid

def test_invalid_pid_returns_placeholder():
    """测试无效 PID 返回占位数据"""
    invalid_pid = 99999999  # 不存在的 PID

    with perfdog.Monitor(
        interval=0.5,
        duration=5,
        process_filter=perfdog.ProcessFilter(pids=[invalid_pid]),
    ) as monitor:
        time.sleep(1)

        result = monitor.get_result()
        for sample in result.samples:
            assert sample.processes is not None
            assert len(sample.processes) == 1
            assert sample.processes[0].pid == invalid_pid
            assert sample.processes[0].cpu_percent == 0.0
            assert sample.processes[0].working_set_mb == 0.0

def test_duration_auto_stop():
    """测试 duration 自动停止"""
    with perfdog.Monitor(
        interval=0.5,
        duration=2,  # 2 秒后自动停止
        top_n_cpu=5,
    ) as monitor:
        time.sleep(3)  # 等待超过 duration

        result = monitor.get_result()
        # duration 后应该没有新数据
        assert len(result.samples) >= 2  # 至少有 2 秒的数据
```

- [ ] **Step 2: 创建使用示例**

```python
# examples/basic_usage.py
import perfdog
import time

def main():
    # 监控特定进程 + 系统 Top 10
    with perfdog.Monitor(
        interval=1.0,
        duration=60,
        process_filter=perfdog.ProcessFilter(name="chrome"),
        top_n_cpu=10,
        top_n_gpu=10,
    ) as monitor:
        for i in range(10):
            time.sleep(5)
            result = monitor.get_result()

            for sample in result.samples:
                print(f"\n时间: {sample.timestamp}")

                # Chrome 进程数据
                if sample.processes:
                    for proc in sample.processes:
                        print(f"  Chrome PID {proc.pid}: CPU={proc.cpu_percent:.1f}%, "
                              f"内存={proc.working_set_mb:.1f}MB")

                # 系统 CPU 前 10
                if sample.top_n_cpu:
                    print("  系统 CPU 占用前 10:")
                    for proc in sample.top_n_cpu:
                        print(f"    {proc.name}({proc.pid}): {proc.cpu_percent:.1f}%")

                # 系统信息
                if sample.system:
                    print(f"  CPU 温度: {sample.system.cpu.temperature}°C")
                    print(f"  GPU 使用率: {sample.system.gpu.percent}%")

if __name__ == "__main__":
    main()
```

- [ ] **Step 3: Commit 测试和示例**

```bash
git add tests/ examples/
git commit -m "feat: add Python tests and usage examples"
```

---

## Task 11: 构建和打包

**Files:**
- Build: 使用 maturin 构建

**Dependencies:** Task 10 (所有代码和测试完成)

- [ ] **Step 1: 安装构建工具**

```bash
pip install maturin
```

- [ ] **Step 2: 构建**

```bash
maturin develop  # 开发模式，安装到当前 Python 环境
```

或发布构建：

```bash
maturin build --release  # 生成 wheel 文件
```

- [ ] **Step 3: 运行测试**

```bash
pytest tests/test_perfdog.py -v
```

- [ ] **Step 4: Final Commit**

```bash
git add .
git commit -m "feat: complete perfdog Rust Python extension module"
```

---

## 注意事项

1. **HWiNFO 共享内存格式**：需要参考 HWiNFO 官方文档或逆向分析共享内存结构，Task 6 中的结构是框架，需要完善。

2. **PDH GPU Counter 路径**：`\GPU Engine(pid_*)\Utilization Percentage` 的具体格式可能因显卡驱动不同，需要测试验证。

3. **Windows 平台限制**：所有代码仅能在 Windows 上编译和运行，macOS/Linux 开发时需要用 Windows 机器或 CI 进行构建测试。

4. **HWiNFO 配置**：用户需要预先配置 HWiNFO 启用共享内存输出和最小化托盘启动。

---

## 待后续完善

1. HWiNFO 共享内存解析的完整实现
2. PDH GPU counter 的多显卡支持
3. 进程名正则筛选的实时追踪新进程
4. 错误处理和日志
5. 文档和 API 说明