use std::thread::{self, JoinHandle};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use chrono::Utc;
use parking_lot::Mutex;
use itertools::Itertools;

use crate::data::{Sample, MonitorConfig, ProcessFilter, ProcessInfo, AggregatedProcessInfo};
use crate::ring_buffer::RingBuffer;
use crate::collector::{SysinfoCollector, PdhCollector, HWiNFOCollector};
use crate::hwinfo_manager::HWiNFOManager;

/// Monitor 核心结构
pub struct MonitorCore {
    config: MonitorConfig,
    buffer: RingBuffer,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    hwinfo_manager: Arc<Mutex<Option<HWiNFOManager>>>,
    /// 标记是否已经尝试过重启 HWiNFO（避免无限循环）
    hwinfo_restarted: Arc<AtomicBool>,
}

impl MonitorCore {
    pub fn new(config: MonitorConfig) -> anyhow::Result<Self> {
        // HWiNFO 强制启用
        let hwinfo_manager = {
            let mut manager = HWiNFOManager::new(config.hwinfo_path.as_deref())?;
            if let Err(e) = manager.start() {
                log::warn!("HWiNFO start failed: {}", e);
                None
            } else {
                Some(manager)
            }
        };

        Ok(Self {
            config,
            buffer: RingBuffer::new(),
            running: Arc::new(AtomicBool::new(false)),
            thread: None,
            hwinfo_manager: Arc::new(Mutex::new(hwinfo_manager)),
            hwinfo_restarted: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn start(&mut self) -> anyhow::Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.running.store(true, Ordering::SeqCst);

        let config = self.config.clone();
        let buffer = self.buffer.clone();
        let running = Arc::clone(&self.running);
        let hwinfo_manager = Arc::clone(&self.hwinfo_manager);
        let hwinfo_restarted = Arc::clone(&self.hwinfo_restarted);

        let thread = thread::spawn(move || {
            let mut sysinfo_collector = SysinfoCollector::new();
            let mut pdh_collector = if config.enable_pdh {
                PdhCollector::new().ok()
            } else {
                None
            };
            // HWiNFO 强制启用，首次创建可能失败（配置过期）
            let mut hwinfo_collector = HWiNFOCollector::new().ok();

            let start_time = Instant::now();
            let interval = Duration::from_secs_f64(config.interval);

            // PDH 预初始化
            if let Some(pdh) = &mut pdh_collector {
                let _ = pdh.collect();  // 第一次 collect 初始化
            }

            thread::sleep(interval);

            while running.load(Ordering::SeqCst) {
                if let Some(duration) = config.duration {
                    if start_time.elapsed().as_secs_f64() >= duration {
                        running.store(false, Ordering::SeqCst);
                        break;
                    }
                }

                // 如果 HWiNFO 采集器无效，尝试检测并重启
                if hwinfo_collector.is_none() && !hwinfo_restarted.load(Ordering::SeqCst) {
                    // 检查是否是配置过期导致
                    let manager_guard = hwinfo_manager.lock();
                    if let Some(manager) = manager_guard.as_ref() {
                        if !manager.check_shared_memory_enabled() {
                            log::warn!("HWiNFO shared memory expired (SensorsSM=1 missing), attempting restart with config fix");
                            // 释放锁后重启
                            drop(manager_guard);

                            let mut manager_guard = hwinfo_manager.lock();
                            if let Some(manager) = manager_guard.as_mut() {
                                if let Err(e) = manager.restart_with_fix() {
                                    log::error!("HWiNFO restart with fix failed: {}", e);
                                } else {
                                    log::info!("HWiNFO restarted successfully with SensorsSM=1");
                                    // 尝试重新创建采集器
                                    hwinfo_collector = HWiNFOCollector::new().ok();
                                    hwinfo_restarted.store(true, Ordering::SeqCst);
                                }
                            }
                        }
                    }
                }

                let sample = collect_sample(
                    &config,
                    &mut sysinfo_collector,
                    &mut pdh_collector,
                    &hwinfo_collector,
                );

                buffer.push(sample);
                thread::sleep(interval);
            }
        });

        self.thread = Some(thread);
        Ok(())
    }

    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.running.store(false, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
        if let Some(mut manager) = self.hwinfo_manager.lock().take() {
            manager.stop()?;
        }
        Ok(())
    }

    pub fn get_result(&self) -> Vec<Sample> {
        self.buffer.drain()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }
}

impl Drop for MonitorCore {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn collect_sample(
    config: &MonitorConfig,
    sysinfo_collector: &mut SysinfoCollector,
    pdh_collector: &mut Option<PdhCollector>,
    hwinfo_collector: &Option<HWiNFOCollector>,
) -> Sample {
    // 获取 HWiNFO 原始数据
    let hwinfo_raw = hwinfo_collector.as_ref()
        .map(|h| h.get_all_entries())
        .unwrap_or_default();

    // 进程数据（有筛选条件时返回）
    let processes = if config.process_filter.is_some() {
        let mut procs = get_filtered_processes(config, sysinfo_collector);

        if let Some(pdh) = pdh_collector {
            let _ = pdh.collect();
            let _ = pdh.update_process_gpu(&mut procs);
        }

        Some(procs)
    } else {
        None
    };

    // 汇总数据（仅进程名筛选且启用汇总时返回）
    let aggregated = if config.enable_aggregation && processes.is_some() {
        let procs = processes.as_ref().unwrap();
        // 判断是否是进程名筛选（Name 或 Names）
        let is_name_filter = matches!(
            &config.process_filter,
            Some(ProcessFilter::Name(_)) | Some(ProcessFilter::Names(_))
        );
        if is_name_filter {
            Some(aggregate_processes(procs))
        } else {
            None
        }
    } else {
        None
    };

    let top_n_cpu = config.top_n_cpu.and_then(|n| {
        let mut procs = get_top_n_processes(sysinfo_collector, n, |p| p.cpu_percent);
        if let Some(pdh) = pdh_collector {
            let _ = pdh.collect();
            let _ = pdh.update_process_gpu(&mut procs);
        }
        Some(procs)
    });

    let top_n_gpu = config.top_n_gpu.and_then(|n| {
        let mut procs = get_top_n_processes(sysinfo_collector, n, |p| p.gpu_percent);
        if let Some(pdh) = pdh_collector {
            let _ = pdh.collect();
            let _ = pdh.update_process_gpu(&mut procs);
        }
        Some(procs)
    });

    Sample {
        timestamp: Utc::now(),
        hwinfo_raw,
        processes,
        aggregated,
        top_n_cpu,
        top_n_gpu,
    }
}

fn get_filtered_processes(
    config: &MonitorConfig,
    sysinfo_collector: &mut SysinfoCollector,
) -> Vec<ProcessInfo> {
    match &config.process_filter {
        Some(ProcessFilter::Pids(pids)) => {
            pids.iter()
                .filter_map(|&pid| {
                    sysinfo_collector.get_process_by_pid(pid)
                        .or_else(|| Some(create_placeholder_process(pid)))
                })
                .collect()
        }
        Some(ProcessFilter::Name(name)) => {
            sysinfo_collector.get_processes_by_name(name)
        }
        Some(ProcessFilter::Names(names)) => {
            // 多个进程名：合并所有匹配的进程
            names.iter()
                .flat_map(|name| sysinfo_collector.get_processes_by_name(name))
                .collect()
        }
        Some(ProcessFilter::NameRegex(pattern)) => {
            match regex::Regex::new(pattern) {
                Ok(re) => {
                    let all_procs = sysinfo_collector.get_all_processes();
                    all_procs.into_iter().filter(|p| re.is_match(&p.name)).collect()
                }
                Err(_) => Vec::new()
            }
        }
        None => Vec::new()
    }
}

/// 汇总同名进程数据
fn aggregate_processes(processes: &[ProcessInfo]) -> Vec<AggregatedProcessInfo> {
    use std::collections::HashMap;

    // 按进程名分组
    let mut groups: HashMap<String, Vec<&ProcessInfo>> = HashMap::new();
    for proc in processes {
        groups.entry(proc.name.clone()).or_default().push(proc);
    }

    // 计算汇总数据
    groups.into_iter()
        .map(|(name, procs)| {
            let pids: Vec<u32> = procs.iter().map(|p| p.pid).collect();
            let cpu_percent_total = procs.iter().map(|p| p.cpu_percent).sum();
            let working_set_mb_total = procs.iter().map(|p| p.working_set_mb).sum();
            let committed_memory_mb_total = procs.iter().map(|p| p.committed_memory_mb).sum();
            let gpu_percent_total = procs.iter().map(|p| p.gpu_percent).sum();
            let handle_count_total = procs.iter().map(|p| p.handle_count).sum();

            AggregatedProcessInfo {
                name,
                pids,
                cpu_percent_total,
                working_set_mb_total,
                committed_memory_mb_total,
                gpu_percent_total,
                handle_count_total,
                process_count: procs.len(),
            }
        })
        .collect()
}

fn get_top_n_processes<F>(
    sysinfo_collector: &mut SysinfoCollector,
    n: usize,
    extractor: F,
) -> Vec<ProcessInfo>
where
    F: Fn(&ProcessInfo) -> f64,
{
    let all_procs = sysinfo_collector.get_all_processes();
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