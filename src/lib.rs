use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pyo3::wrap_pyfunction;
use std::sync::Arc;
use parking_lot::Mutex;

pub mod data;
pub mod ring_buffer;
pub mod collector;
pub mod hwinfo_manager;
pub mod monitor;

use crate::data::{
    ProcessInfo, AggregatedProcessInfo, Sample, SystemMetrics, MonitorConfig, ProcessFilter,
};
use crate::monitor::MonitorCore;
use crate::collector::hwinfo::{HWiNFOCollector, SensorEntry};
use crate::collector::sysinfo::SysinfoCollector;

// ============================================================================
// PyO3 绑定类
// ============================================================================

// ----------------------------------------------------------------------------
// PyProcessInfo - 进程信息类
// ----------------------------------------------------------------------------

/// 进程信息类
///
/// 包含单个进程的性能数据
#[pyclass(name = "ProcessInfo")]
#[derive(Debug, Clone)]
pub struct PyProcessInfo {
    inner: ProcessInfo,
}

#[pymethods]
impl PyProcessInfo {
    #[new]
    fn new(pid: u32, name: String) -> Self {
        Self {
            inner: ProcessInfo {
                pid,
                name,
                cpu_percent: 0.0,
                working_set_mb: 0.0,
                committed_memory_mb: 0.0,
                gpu_percent: 0.0,
                gpu_memory_mb: 0.0,
                handle_count: 0,
            },
        }
    }

    /// 进程 ID
    #[getter]
    fn pid(&self) -> u32 {
        self.inner.pid
    }

    /// 进程名称
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// CPU 使用率百分比
    #[getter]
    fn cpu_percent(&self) -> f64 {
        self.inner.cpu_percent
    }

    /// 工作集内存 (MB)
    #[getter]
    fn working_set_mb(&self) -> f64 {
        self.inner.working_set_mb
    }

    /// 提交内存 (MB)
    #[getter]
    fn committed_memory_mb(&self) -> f64 {
        self.inner.committed_memory_mb
    }

    /// GPU 使用率百分比
    #[getter]
    fn gpu_percent(&self) -> f64 {
        self.inner.gpu_percent
    }

    /// GPU 显存使用量 (MB)
    #[getter]
    fn gpu_memory_mb(&self) -> f64 {
        self.inner.gpu_memory_mb
    }

    /// 句柄数
    #[getter]
    fn handle_count(&self) -> u32 {
        self.inner.handle_count
    }

    fn __repr__(&self) -> String {
        format!(
            "ProcessInfo(pid={}, name='{}', cpu={:.1}%, mem={:.1}MB)",
            self.inner.pid, self.inner.name, self.inner.cpu_percent, self.inner.working_set_mb
        )
    }
}

impl From<ProcessInfo> for PyProcessInfo {
    fn from(info: ProcessInfo) -> Self {
        Self { inner: info }
    }
}

// ----------------------------------------------------------------------------
// PyAggregatedProcessInfo - 进程汇总信息类
// ----------------------------------------------------------------------------

/// 进程汇总信息类
///
/// 包含同名进程聚合后的性能数据
#[pyclass(name = "AggregatedProcessInfo")]
#[derive(Debug, Clone)]
pub struct PyAggregatedProcessInfo {
    inner: AggregatedProcessInfo,
}

#[pymethods]
impl PyAggregatedProcessInfo {
    /// 进程名称
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// 所有 PID 列表
    #[getter]
    fn pids(&self) -> Vec<u32> {
        self.inner.pids.clone()
    }

    /// CPU 使用率总和百分比
    #[getter]
    fn cpu_percent_total(&self) -> f64 {
        self.inner.cpu_percent_total
    }

    /// 工作集内存总和 (MB)
    #[getter]
    fn working_set_mb_total(&self) -> f64 {
        self.inner.working_set_mb_total
    }

