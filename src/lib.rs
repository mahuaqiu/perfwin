use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::sync::Arc;
use parking_lot::Mutex;

pub mod data;
pub mod ring_buffer;
pub mod collector;
pub mod hwinfo_manager;
pub mod monitor;

use crate::data::{
    CPUInfo, GPUInfo, MemoryInfo, NetworkInfo, SystemInfo,
    ProcessInfo, Sample, MonitorConfig, ProcessFilter,
};
use crate::monitor::MonitorCore;

// ============================================================================
// PyO3 绑定类
// ============================================================================

// ----------------------------------------------------------------------------
// PyCPUInfo - CPU 信息类
// ----------------------------------------------------------------------------

/// CPU 信息类
///
/// 包含 CPU 使用率、温度、功率等信息
#[pyclass(name = "CPUInfo")]
#[derive(Debug, Clone)]
pub struct PyCPUInfo {
    inner: CPUInfo,
}

#[pymethods]
impl PyCPUInfo {
    #[new]
    fn new() -> Self {
        Self {
            inner: CPUInfo::default(),
        }
    }

    /// CPU 使用率百分比 (0-100)
    #[getter]
    fn percent(&self) -> f64 {
        self.inner.percent
    }

    /// CPU 温度 (摄氏度)，可能为 None
    #[getter]
    fn temperature(&self) -> Option<f64> {
        self.inner.temperature
    }

    /// CPU 功耗 (瓦特)，可能为 None
    #[getter]
    fn power(&self) -> Option<f64> {
        self.inner.power
    }

    fn __repr__(&self) -> String {
        format!(
            "CPUInfo(percent={:.1}%, temperature={:?}, power={:?})",
            self.inner.percent, self.inner.temperature, self.inner.power
        )
    }
}

impl From<CPUInfo> for PyCPUInfo {
    fn from(info: CPUInfo) -> Self {
        Self { inner: info }
    }
}

// ----------------------------------------------------------------------------
// PyGPUInfo - GPU 信息类
// ----------------------------------------------------------------------------

/// GPU 信息类
///
/// 包含 GPU 使用率、温度、功率、显存等信息
#[pyclass(name = "GPUInfo")]
#[derive(Debug, Clone)]
pub struct PyGPUInfo {
    inner: GPUInfo,
}

#[pymethods]
impl PyGPUInfo {
    #[new]
    fn new() -> Self {
        Self {
            inner: GPUInfo::default(),
        }
    }

    /// GPU 使用率百分比 (0-100)
    #[getter]
    fn percent(&self) -> f64 {
        self.inner.percent
    }

    /// GPU 温度 (摄氏度)，可能为 None
    #[getter]
    fn temperature(&self) -> Option<f64> {
        self.inner.temperature
    }

    /// GPU 功耗 (瓦特)，可能为 None
    #[getter]
    fn power(&self) -> Option<f64> {
        self.inner.power
    }

    /// GPU 显存使用量 (MB)，可能为 None
    #[getter]
    fn memory_mb(&self) -> Option<f64> {
        self.inner.memory_mb
    }

    fn __repr__(&self) -> String {
        format!(
            "GPUInfo(percent={:.1}%, temperature={:?}, power={:?}, memory_mb={:?})",
            self.inner.percent, self.inner.temperature, self.inner.power, self.inner.memory_mb
        )
    }
}

impl From<GPUInfo> for PyGPUInfo {
    fn from(info: GPUInfo) -> Self {
        Self { inner: info }
    }
}

// ----------------------------------------------------------------------------
// PyMemoryInfo - 内存信息类
// ----------------------------------------------------------------------------

/// 内存信息类
///
/// 包含内存使用率、已用/总量、提交内存等信息
#[pyclass(name = "MemoryInfo")]
#[derive(Debug, Clone)]
pub struct PyMemoryInfo {
    inner: MemoryInfo,
}

#[pymethods]
impl PyMemoryInfo {
    #[new]
    fn new() -> Self {
        Self {
            inner: MemoryInfo::default(),
        }
    }

    /// 内存使用率百分比 (0-100)
    #[getter]
    fn percent(&self) -> f64 {
        self.inner.percent
    }

    /// 已用内存 (MB)
    #[getter]
    fn used_mb(&self) -> f64 {
        self.inner.used_mb
    }

