use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// HWiNFO 传感器数据值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorValue {
    pub value: f64,
    pub unit: String,
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
/// HWiNFO原始数据每次采集都返回，进程级数据按筛选条件返回
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub timestamp: DateTime<Utc>,
    /// HWiNFO 原始数据 - 所有传感器数据（按原始名称索引）
    pub hwinfo_raw: HashMap<String, SensorValue>,
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