    /// 提交内存总和 (MB)
    #[getter]
    fn committed_memory_mb_total(&self) -> f64 {
        self.inner.committed_memory_mb_total
    }

    /// GPU 使用率总和百分比
    #[getter]
    fn gpu_percent_total(&self) -> f64 {
        self.inner.gpu_percent_total
    }

    /// 句柄数总和
    #[getter]
    fn handle_count_total(&self) -> u32 {
        self.inner.handle_count_total
    }

    /// 进程数量
    #[getter]
    fn process_count(&self) -> usize {
        self.inner.process_count
    }

    fn __repr__(&self) -> String {
        format!(
            "AggregatedProcessInfo(name='{}', pids={}, cpu={:.1}%, mem={:.1}MB)",
            self.inner.name, self.inner.pids.len(), self.inner.cpu_percent_total, self.inner.working_set_mb_total
        )
    }
}

impl From<AggregatedProcessInfo> for PyAggregatedProcessInfo {
    fn from(info: AggregatedProcessInfo) -> Self {
        Self { inner: info }
    }
}

// ----------------------------------------------------------------------------
// PySample - 单次采样类
// ----------------------------------------------------------------------------

/// 单次采样数据类
///
/// 包含某一时刻的系统快照数据
#[pyclass(name = "Sample")]
#[derive(Debug, Clone)]
pub struct PySample {
    inner: Sample,
}

#[pymethods]
impl PySample {
    #[new]
    fn new() -> Self {
        Self {
            inner: Sample {
                sequence: 0,
                elapsed_ms: 0,
                timestamp: chrono::Utc::now(),
                system: SystemMetrics::default(),
                hwinfo_raw: std::collections::HashMap::new(),
                processes: None,
                aggregated: None,
                top_n_cpu: None,
                top_n_gpu: None,
            },
        }
    }

    /// 采样序号
    #[getter]
    fn sequence(&self) -> u64 {
        self.inner.sequence
    }

    /// 相对监控开始时间，单位毫秒
    #[getter]
    fn elapsed_ms(&self) -> u64 {
        self.inner.elapsed_ms
    }

    /// 系统指标
    #[getter]
    fn system(&self, py: Python<'_>) -> PyResult<PyObject> {
        system_metrics_to_pyobject(py, &self.inner.system)
    }
    /// 采样时间戳 (ISO 8601 格式字符串)
    #[getter]
    fn timestamp(&self) -> String {
        self.inner.timestamp.to_rfc3339()
    }

    /// HWiNFO 所有传感器原始数据（字典格式）
    #[getter]
    fn hwinfo_raw(&self, py: Python<'_>) -> PyResult<PyObject> {
        let dict = PyDict::new_bound(py);
        for (name, value) in &self.inner.hwinfo_raw {
            let value_dict = PyDict::new_bound(py);
            value_dict.set_item("value", value.value)?;
            value_dict.set_item("unit", &value.unit)?;
            dict.set_item(name, value_dict)?;
        }
        Ok(dict.into())
    }

    /// 目标进程明细列表，可能为 None
    #[getter]
    fn processes(&self) -> Option<Vec<PyProcessInfo>> {
        self.inner.processes.as_ref().map(|procs| {
            procs.iter().map(|p| PyProcessInfo::from(p.clone())).collect()
        })
    }

    /// 进程汇总列表，可能为 None（仅进程名筛选时返回）
    #[getter]
    fn aggregated(&self) -> Option<Vec<PyAggregatedProcessInfo>> {
        self.inner.aggregated.as_ref().map(|agg| {
            agg.iter().map(|a| PyAggregatedProcessInfo::from(a.clone())).collect()
        })
    }

    /// Top N CPU 进程列表（同名进程合并后排序），可能为 None
    #[getter]
    fn top_n_cpu(&self) -> Option<Vec<PyAggregatedProcessInfo>> {
        self.inner.top_n_cpu.as_ref().map(|agg| {
            agg.iter().map(|a| PyAggregatedProcessInfo::from(a.clone())).collect()
        })
    }

