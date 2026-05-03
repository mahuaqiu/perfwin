// sysinfo 采集器 - 进程级 CPU/内存/句柄

use sysinfo::System;
use crate::data::{ProcessInfo, SystemInfo, CPUInfo, GPUInfo, MemoryInfo, NetworkInfo};

#[cfg(target_os = "windows")]
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{OpenProcess, GetProcessHandleCount, PROCESS_QUERY_LIMITED_INFORMATION};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::CloseHandle;

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
            sysinfo::ProcessRefreshKind::everything()
        );
    }

    /// 获取所有进程信息
    pub fn get_all_processes(&mut self) -> Vec<ProcessInfo> {
        self.refresh();
        self.sys.processes()
            .iter()
            .map(|(pid, proc)| {
                let (committed_memory_mb, handle_count) = get_process_memory_and_handles(pid.as_u32())
                    .unwrap_or((0.0, 0));
                ProcessInfo {
                    pid: pid.as_u32(),
                    name: proc.name().to_string_lossy().to_string(),
                    cpu_percent: proc.cpu_usage() as f64,
                    working_set_mb: proc.memory() as f64 / 1024.0 / 1024.0,
                    committed_memory_mb,
                    gpu_percent: 0.0,          // PDH 采集
                    gpu_memory_mb: 0.0,        // PDH 采集
                    handle_count,
                }
            })
            .collect()
    }

    /// 获取指定 PID 进程信息
    pub fn get_process_by_pid(&mut self, pid: u32) -> Option<ProcessInfo> {
        self.refresh();
        let sysinfo_pid = sysinfo::Pid::from_u32(pid);
        self.sys.process(sysinfo_pid).map(|proc| {
            let (committed_memory_mb, handle_count) = get_process_memory_and_handles(pid)
                .unwrap_or((0.0, 0));
            ProcessInfo {
                pid,
                name: proc.name().to_string_lossy().to_string(),
                cpu_percent: proc.cpu_usage() as f64,
                working_set_mb: proc.memory() as f64 / 1024.0 / 1024.0,
                committed_memory_mb,
                gpu_percent: 0.0,
                gpu_memory_mb: 0.0,
                handle_count,
            }
        })
    }

    /// 按进程名筛选
    pub fn get_processes_by_name(&mut self, name: &str) -> Vec<ProcessInfo> {
        self.refresh();
        self.sys.processes()
            .iter()
            .filter(|(_, proc)| proc.name() == name)
            .map(|(pid, proc)| {
                let (committed_memory_mb, handle_count) = get_process_memory_and_handles(pid.as_u32())
                    .unwrap_or((0.0, 0));
                ProcessInfo {
                    pid: pid.as_u32(),
                    name: proc.name().to_string_lossy().to_string(),
                    cpu_percent: proc.cpu_usage() as f64,
                    working_set_mb: proc.memory() as f64 / 1024.0 / 1024.0,
                    committed_memory_mb,
                    gpu_percent: 0.0,
                    gpu_memory_mb: 0.0,
                    handle_count,
                }
            })
            .collect()
    }

    /// 获取系统级信息（内存）
    pub fn get_system_info(&mut self) -> SystemInfo {
        self.refresh();

        // 刷新内存信息
        self.sys.refresh_memory_specifics(
            sysinfo::MemoryRefreshKind::everything()
        );

        let total_memory = self.sys.total_memory() as f64 / 1024.0 / 1024.0;  // MB
        let used_memory = self.sys.used_memory() as f64 / 1024.0 / 1024.0;    // MB
        let memory_percent = if total_memory > 0.0 {
            used_memory / total_memory * 100.0
        } else {
            0.0
        };

        SystemInfo {
            cpu: CPUInfo {
                percent: 0.0,  // 由 HWiNFO 或其他来源提供
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
                percent: memory_percent,
                used_mb: used_memory,
                total_mb: total_memory,
                committed_mb: 0.0,  // 可从 Windows API 获取
                committed_limit_mb: 0.0,
            },
            network: NetworkInfo {
                upload_speed: 0.0,
                download_speed: 0.0,
            },
        }
    }
}

impl Default for SysinfoCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// 获取进程提交内存和句柄数（Windows API）
#[cfg(target_os = "windows")]
pub fn get_process_memory_and_handles(pid: u32) -> Option<(f64, u32)> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) };
    let handle = handle.ok()?;
    if handle.is_invalid() {
        return None;
    }

    // 获取内存信息
    let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
    counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    let memory_result = unsafe {
        GetProcessMemoryInfo(handle, &mut counters as *mut _ as *mut _, counters.cb)
    };

    // 获取句柄数
    let mut handle_count = 0u32;
    let handle_result = unsafe { GetProcessHandleCount(handle, &mut handle_count) };

    unsafe { let _ = CloseHandle(handle); };

    if memory_result.is_ok() && handle_result.is_ok() {
        let committed_mb = counters.PrivateUsage as f64 / 1024.0 / 1024.0;
        Some((committed_mb, handle_count))
    } else {
        None
    }
}

/// 非 Windows 平台的占位实现
#[cfg(not(target_os = "windows"))]
pub fn get_process_memory_and_handles(_pid: u32) -> Option<(f64, u32)> {
    // 在非 Windows 平台上，提交内存和句柄数不可用
    None
}