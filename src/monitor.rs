use std::thread::{self, JoinHandle};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use chrono::Utc;
use parking_lot::Mutex;
use itertools::Itertools;

use crate::data::{Sample, MonitorConfig, ProcessFilter, ProcessInfo};
use crate::ring_buffer::RingBuffer;
use crate::collector::{SysinfoCollector, PdhCollector, HWiNFOCollector};
use crate::hwinfo_manager::HWiNFOManager;

/// Monitor 核心结构
pub struct MonitorCore {
    config: MonitorConfig,
    buffer: RingBuffer,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    hwinfo_manager: Mutex<Option<HWiNFOManager>>,
}

impl MonitorCore {
    pub fn new(config: MonitorConfig) -> anyhow::Result<Self> {
        let hwinfo_manager = if config.enable_hwinfo {
            let mut manager = HWiNFOManager::new(config.hwinfo_path.as_deref())?;
            if let Err(e) = manager.start() {
                log::warn!("HWiNFO start failed: {}", e);
                None
            } else {
                Some(manager)
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

    pub fn start(&mut self) -> anyhow::Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.running.store(true, Ordering::SeqCst);

        let config = self.config.clone();
        let buffer = self.buffer.clone();
        let running = Arc::clone(&self.running);

        let thread = thread::spawn(move || {
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
    let system = if config.enable_sysinfo {
        let mut system_info = sysinfo_collector.get_system_info();

        match hwinfo_collector {
            Some(hwinfo) => {
                match hwinfo.get_system_info() {
                    Ok(hwinfo_data) => {
                        system_info.cpu.percent = hwinfo_data.cpu.percent;
                        system_info.gpu.percent = hwinfo_data.gpu.percent;
                        system_info.cpu.temperature = hwinfo_data.cpu.temperature;
                        system_info.cpu.power = hwinfo_data.cpu.power;
                        system_info.gpu.temperature = hwinfo_data.gpu.temperature;
                        system_info.gpu.power = hwinfo_data.gpu.power;
                        system_info.network = hwinfo_data.network;
                        system_info.battery = hwinfo_data.battery;
                        system_info.system_power = hwinfo_data.system_power;
                    }
                    Err(e) => {
                        log::warn!("HWiNFO get_system_info failed: {}", e);
                    }
                }
            }
            None => {
                log::warn!("HWiNFO not enabled, cannot get CPU/GPU usage");
            }
        }

        Some(system_info)
    } else {
        None
    };

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
        system,
        processes,
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