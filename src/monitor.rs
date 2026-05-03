use std::thread::{self, JoinHandle};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use chrono::Utc;
use parking_lot::Mutex;
use itertools::Itertools;

use crate::data::{Sample, MonitorConfig, ProcessFilter, ProcessInfo};
use crate::ring_buffer::RingBuffer;
use crate::collector::{sysinfo::SysinfoCollector, pdh::PdhCollector, hwinfo::HWiNFOCollector};
use crate::hwinfo_manager::HWiNFOManager;

/// Monitor 核心结构
///
/// 负责管理后台采集线程，协调各采集器收集系统性能数据
pub struct MonitorCore {
    /// 监控配置
    config: MonitorConfig,
    /// 环形缓冲区，存储采样数据
    buffer: RingBuffer,
    /// 运行标志，用于控制线程启停
    running: Arc<AtomicBool>,
    /// 后台采集线程句柄
    thread: Option<JoinHandle<()>>,
    /// HWiNFO 进程管理器
    hwinfo_manager: Mutex<Option<HWiNFOManager>>,
}

impl MonitorCore {
    /// 创建新的 Monitor 实例
    ///
    /// # 参数
    /// - `config`: 监控配置参数
    ///
    /// # 返回
    /// 成功返回 MonitorCore 实例，失败返回错误
    pub fn new(config: MonitorConfig) -> anyhow::Result<Self> {
        // 如果启用了 HWiNFO，尝试连接或启动
        let hwinfo_manager = if config.enable_hwinfo {
            // 先尝试直接连接共享内存（HWiNFO 可能已在运行）
            if HWiNFOCollector::new().is_ok() {
                // HWiNFO 已在运行，不需要启动进程
                None
            } else {
                // HWiNFO 未运行，尝试启动进程
                let mut manager = HWiNFOManager::new(config.hwinfo_path.as_deref())?;
                if let Err(e) = manager.start() {
                    // 启动失败，打印警告但不报错
                    // 用户可能需要手动启动 HWiNFO（需要管理员权限）
                    eprintln!("警告: 无法自动启动 HWiNFO: {}", e);
                    eprintln!("请手动启动 HWiNFO64 并启用共享内存功能，或使用 hwinfo_path 参数指定正确路径。");
                    None
                } else {
                    Some(manager)
                }
            }
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

    /// 启动后台采集线程
    ///
    /// # 返回
    /// 成功返回 Ok(())，如果已经在运行也返回 Ok(())
    pub fn start(&mut self) -> anyhow::Result<()> {
        // 检查是否已经在运行
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        // 设置运行标志
        self.running.store(true, Ordering::SeqCst);

        // 克隆配置和共享资源用于线程
        let config = self.config.clone();
        let buffer = self.buffer.clone();
        let running = Arc::clone(&self.running);

        // 启动后台采集线程
        let thread = thread::spawn(move || {
            // 初始化采集器
            let mut sysinfo_collector = SysinfoCollector::new();
            let mut pdh_collector = if config.enable_pdh {
                PdhCollector::new().ok()
            } else {
                None
            };
            let hwinfo_collector = if config.enable_hwinfo {
                HWiNFOCollector::new().ok()
            } else {
                None
            };

            let start_time = Instant::now();
            let interval = Duration::from_secs_f64(config.interval);

            // 主采集循环
            while running.load(Ordering::SeqCst) {
                // 检查是否超时
                if let Some(duration) = config.duration {
                    if start_time.elapsed().as_secs_f64() >= duration {
                        running.store(false, Ordering::SeqCst);
                        break;
                    }
                }

                // 收集采样数据
                let sample = collect_sample(
                    &config,
                    &mut sysinfo_collector,
                    &mut pdh_collector,
                    &hwinfo_collector,
                );

                // 存入缓冲区
                buffer.push(sample);

                // 等待下一个采集周期
                thread::sleep(interval);
            }
        });

        self.thread = Some(thread);
        Ok(())
    }

    /// 停止后台采集线程
    ///
    /// # 返回
    /// 成功返回 Ok(())
    pub fn stop(&mut self) -> anyhow::Result<()> {
        // 设置停止标志
        self.running.store(false, Ordering::SeqCst);

        // 等待线程结束
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }

        // 停止 HWiNFO 管理器
        if let Some(mut manager) = self.hwinfo_manager.lock().take() {
            manager.stop()?;
        }

        Ok(())
    }

    /// 获取采集结果
    ///
    /// 返回缓冲区中的所有数据并清空缓冲区
    pub fn get_result(&self) -> Vec<Sample> {
        self.buffer.drain()
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// 获取缓冲区中的数据数量
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }
}

impl Drop for MonitorCore {
    fn drop(&mut self) {
        // 确保 Stop 被调用
        let _ = self.stop();
    }
}

/// 收集一次采样数据
///
/// # 参数
/// - `config`: 监控配置
/// - `sysinfo_collector`: sysinfo 采集器
/// - `pdh_collector`: PDH 采集器（可选）
/// - `hwinfo_collector`: HWiNFO 采集器（可选）
///
/// # 返回
/// 返回一个 Sample 实例
fn collect_sample(
    config: &MonitorConfig,
    sysinfo_collector: &mut SysinfoCollector,
    pdh_collector: &mut Option<PdhCollector>,
    hwinfo_collector: &Option<HWiNFOCollector>,
) -> Sample {
    // 收集系统级信息
    let system = if config.enable_sysinfo {
        // 从 sysinfo 获取基础数据
        let mut system_info = sysinfo_collector.get_system_info();

        // 从 HWiNFO 补充温度、功耗数据
        if let Some(hwinfo) = hwinfo_collector {
            if let Ok(hwinfo_data) = hwinfo.get_system_info() {
                // 合并 HWiNFO 的 CPU/GPU 数据（温度、功耗）
                system_info.cpu.temperature = hwinfo_data.cpu.temperature;
                system_info.cpu.power = hwinfo_data.cpu.power;
                system_info.gpu.temperature = hwinfo_data.gpu.temperature;
                system_info.gpu.power = hwinfo_data.gpu.power;
                // 如果 HWiNFO 有 CPU/GPU 使用率数据，也使用它
                if hwinfo_data.cpu.percent > 0.0 {
                    system_info.cpu.percent = hwinfo_data.cpu.percent;
                }
                if hwinfo_data.gpu.percent > 0.0 {
                    system_info.gpu.percent = hwinfo_data.gpu.percent;
                }
                // 网络速度
                if hwinfo_data.network.upload_speed > 0.0 || hwinfo_data.network.download_speed > 0.0 {
                    system_info.network = hwinfo_data.network;
                }
                // 电池电量
                if hwinfo_data.battery.charge_level > 0.0 {
                    system_info.battery = hwinfo_data.battery;
                }
                // 系统总功耗
                if hwinfo_data.system_power > 0.0 {
                    system_info.system_power = hwinfo_data.system_power;
                }
            }
        }

        Some(system_info)
    } else {
        None
    };

    // 收集进程级信息
    let processes = if config.process_filter.is_some() {
        let mut procs = get_filtered_processes(config, sysinfo_collector);

        // 为目标进程添加 PDH counter 并更新 GPU 数据
        if let Some(pdh) = pdh_collector {
            // 提取 PID 列表并添加 counters
            let pids: Vec<u32> = procs.iter().map(|p| p.pid).collect();
            pdh.add_process_counters(&pids);
            let _ = pdh.update_process_gpu(&mut procs);
        }

        Some(procs)
    } else {
        None
    };

    // 获取 Top N 进程
    let top_n_cpu = config.top_n_cpu.and_then(|n| {
        let mut procs = get_top_n_processes(sysinfo_collector, n, |p| p.cpu_percent);
        // 为 top N CPU 进程添加 PDH counter 并更新 GPU 数据
        if let Some(pdh) = pdh_collector {
            let pids: Vec<u32> = procs.iter().map(|p| p.pid).collect();
            pdh.add_process_counters(&pids);
            let _ = pdh.update_process_gpu(&mut procs);
        }
        Some(procs)
    });

    let top_n_gpu = config.top_n_gpu.and_then(|n| {
        let mut procs = get_top_n_processes(sysinfo_collector, n, |p| p.gpu_percent);
        // 为 top N GPU 进程添加 PDH counter 并更新 GPU 数据
        if let Some(pdh) = pdh_collector {
            let pids: Vec<u32> = procs.iter().map(|p| p.pid).collect();
            pdh.add_process_counters(&pids);
            let _ = pdh.update_process_gpu(&mut procs);
        }
        Some(procs)
    });

    Sample {
        timestamp: Utc::now(),
        system,
        processes,
        top_n_cpu,
        top_n_gpu,
    }
}

/// 根据进程过滤器获取目标进程列表
///
/// # 参数
/// - `config`: 监控配置
/// - `sysinfo_collector`: sysinfo 采集器
///
/// # 返回
/// 返回匹配的进程列表
fn get_filtered_processes(
    config: &MonitorConfig,
    sysinfo_collector: &mut SysinfoCollector,
) -> Vec<ProcessInfo> {
    match &config.process_filter {
        Some(ProcessFilter::Pids(pids)) => {
            // 按 PID 筛选
            pids.iter()
                .filter_map(|&pid| {
                    sysinfo_collector.get_process_by_pid(pid)
                        .or_else(|| Some(create_placeholder_process(pid)))
                })
                .collect()
        }
        Some(ProcessFilter::Name(name)) => {
            // 按进程名精确匹配
            sysinfo_collector.get_processes_by_name(name)
        }
        Some(ProcessFilter::NameRegex(pattern)) => {
            // 按正则表达式匹配进程名
            match regex::Regex::new(pattern) {
                Ok(re) => {
                    let all_procs = sysinfo_collector.get_all_processes();
                    all_procs
                        .into_iter()
                        .filter(|p| re.is_match(&p.name))
                        .collect()
                }
                Err(_) => {
                    // 正则表达式无效，返回空列表
                    Vec::new()
                }
            }
        }
        None => {
            // 无过滤器，返回空列表
            Vec::new()
        }
    }
}

/// 获取 Top N 进程
///
/// # 参数
/// - `sysinfo_collector`: sysinfo 采集器
/// - `n`: 获取的数量
/// - `extractor`: 用于排序的值提取函数
///
/// # 返回
/// 返回按指定字段排序后的前 N 个进程
fn get_top_n_processes<F>(
    sysinfo_collector: &mut SysinfoCollector,
    n: usize,
    extractor: F,
) -> Vec<ProcessInfo>
where
    F: Fn(&ProcessInfo) -> f64,
{
    let all_procs = sysinfo_collector.get_all_processes();

    // 使用 itertools 的 sorted_by 进行排序（降序）
    all_procs
        .into_iter()
        .sorted_by(|a, b| {
            let val_a = extractor(a);
            let val_b = extractor(b);
            val_b.partial_cmp(&val_a).unwrap_or(std::cmp::Ordering::Equal)
        })
        .take(n)
        .collect()
}

/// 创建占位进程信息
///
/// 当无法获取指定 PID 的进程信息时，创建一个占位对象
///
/// # 参数
/// - `pid`: 进程 ID
///
/// # 返回
/// 返回一个默认值的 ProcessInfo 实例
fn create_placeholder_process(pid: u32) -> ProcessInfo {
    ProcessInfo {
        pid,
        name: String::from("<unknown>"),
        cpu_percent: 0.0,
        working_set_mb: 0.0,
        committed_memory_mb: 0.0,
        gpu_percent: 0.0,
        gpu_memory_mb: 0.0,
        handle_count: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_placeholder_process() {
        let proc = create_placeholder_process(12345);
        assert_eq!(proc.pid, 12345);
        assert_eq!(proc.name, "<unknown>");
        assert_eq!(proc.cpu_percent, 0.0);
    }

    #[test]
    fn test_monitor_creation() {
        let config = MonitorConfig {
            interval: 1.0,
            duration: None,
            enable_hwinfo: false,
            enable_pdh: false,
            enable_sysinfo: true,
            hwinfo_path: None,
            process_filter: None,
            top_n_cpu: None,
            top_n_gpu: None,
        };

        let monitor = MonitorCore::new(config);
        assert!(monitor.is_ok());
        let monitor = monitor.unwrap();
        assert!(!monitor.is_running());
    }

    #[test]
    fn test_get_filtered_processes_by_pids() {
        let config = MonitorConfig {
            interval: 1.0,
            duration: None,
            enable_hwinfo: false,
            enable_pdh: false,
            enable_sysinfo: true,
            hwinfo_path: None,
            process_filter: Some(ProcessFilter::Pids(vec![999999])),
            top_n_cpu: None,
            top_n_gpu: None,
        };

        let mut collector = SysinfoCollector::new();
        let procs = get_filtered_processes(&config, &mut collector);

        // PID 999999 通常不存在，应该创建占位进程
        assert_eq!(procs.len(), 1);
        assert_eq!(procs[0].pid, 999999);
        assert_eq!(procs[0].name, "<unknown>");
    }
}