use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// CPU 系统信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CPUInfo {
    pub percent: f64,
    pub temperature: Option<f64>,
    pub power: Option<f64>,
}

/// GPU 系统信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GPUInfo {
    pub percent: f64,
    pub temperature: Option<f64>,
    pub power: Option<f64>,
    pub memory_mb: Option<f64>,
}

/// 内存系统信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryInfo {
    pub percent: f64,
    pub used_mb: f64,
    pub total_mb: f64,
    pub committed_mb: f64,
    pub committed_limit_mb: f64,
}

/// 网络系统信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkInfo {
    pub upload_speed: f64,      // bytes/s
    pub download_speed: f64,    // bytes/s
}

/// 系统级信息汇总
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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