    /// Top N GPU 进程列表（同名进程合并后排序），可能为 None
    #[getter]
    fn top_n_gpu(&self) -> Option<Vec<PyAggregatedProcessInfo>> {
        self.inner.top_n_gpu.as_ref().map(|agg| {
            agg.iter().map(|a| PyAggregatedProcessInfo::from(a.clone())).collect()
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "Sample(timestamp='{}', processes={:?}, aggregated={:?})",
            self.inner.timestamp.format("%Y-%m-%d %H:%M:%S"),
            self.inner.processes.as_ref().map(|p| p.len()),
            self.inner.aggregated.as_ref().map(|a| a.len())
        )
    }
}

impl From<Sample> for PySample {
    fn from(sample: Sample) -> Self {
        Self { inner: sample }
    }
}

// ----------------------------------------------------------------------------
// PyMonitorResult - 返回结果类
// ----------------------------------------------------------------------------

/// 监控结果类
///
/// 包含监控期间采集的所有数据
#[pyclass(name = "MonitorResult")]
#[derive(Debug, Clone)]
pub struct PyMonitorResult {
    samples: Vec<PySample>,
}

#[pymethods]
impl PyMonitorResult {
    #[new]
    fn new() -> Self {
        Self { samples: Vec::new() }
    }

    /// 获取采样数量
    fn __len__(&self) -> usize {
        self.samples.len()
    }

    /// 获取指定索引的采样
    fn __getitem__(&self, index: usize) -> PyResult<PySample> {
        self.samples
            .get(index)
            .cloned()
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyIndexError, _>("index out of range"))
    }

    /// 迭代支持
    fn __iter__(slf: Py<Self>) -> PyIterWrapper {
        PyIterWrapper {
            samples: slf,
            index: 0,
        }
    }

    /// 获取所有采样
    #[getter]
    fn samples(&self) -> Vec<PySample> {
        self.samples.clone()
    }

    /// 转换为 Python 字典列表
    fn to_dicts<'a>(&self, py: Python<'a>) -> PyResult<Vec<Bound<'a, PyDict>>> {
        let mut result = Vec::new();
        for sample in &self.samples {
            let dict = sample.to_dict(py)?;
            result.push(dict);
        }
        Ok(result)
    }

    fn __repr__(&self) -> String {
        format!("MonitorResult(samples={})", self.samples.len())
    }
}

/// 迭代器包装器，用于支持 `for sample in result:` 语法
#[pyclass(name = "MonitorResultIterator")]
struct PyIterWrapper {
    samples: Py<PyMonitorResult>,
    index: usize,
}

#[pymethods]
impl PyIterWrapper {
    fn __next__(&mut self, py: Python<'_>) -> Option<PySample> {
        let inner = self.samples.borrow(py);
        if self.index < inner.samples.len() {
            let sample = inner.samples[self.index].clone();
            self.index += 1;
            Some(sample)
        } else {
            None
        }
    }
}

