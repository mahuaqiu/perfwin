use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// CPU 系统信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CPUInfo {
    pub percent: f64,
    pub temperature: Option<f64>,
    pub power: Option<f64>,
    pub clock_speed: Option<f64>,  // GHz
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

/// 电池信息（笔记本电脑）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatteryInfo {
    pub charge_level: f64,      // 电量百分比 (0-100)
}

/// 系统级信息汇总
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemInfo {
    pub cpu: CPUInfo,
    pub gpu: GPUInfo,
    pub memory: MemoryInfo,
    pub network: NetworkInfo,
    pub battery: BatteryInfo,
    pub system_power: f64,      // 系统总功耗 (W)
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

/// 进程汇总信息（同名进程聚合）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedProcessInfo {
    pub name: String,
    pub pids: Vec<u32>,
    pub cpu_percent_total: f64,
    pub working_set_mb_total: f64,
    pub committed_memory_mb_total: f64,
    pub gpu_percent_total: f64,
    pub handle_count_total: u32,
    pub process_count: usize,      // 进程数量
}

/// 单次采样数据
/// 系统级数据每次采集都返回，进程级数据按筛选条件返回
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub timestamp: DateTime<Utc>,
    /// 系统级数据 - 每次采集强制返回
    pub system: SystemInfo,
    /// 进程明细数据 - 仅在有筛选条件时返回
    pub processes: Option<Vec<ProcessInfo>>,
    /// 进程汇总数据 - 仅在按进程名筛选时返回（多个同名PID聚合）
    pub aggregated: Option<Vec<AggregatedProcessInfo>>,
    /// Top N CPU 进程 - 仅在设置 top_n_cpu 参数时返回
    pub top_n_cpu: Option<Vec<ProcessInfo>>,
    /// Top N GPU 进程 - 仅在设置 top_n_gpu 参数时返回
    pub top_n_gpu: Option<Vec<ProcessInfo>>,
}

/// 进程筛选配置
#[derive(Debug, Clone)]
pub enum ProcessFilter {
    Pids(Vec<u32>),
    Name(String),
    Names(Vec<String>),     // 多个进程名
    NameRegex(String),
}

/// Monitor 配置参数
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub interval: f64,                  // 秒
    pub duration: Option<f64>,          // 秒，None 表示无限
    pub enable_pdh: bool,
    pub enable_sysinfo: bool,
    pub hwinfo_path: Option<String>,
    pub process_filter: Option<ProcessFilter>,
    pub top_n_cpu: Option<usize>,
    pub top_n_gpu: Option<usize>,
    pub enable_aggregation: bool,       // 是否生成汇总数据
}