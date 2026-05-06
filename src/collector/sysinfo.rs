// sysinfo 采集器 - 进程级 CPU/内存/句柄

use sysinfo::System;
use crate::data::{ProcessInfo, SystemInfo, CPUInfo, GPUInfo, MemoryInfo, NetworkInfo, BatteryInfo};
use std::time::Instant;

#[cfg(target_os = "windows")]
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX2};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{OpenProcess, GetProcessHandleCount, PROCESS_QUERY_LIMITED_INFORMATION};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::CloseHandle;

/// 进程级数据采集器
pub struct SysinfoCollector {
    sys: System,
    last_refresh_time: Option<Instant>,
    cpu_count: usize,  // CPU 核数，用于计算单核 CPU 使用率
}

impl SysinfoCollector {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        // 初始化时先刷新一次，让后续 cpu_usage() 有正确的基准值
        sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            sysinfo::ProcessRefreshKind::everything()
        );

        // 获取 CPU 核数
        let cpu_count = sys.cpus().len();

        Self {
            sys,
            last_refresh_time: Some(Instant::now()),
            cpu_count,
        }
    }

    /// 刷新进程信息（需要间隔足够才能正确计算CPU使用率）
    pub fn refresh(&mut self) {
        // sysinfo 的 cpu_usage() 需要在两次刷新之间有足够间隔
        // 如果间隔太短（< 100ms），CPU 使用率计算不准确
        let now = Instant::now();
        if let Some(last) = self.last_refresh_time {
            let elapsed = now.duration_since(last);
            if elapsed < std::time::Duration::from_millis(100) {
                // 间隔太短，跳过刷新，避免CPU使用率异常
                return;
            }
        }

        self.sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            sysinfo::ProcessRefreshKind::everything()
        );
        self.last_refresh_time = Some(now);
    }

    /// 获取所有进程信息
    pub fn get_all_processes(&mut self) -> Vec<ProcessInfo> {
        self.refresh();
        self.sys.processes()
            .iter()
            .map(|(pid, proc)| {
                // 使用 Windows API 获取准确的内存数据
                let (working_set_mb, committed_mb, handle_count) = get_process_memory_info(pid.as_u32())
                    .unwrap_or((0.0, 0.0, 0));
                // sysinfo 的 cpu_usage() 返回跨所有核的总使用率
                // 任务管理器显示的是单核占比，需要除以核数
                let cpu_percent = if self.cpu_count > 0 {
                    proc.cpu_usage() as f64 / self.cpu_count as f64
                } else {
                    0.0
                };
                ProcessInfo {
                    pid: pid.as_u32(),
                    name: proc.name().to_string_lossy().to_string(),
                    cpu_percent,
                    working_set_mb,
                    committed_memory_mb: committed_mb,
                    gpu_percent: 0.0,          // PDH 采集
                    gpu_memory_mb: 0.0,        // 不需要
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
            let (working_set_mb, committed_mb, handle_count) = get_process_memory_info(pid)
                .unwrap_or((0.0, 0.0, 0));
            let cpu_percent = if self.cpu_count > 0 {
                proc.cpu_usage() as f64 / self.cpu_count as f64
            } else {
                0.0
            };
            ProcessInfo {
                pid,
                name: proc.name().to_string_lossy().to_string(),
                cpu_percent,
                working_set_mb,
                committed_memory_mb: committed_mb,
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
                let (working_set_mb, committed_mb, handle_count) = get_process_memory_info(pid.as_u32())
                    .unwrap_or((0.0, 0.0, 0));
                let cpu_percent = if self.cpu_count > 0 {
                    proc.cpu_usage() as f64 / self.cpu_count as f64
                } else {
                    0.0
                };
                ProcessInfo {
                    pid: pid.as_u32(),
                    name: proc.name().to_string_lossy().to_string(),
                    cpu_percent,
                    working_set_mb,
                    committed_memory_mb: committed_mb,
                    gpu_percent: 0.0,
                    gpu_memory_mb: 0.0,
                    handle_count,
                }
            })
            .collect()
    }

    /// 获取系统级信息（仅内存，CPU/GPU 使用率由 HWiNFO 提供）
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

        // CPU/GPU 使用率由 HWiNFO 提供，sysinfo 不提供
        SystemInfo {
            cpu: CPUInfo {
                percent: 0.0,        // 由 HWiNFO 提供
                temperature: None,   // 由 HWiNFO 提供
                power: None,         // 由 HWiNFO 提供
                clock_speed: None,   // 由 HWiNFO 提供
            },
            gpu: GPUInfo {
                percent: 0.0,        // 由 HWiNFO 提供
                temperature: None,
                power: None,
                memory_mb: None,
            },
            memory: MemoryInfo {
                percent: memory_percent,
                used_mb: used_memory,
                total_mb: total_memory,
                committed_mb: 0.0,
                committed_limit_mb: 0.0,
            },
            network: NetworkInfo {
                upload_speed: 0.0,   // 由 HWiNFO 提供
                download_speed: 0.0, // 由 HWiNFO 提供
            },
            battery: BatteryInfo {
                charge_level: 0.0,   // 由 HWiNFO 提供
            },
            system_power: 0.0,       // 由 HWiNFO 提供
        }
    }
}

impl Default for SysinfoCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// 获取进程私有工作集、提交内存和句柄数（Windows API）
/// 返回 (private_working_set_mb, committed_mb, handle_count)
/// 私有工作集 = 任务管理器进程页显示的"内存"
#[cfg(target_os = "windows")]
pub fn get_process_memory_info(pid: u32) -> Option<(f64, f64, u32)> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) };
    let handle = handle.ok()?;
    if handle.is_invalid() {
        return None;
    }

    // 使用 EX2 结构体获取私有工作集
    let mut counters = PROCESS_MEMORY_COUNTERS_EX2::default();
    counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32;
    let memory_result = unsafe {
        GetProcessMemoryInfo(handle, &mut counters as *mut _ as *mut _, counters.cb)
    };

    // 获取句柄数
    let mut handle_count = 0u32;
    let handle_result = unsafe { GetProcessHandleCount(handle, &mut handle_count) };

    unsafe { let _ = CloseHandle(handle); };

    if memory_result.is_ok() && handle_result.is_ok() {
        // PrivateWorkingSetSize = 私有工作集，任务管理器显示的 "内存"
        let private_working_set_mb = counters.PrivateWorkingSetSize as f64 / 1024.0 / 1024.0;
        // PrivateUsage = 提交内存（私有内存），任务管理器显示的 "提交大小"
        let committed_mb = counters.PrivateUsage as f64 / 1024.0 / 1024.0;
        Some((private_working_set_mb, committed_mb, handle_count))
    } else {
        None
    }
}

/// 非 Windows 平台的占位实现
#[cfg(not(target_os = "windows"))]
pub fn get_process_memory_info(_pid: u32) -> Option<(f64, f64, u32)> {
    None
}