impl PySample {
    /// 将采样数据转换为 Python 字典
    fn to_dict<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyDict>> {
        let dict = PyDict::new_bound(py);

        dict.set_item("sequence", self.inner.sequence)?;
        dict.set_item("elapsed_ms", self.inner.elapsed_ms)?;
        dict.set_item("timestamp", self.inner.timestamp.to_rfc3339())?;
        dict.set_item("system", system_metrics_to_pyobject(py, &self.inner.system)?)?;

        // hwinfo_raw 必须返回
        let hwinfo_dict = PyDict::new_bound(py);
        for (name, value) in &self.inner.hwinfo_raw {
            let value_dict = PyDict::new_bound(py);
            value_dict.set_item("value", value.value)?;
            value_dict.set_item("unit", &value.unit)?;
            hwinfo_dict.set_item(name, value_dict)?;
        }
        dict.set_item("hwinfo_raw", hwinfo_dict)?;

        // processes 可选
        if let Some(processes) = &self.inner.processes {
            let procs_list = PyList::new_bound(py, Vec::<Bound<'_, PyDict>>::new());
            for proc in processes {
                let proc_dict = PyDict::new_bound(py);
                proc_dict.set_item("pid", proc.pid)?;
                proc_dict.set_item("name", &proc.name)?;
                proc_dict.set_item("cpu_percent", proc.cpu_percent)?;
                proc_dict.set_item("working_set_mb", proc.working_set_mb)?;
                proc_dict.set_item("committed_memory_mb", proc.committed_memory_mb)?;
                proc_dict.set_item("gpu_percent", proc.gpu_percent)?;
                proc_dict.set_item("gpu_memory_mb", proc.gpu_memory_mb)?;
                proc_dict.set_item("handle_count", proc.handle_count)?;
                procs_list.append(proc_dict)?;
            }
            dict.set_item("processes", procs_list)?;
        }

        // aggregated 可选
        if let Some(aggregated) = &self.inner.aggregated {
            let agg_list = PyList::new_bound(py, Vec::<Bound<'_, PyDict>>::new());
            for agg in aggregated {
                let agg_dict = PyDict::new_bound(py);
                agg_dict.set_item("name", &agg.name)?;
                agg_dict.set_item("pids", agg.pids.clone())?;
                agg_dict.set_item("cpu_percent_total", agg.cpu_percent_total)?;
                agg_dict.set_item("working_set_mb_total", agg.working_set_mb_total)?;
                agg_dict.set_item("committed_memory_mb_total", agg.committed_memory_mb_total)?;
                agg_dict.set_item("gpu_percent_total", agg.gpu_percent_total)?;
                agg_dict.set_item("handle_count_total", agg.handle_count_total)?;
                agg_dict.set_item("process_count", agg.process_count)?;
                agg_list.append(agg_dict)?;
            }
            dict.set_item("aggregated", agg_list)?;
        }

        if let Some(top_n) = &self.inner.top_n_cpu {
            let list = aggregated_list_to_pylist(py, top_n)?;
            dict.set_item("top_n_cpu", list)?;
        }

        if let Some(top_n) = &self.inner.top_n_gpu {
            let list = aggregated_list_to_pylist(py, top_n)?;
            dict.set_item("top_n_gpu", list)?;
        }

        Ok(dict)
    }
}

fn system_metrics_to_pyobject<'py>(py: Python<'py>, system: &SystemMetrics) -> PyResult<PyObject> {
    let dict = PyDict::new_bound(py);
    dict.set_item("cpu_percent", system.cpu_percent)?;
    dict.set_item("gpu_percent", system.gpu_percent)?;
    dict.set_item("gpu_source", &system.gpu_source)?;
    let adapters = PyList::new_bound(py, Vec::<Bound<'py, PyDict>>::new());
    for adapter in &system.gpu_adapters {
        let item = PyDict::new_bound(py);
        item.set_item("luid", &adapter.luid)?;
        item.set_item("name", &adapter.name)?;
        item.set_item("utilization_percent", adapter.utilization_percent)?;
        adapters.append(item)?;
    }
    dict.set_item("gpu_adapters", adapters)?;
    Ok(dict.into())
}
/// 将进程列表转换为 Python 列表
fn process_list_to_pylist<'py>(py: Python<'py>, processes: &[ProcessInfo]) -> PyResult<Bound<'py, PyList>> {
    let list = PyList::new_bound(py, Vec::<Bound<'py, PyDict>>::new());
    for proc in processes {
        let dict = PyDict::new_bound(py);
        dict.set_item("pid", proc.pid)?;
        dict.set_item("name", &proc.name)?;
        dict.set_item("cpu_percent", proc.cpu_percent)?;
        dict.set_item("working_set_mb", proc.working_set_mb)?;
        dict.set_item("committed_memory_mb", proc.committed_memory_mb)?;
        dict.set_item("gpu_percent", proc.gpu_percent)?;
        dict.set_item("gpu_memory_mb", proc.gpu_memory_mb)?;
        dict.set_item("handle_count", proc.handle_count)?;
        list.append(dict)?;
    }
    Ok(list)
}

/// 将汇总进程列表转换为 Python 列表
fn aggregated_list_to_pylist<'py>(py: Python<'py>, aggregated: &[AggregatedProcessInfo]) -> PyResult<Bound<'py, PyList>> {
    let list = PyList::new_bound(py, Vec::<Bound<'py, PyDict>>::new());
    for agg in aggregated {
        let dict = PyDict::new_bound(py);
        dict.set_item("name", &agg.name)?;
        dict.set_item("pids", &agg.pids)?;
        dict.set_item("cpu_percent_total", agg.cpu_percent_total)?;
        dict.set_item("working_set_mb_total", agg.working_set_mb_total)?;
        dict.set_item("committed_memory_mb_total", agg.committed_memory_mb_total)?;
        dict.set_item("gpu_percent_total", agg.gpu_percent_total)?;
        dict.set_item("handle_count_total", agg.handle_count_total)?;
        dict.set_item("process_count", agg.process_count)?;
        list.append(dict)?;
    }
    Ok(list)
}

