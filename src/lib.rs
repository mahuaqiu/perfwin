use pyo3::prelude::*;

pub mod data;
pub mod ring_buffer;
pub mod collector;
pub mod hwinfo_manager;
pub mod monitor;

/// Perfdog Python module
#[pymodule]
fn perfdog(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // 模块初始化 - 后续添加类
    Ok(())
}