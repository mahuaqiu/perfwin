use pyo3::prelude::*;

/// Perfdog Python module
#[pymodule]
fn perfdog(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // 模块初始化 - 后续添加类
    Ok(())
}