    /// 总内存 (MB)
    #[getter]
    fn total_mb(&self) -> f64 {
        self.inner.total_mb
    }

    /// 已提交内存 (MB)
    #[getter]
    fn committed_mb(&self) -> f64 {
        self.inner.committed_mb
    }

    /// 提交内存上限 (MB)
    #[getter]
    fn committed_limit_mb(&self) -> f64 {
        self.inner.committed_limit_mb
    }

    fn __repr__(&self) -> String {
        format!(
            "MemoryInfo(percent={:.1}%, used={:.1}MB, total={:.1}MB)",
            self.inner.percent, self.inner.used_mb, self.inner.total_mb
        )
    }
}

impl From<MemoryInfo> for PyMemoryInfo {
    fn from(info: MemoryInfo) -> Self {
        Self { inner: info }
    }
}

// ----------------------------------------------------------------------------
// PyNetworkInfo - 网络信息类
// ----------------------------------------------------------------------------

/// 网络信息类
///
/// 包含上传/下载速度等信息
#[pyclass(name = "NetworkInfo")]
#[derive(Debug, Clone)]
pub struct PyNetworkInfo {
    inner: NetworkInfo,
}

#[pymethods]
impl PyNetworkInfo {
    #[new]
    fn new() -> Self {
        Self {
            inner: NetworkInfo::default(),
        }
    }

    /// 上传速度 (bytes/s)
    #[getter]
    fn upload_speed(&self) -> f64 {
        self.inner.upload_speed
    }

    /// 下载速度 (bytes/s)
    #[getter]
    fn download_speed(&self) -> f64 {
        self.inner.download_speed
    }

    fn __repr__(&self) -> String {
        format!(
            "NetworkInfo(upload={:.1}B/s, download={:.1}B/s)",
            self.inner.upload_speed, self.inner.download_speed
        )
    }
}

impl From<NetworkInfo> for PyNetworkInfo {
    fn from(info: NetworkInfo) -> Self {
        Self { inner: info }
    }
}

// ----------------------------------------------------------------------------
// PySystemInfo - 系统信息类
// ----------------------------------------------------------------------------

/// 系统信息类
///
/// 包含 CPU、GPU、内存、网络等系统级性能数据
#[pyclass(name = "SystemInfo")]
#[derive(Debug, Clone)]
pub struct PySystemInfo {
    inner: SystemInfo,
}

#[pymethods]
impl PySystemInfo {
    #[new]
    fn new() -> Self {
        Self {
            inner: SystemInfo::default(),
        }
    }

    /// CPU 信息
    #[getter]
    fn cpu(&self) -> PyCPUInfo {
        PyCPUInfo::from(self.inner.cpu.clone())
    }

    /// GPU 信息
    #[getter]
    fn gpu(&self) -> PyGPUInfo {
        PyGPUInfo::from(self.inner.gpu.clone())
    }

    /// 内存信息
    #[getter]
    fn memory(&self) -> PyMemoryInfo {
        PyMemoryInfo::from(self.inner.memory.clone())
    }

    /// 网络信息
    #[getter]
    fn network(&self) -> PyNetworkInfo {
        PyNetworkInfo::from(self.inner.network.clone())
    }

    fn __repr__(&self) -> String {
        format!(
            "SystemInfo(cpu={:.1}%, gpu={:.1}%, memory={:.1}%)",
            self.inner.cpu.percent, self.inner.gpu.percent, self.inner.memory.percent
        )
    }
}