// ----------------------------------------------------------------------------
// PyProcessFilter - 进程筛选类
// ----------------------------------------------------------------------------

/// 进程筛选配置类
///
/// 用于指定需要监控的目标进程
///
/// 示例:
///     # 按 PID 筛选
///     filter = ProcessFilter(pids=[1234, 5678])
///
///     # 按进程名精确匹配（单个）
///     filter = ProcessFilter(name="chrome.exe")
///
///     # 按进程名精确匹配（多个）
///     filter = ProcessFilter(names=["chrome.exe", "firefox.exe"])
///
///     # 按进程名正则匹配
///     filter = ProcessFilter(name_regex=r"chrome.*")
#[pyclass(name = "ProcessFilter")]
#[derive(Debug, Clone)]
pub struct PyProcessFilter {
    inner: ProcessFilter,
}

#[pymethods]
impl PyProcessFilter {
    #[new]
    #[pyo3(signature = (pids=None, name=None, names=None, name_regex=None))]
    fn new(
        pids: Option<Vec<u32>>,
        name: Option<String>,
        names: Option<Vec<String>>,
        name_regex: Option<String>,
    ) -> PyResult<Self> {
        let inner = if let Some(pids) = pids {
            ProcessFilter::Pids(pids)
        } else if let Some(names) = names {
            ProcessFilter::Names(names)
        } else if let Some(name) = name {
            ProcessFilter::Name(name)
        } else if let Some(pattern) = name_regex {
            ProcessFilter::NameRegex(pattern)
        } else {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "Must specify one of: pids, name, names, or name_regex"
            ));
        };
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            ProcessFilter::Pids(pids) => format!("ProcessFilter(pids={:?})", pids),
            ProcessFilter::Name(name) => format!("ProcessFilter(name='{}')", name),
            ProcessFilter::Names(names) => format!("ProcessFilter(names={:?})", names),
            ProcessFilter::NameRegex(pattern) => format!("ProcessFilter(name_regex='{}')", pattern),
        }
    }
}

// ----------------------------------------------------------------------------
// PyMonitor - 主监控类
// ----------------------------------------------------------------------------

/// 性能监控主类
///
/// 用于采集系统性能数据
///
/// 示例:
///     with Monitor() as m:
///         time.sleep(10)
///     result = m.get_result()
///
///     # 或手动控制
///     m = Monitor()
///     m.start()
///     time.sleep(10)
///     m.stop()
///     result = m.get_result()
#[pyclass(name = "Monitor")]
pub struct PyMonitor {
    /// 监控配置
    config: MonitorConfig,
    /// Monitor 核心实例（使用 Arc<Mutex> 支持线程安全）
    core: Option<Arc<Mutex<MonitorCore>>>,
}

#[pymethods]
impl PyMonitor {
    /// 创建新的 Monitor 实例
    ///
    /// # 参数
    /// - `interval`: 采样间隔 (秒)，默认 1.0，最小值 1.0
    /// - `duration`: 监控时长 (秒)，None 表示无限，默认 None
    /// - `enable_pdh`: 是否启用 PDH (GPU采集)，默认 True
    /// - `enable_sysinfo`: 是否启用系统信息采集，默认 True
    /// - `hwinfo_path`: HWiNFO 路径，默认自动检测模块目录下的 HWiNFO64 子目录
    /// - `process_filter`: 进程筛选器，默认 None
    /// - `top_n_cpu`: 获取 Top N CPU 进程，默认 None
    /// - `top_n_gpu`: 获取 Top N GPU 进程，默认 None
    /// - `enable_aggregation`: 是否生成汇总数据（仅进程名筛选时有效），默认 True
    #[new]
    #[pyo3(signature = (
        interval=1.0,
        duration=None,
        enable_pdh=true,
        enable_sysinfo=true,
        hwinfo_path=None,
        process_filter=None,
        top_n_cpu=None,
        top_n_gpu=None,
        enable_aggregation=true,
        _module_path=None
    ))]
    fn new(
        interval: f64,
        duration: Option<f64>,
        enable_pdh: bool,
        enable_sysinfo: bool,
        hwinfo_path: Option<String>,
        process_filter: Option<PyProcessFilter>,
        top_n_cpu: Option<usize>,
        top_n_gpu: Option<usize>,
        enable_aggregation: bool,
        _module_path: Option<String>,  // 内部参数，用于获取模块路径
    ) -> PyResult<Self> {
        // 校验 interval 不能小于 1 秒
        if interval < 1.0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "采集间隔不能小于 1 秒"
            ));
        }

        let filter = process_filter.map(|f| f.inner);

        // 如果未指定 hwinfo_path，使用模块目录下的 HWiNFO64 子目录
        let hwinfo_path = hwinfo_path.or_else(|| {
            _module_path.map(|p| {
                // 从模块路径获取目录，然后拼接 HWiNFO64/HWiNFO64.EXE
                let module_dir = std::path::Path::new(&p)
                    .parent()
                    .map(|d| d.to_string_lossy().into_owned())
                    .unwrap_or_default();
                format!("{}\\HWiNFO64\\HWiNFO64.EXE", module_dir)
            })
        });

        let config = MonitorConfig {
            interval,
            duration,
            enable_pdh,
            enable_sysinfo,
            hwinfo_path,
            process_filter: filter,
            top_n_cpu,
            top_n_gpu,
            enable_aggregation,
        };

        // 创建 MonitorCore 实例（HWiNFO 强制启用）
        let core = MonitorCore::new(config.clone())
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        Ok(Self {
            config,
            core: Some(Arc::new(Mutex::new(core))),
        })
    }

    /// 启动监控
    ///
    /// 开始后台采集线程
    fn start(slf: Py<Self>, py: Python<'_>) -> PyResult<()> {
        let inner = slf.borrow(py);
        if let Some(core) = &inner.core {
            let mut core_guard = core.lock();
            core_guard
                .start()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        }
        Ok(())
    }

    /// 停止监控
    ///
    /// 停止后台采集线程
    fn stop(slf: Py<Self>, py: Python<'_>) -> PyResult<()> {
        let inner = slf.borrow(py);
        if let Some(core) = &inner.core {
            let mut core_guard = core.lock();
            core_guard
                .stop()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        }
        Ok(())
    }

    /// 获取监控结果
    ///
    /// 返回采集的所有数据并清空缓冲区
    fn get_result(slf: Py<Self>, py: Python<'_>) -> PyResult<PyMonitorResult> {
        let inner = slf.borrow(py);
        if let Some(core) = &inner.core {
            let core_guard = core.lock();
            let samples = core_guard.get_result();
            let py_samples: Vec<PySample> = samples.into_iter().map(PySample::from).collect();
            Ok(PyMonitorResult { samples: py_samples })
        } else {
            Ok(PyMonitorResult { samples: Vec::new() })
        }
    }

    /// 检查是否正在运行
    fn is_running(slf: Py<Self>, py: Python<'_>) -> bool {
        let inner = slf.borrow(py);
        if let Some(core) = &inner.core {
            let core_guard = core.lock();
            core_guard.is_running()
        } else {
            false
        }
    }

    /// 获取缓冲区中的数据数量
    fn buffer_len(slf: Py<Self>, py: Python<'_>) -> usize {
        let inner = slf.borrow(py);
        if let Some(core) = &inner.core {
            let core_guard = core.lock();
            core_guard.buffer_len()
        } else {
            0
        }
    }

    /// 获取采样间隔
    #[getter]
    fn interval(&self) -> f64 {
        self.config.interval
    }

    /// 获取监控时长
    #[getter]
    fn duration(&self) -> Option<f64> {
        self.config.duration
    }

    /// 是否启用 PDH
    #[getter]
    fn enable_pdh(&self) -> bool {
        self.config.enable_pdh
    }

    /// 是否启用系统信息采集
    #[getter]
    fn enable_sysinfo(&self) -> bool {
        self.config.enable_sysinfo
    }

    /// 是否启用汇总数据
    #[getter]
    fn enable_aggregation(&self) -> bool {
        self.config.enable_aggregation
    }

    /// 进入上下文管理器
    ///
    /// 返回 `Py<Self>` 以支持 `with Monitor(...) as m:` 语法
    fn __enter__(slf: Py<Self>, py: Python<'_>) -> PyResult<Py<Self>> {
        // 启动监控
        {
            let inner = slf.borrow(py);
            if let Some(core) = &inner.core {
                let mut core_guard = core.lock();
                core_guard
                    .start()
                    .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
            }
        }
        // 返回 Py<Self> 以支持 with 语法
        Ok(slf)
    }

    /// 退出上下文管理器
    fn __exit__(
        slf: Py<Self>,
        py: Python<'_>,
        _exc_type: &Bound<'_, PyAny>,
        _exc_value: &Bound<'_, PyAny>,
        _traceback: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        // 停止监控
        {
            let inner = slf.borrow(py);
            if let Some(core) = &inner.core {
                let mut core_guard = core.lock();
                let _ = core_guard.stop();
            }
        }
        // 不抑制异常
        Ok(false)
    }

    fn __repr__(&self) -> String {
        format!(
            "Monitor(interval={}, duration={:?})",
            self.config.interval, self.config.duration
        )
    }
}