impl From<SystemInfo> for PySystemInfo {
    fn from(info: SystemInfo) -> Self {
        Self { inner: info }
    }
}

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
                timestamp: chrono::Utc::now(),
                system: None,
                processes: None,
                top_n_cpu: None,
                top_n_gpu: None,
            },
        }
    }

    /// 采样时间戳 (ISO 8601 格式字符串)
    #[getter]
    fn timestamp(&self) -> String {
        self.inner.timestamp.to_rfc3339()
    }

    /// 系统信息，可能为 None
    #[getter]
    fn system(&self) -> Option<PySystemInfo> {
        self.inner.system.as_ref().map(|s| PySystemInfo::from(s.clone()))
    }

    /// 目标进程列表，可能为 None
    #[getter]
    fn processes(&self) -> Option<Vec<PyProcessInfo>> {
        self.inner.processes.as_ref().map(|procs| {
            procs.iter().map(|p| PyProcessInfo::from(p.clone())).collect()
        })
    }

    /// Top N CPU 进程列表，可能为 None
    #[getter]
    fn top_n_cpu(&self) -> Option<Vec<PyProcessInfo>> {
        self.inner.top_n_cpu.as_ref().map(|procs| {
            procs.iter().map(|p| PyProcessInfo::from(p.clone())).collect()
        })
    }

    /// Top N GPU 进程列表，可能为 None
    #[getter]
    fn top_n_gpu(&self) -> Option<Vec<PyProcessInfo>> {
        self.inner.top_n_gpu.as_ref().map(|procs| {
            procs.iter().map(|p| PyProcessInfo::from(p.clone())).collect()
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "Sample(timestamp='{}', processes={:?})",
            self.inner.timestamp.format("%Y-%m-%d %H:%M:%S"),
            self.inner.processes.as_ref().map(|p| p.len())
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
    fn to_dicts(&self, py: Python<'_>) -> PyResult<Vec<Bound<'_, PyDict>>> {
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
    fn to_dict(&self, py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
        let dict = PyDict::new(py);

        dict.set_item("timestamp", self.inner.timestamp.to_rfc3339())?;

        if let Some(system) = &self.inner.system {
            let sys_dict = PyDict::new(py);

            let cpu_dict = PyDict::new(py);
            cpu_dict.set_item("percent", system.cpu.percent)?;
            cpu_dict.set_item("temperature", system.cpu.temperature)?;
            cpu_dict.set_item("power", system.cpu.power)?;
            sys_dict.set_item("cpu", cpu_dict)?;

            let gpu_dict = PyDict::new(py);
            gpu_dict.set_item("percent", system.gpu.percent)?;
            gpu_dict.set_item("temperature", system.gpu.temperature)?;
            gpu_dict.set_item("power", system.gpu.power)?;
            gpu_dict.set_item("memory_mb", system.gpu.memory_mb)?;
            sys_dict.set_item("gpu", gpu_dict)?;

            let mem_dict = PyDict::new(py);
            mem_dict.set_item("percent", system.memory.percent)?;
            mem_dict.set_item("used_mb", system.memory.used_mb)?;
            mem_dict.set_item("total_mb", system.memory.total_mb)?;
            mem_dict.set_item("committed_mb", system.memory.committed_mb)?;
            mem_dict.set_item("committed_limit_mb", system.memory.committed_limit_mb)?;
            sys_dict.set_item("memory", mem_dict)?;

            let net_dict = PyDict::new(py);
            net_dict.set_item("upload_speed", system.network.upload_speed)?;
            net_dict.set_item("download_speed", system.network.download_speed)?;
            sys_dict.set_item("network", net_dict)?;

            dict.set_item("system", sys_dict)?;
        }

        if let Some(processes) = &self.inner.processes {
            let procs_list = PyList::empty(py);
            for proc in processes {
                let proc_dict = PyDict::new(py);
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

        if let Some(top_n) = &self.inner.top_n_cpu {
            let list = process_list_to_pylist(py, top_n)?;
            dict.set_item("top_n_cpu", list)?;
        }

        if let Some(top_n) = &self.inner.top_n_gpu {
            let list = process_list_to_pylist(py, top_n)?;
            dict.set_item("top_n_gpu", list)?;
        }

        Ok(dict)
    }
}

/// 将进程列表转换为 Python 列表
fn process_list_to_pylist(py: Python<'_>, processes: &[ProcessInfo]) -> PyResult<Bound<'_, PyList>> {
    let list = PyList::empty(py);
    for proc in processes {
        let dict = PyDict::new(py);
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
///     # 按进程名精确匹配
///     filter = ProcessFilter(name="chrome.exe")
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
    #[pyo3(signature = (pids=None, name=None, name_regex=None))]
    fn new(
        pids: Option<Vec<u32>>,
        name: Option<String>,
        name_regex: Option<String>,
    ) -> PyResult<Self> {
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

    fn __repr__(&self) -> String {
        match &self.inner {
            ProcessFilter::Pids(pids) => format!("ProcessFilter(pids={:?})", pids),
            ProcessFilter::Name(name) => format!("ProcessFilter(name='{}')", name),
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
    /// - `interval`: 采样间隔 (秒)，默认 1.0
    /// - `duration`: 监控时长 (秒)，None 表示无限，默认 None
    /// - `enable_hwinfo`: 是否启用 HWiNFO，默认 False
    /// - `enable_pdh`: 是否启用 PDH，默认 True
    /// - `enable_sysinfo`: 是否启用系统信息采集，默认 True
    /// - `hwinfo_path`: HWiNFO 路径，默认 None
    /// - `process_filter`: 进程筛选器，默认 None
    /// - `top_n_cpu`: 获取 Top N CPU 进程，默认 None
    /// - `top_n_gpu`: 获取 Top N GPU 进程，默认 None
    #[new]
    #[pyo3(signature = (
        interval=1.0,
        duration=None,
        enable_hwinfo=false,
        enable_pdh=true,
        enable_sysinfo=true,
        hwinfo_path=None,
        process_filter=None,
        top_n_cpu=None,
        top_n_gpu=None
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
        let filter = process_filter.map(|f| f.inner);

        let config = MonitorConfig {
            interval,
            duration,
            enable_hwinfo,
            enable_pdh,
            enable_sysinfo,
            hwinfo_path,
            process_filter: filter,
            top_n_cpu,
            top_n_gpu,
        };

        // 创建 MonitorCore 实例
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
    fn start(slf: Py<Self>) -> PyResult<()> {
        let inner = slf.borrow();
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
    fn stop(slf: Py<Self>) -> PyResult<()> {
        let inner = slf.borrow();
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
    fn get_result(slf: Py<Self>) -> PyResult<PyMonitorResult> {
        let inner = slf.borrow();
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
    fn is_running(slf: Py<Self>) -> bool {
        let inner = slf.borrow();
        if let Some(core) = &inner.core {
            let core_guard = core.lock();
            core_guard.is_running()
        } else {
            false
        }
    }

    /// 获取缓冲区中的数据数量
    fn buffer_len(slf: Py<Self>) -> usize {
        let inner = slf.borrow();
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

    /// 是否启用 HWiNFO
    #[getter]
    fn enable_hwinfo(&self) -> bool {
        self.config.enable_hwinfo
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

    /// 进入上下文管理器
    ///
    /// 返回 `Py<Self>` 以支持 `with Monitor(...) as m:` 语法
    fn __enter__(slf: Py<Self>) -> PyResult<Py<Self>> {
        // 启动监控
        {
            let inner = slf.borrow();
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
        _exc_type: &Bound<'_, PyAny>,
        _exc_value: &Bound<'_, PyAny>,
        _traceback: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        // 停止监控
        {
            let inner = slf.borrow();
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

// ============================================================================
// 模块定义
// ============================================================================

/// Perfdog Python 模块
///
/// 提供 Windows 系统性能监控功能
///
/// 示例:
///     from perfdog import Monitor, ProcessFilter
///
///     # 创建进程筛选器
///     filter = ProcessFilter(name="chrome.exe")
///
///     # 使用上下文管理器
///     with Monitor(interval=0.5, process_filter=filter) as m:
///         time.sleep(10)
///
///     # 获取结果
///     result = m.get_result()
///     for sample in result.samples:
///         print(f"Time: {sample.timestamp}")
///         if sample.system:
///             print(f"  CPU: {sample.system.cpu.percent}%")
#[pymodule]
fn perfdog(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // 注册类
    m.add_class::<PyCPUInfo>()?;
    m.add_class::<PyGPUInfo>()?;
    m.add_class::<PyMemoryInfo>()?;
    m.add_class::<PyNetworkInfo>()?;
    m.add_class::<PySystemInfo>()?;
    m.add_class::<PyProcessInfo>()?;
    m.add_class::<PySample>()?;
    m.add_class::<PyMonitorResult>()?;
    m.add_class::<PyIterWrapper>()?;
    m.add_class::<PyProcessFilter>()?;
    m.add_class::<PyMonitor>()?;

    // 模块版本
    m.add("__version__", "0.1.0")?;

    Ok(())
}