// ----------------------------------------------------------------------------
// PySensorEntry - HWiNFO 传感器条目类
// ----------------------------------------------------------------------------

/// HWiNFO 传感器条目类
///
/// 包含单个传感器的数据
#[pyclass(name = "SensorEntry")]
#[derive(Debug, Clone)]
pub struct PySensorEntry {
    inner: SensorEntry,
}

#[pymethods]
impl PySensorEntry {
    /// 传感器类型
    #[getter]
    fn sensor_type(&self) -> String {
        format!("{:?}", self.inner.sensor_type)
    }

    /// 传感器索引
    #[getter]
    fn sensor_index(&self) -> u32 {
        self.inner.sensor_index
    }

    /// ID
    #[getter]
    fn id(&self) -> u32 {
        self.inner.id
    }

    /// 原始名称
    #[getter]
    fn name_original(&self) -> &str {
        &self.inner.name_original
    }

    /// 用户自定义名称
    #[getter]
    fn name_user(&self) -> &str {
        &self.inner.name_user
    }

    /// 显示名称（优先用户自定义名称）
    fn label(&self) -> &str {
        self.inner.label()
    }

    /// 单位
    #[getter]
    fn unit(&self) -> &str {
        &self.inner.unit
    }

    /// 当前值
    #[getter]
    fn value(&self) -> f64 {
        self.inner.value
    }

    /// 最小值
    #[getter]
    fn value_min(&self) -> f64 {
        self.inner.value_min
    }

    /// 最大值
    #[getter]
    fn value_max(&self) -> f64 {
        self.inner.value_max
    }

    /// 平均值
    #[getter]
    fn value_avg(&self) -> f64 {
        self.inner.value_avg
    }

    fn __repr__(&self) -> String {
        format!(
            "SensorEntry(type={:?}, name='{}', value={:.2} {})",
            self.inner.sensor_type, self.inner.label(), self.inner.value, self.inner.unit
        )
    }
}

impl From<SensorEntry> for PySensorEntry {
    fn from(entry: SensorEntry) -> Self {
        Self { inner: entry }
    }
}

// ============================================================================
// 模块级函数
// ============================================================================

/// 列出系统中所有进程（PID + 进程名）
///
/// 返回: List[Tuple[int, str]] - [(pid, name), ...]
///
/// 示例:
///     processes = perfwin.list_processes()
///     for pid, name in processes:
///         print(f"{name}: {pid}")
#[pyfunction]
fn list_processes() -> PyResult<Vec<(u32, String)>> {
    #[cfg(target_os = "windows")]
    {
        let mut collector = SysinfoCollector::new();
        let processes = collector.get_all_processes();
        Ok(processes.into_iter().map(|p| (p.pid, p.name)).collect())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
            "list_processes 仅在 Windows 平台上可用"
        ))
    }
}

/// 列出 HWiNFO 共享内存中的所有传感器
#[pyfunction]
fn list_hwinfo_sensors() -> PyResult<Vec<PySensorEntry>> {
    #[cfg(target_os = "windows")]
    {
        HWiNFOCollector::new()
            .map(|collector| {
                collector.iter_entries()
                    .map(PySensorEntry::from)
                    .collect()
            })
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
            "HWiNFO 仅在 Windows 平台上可用"
        ))
    }
}

// ============================================================================
// 模块定义
// ============================================================================

/// Perfwin Python 模块
///
/// 提供 Windows 系统性能监控功能
///
/// 示例:
///     from perfwin import Monitor, ProcessFilter
///
///     # 创建进程筛选器
///     filter = ProcessFilter(name="chrome.exe")
///
///     # 使用上下文管理器
///     with Monitor(interval=1.0, process_filter=filter) as m:
///         time.sleep(10)
///
///     # 获取结果
///     result = m.get_result()
///     for sample in result.samples:
///         print(f"Time: {sample.timestamp}")
///         print(f"  HWiNFO sensors: {len(sample.hwinfo_raw)}")
#[pymodule]
fn perfwin(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // 注册类
    m.add_class::<PyProcessInfo>()?;
    m.add_class::<PyAggregatedProcessInfo>()?;
    m.add_class::<PySample>()?;
    m.add_class::<PyMonitorResult>()?;
    m.add_class::<PyIterWrapper>()?;
    m.add_class::<PyProcessFilter>()?;
    m.add_class::<PyMonitor>()?;
    m.add_class::<PySensorEntry>()?;

    // 注册函数
    m.add_function(wrap_pyfunction!(list_hwinfo_sensors, m)?)?;
    m.add_function(wrap_pyfunction!(list_processes, m)?)?;

    // 模块版本
    m.add("__version__", "0.4.1")?;

    Ok(())
}