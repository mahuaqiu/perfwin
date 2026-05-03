"""
perfdog - Windows 系统性能监控模块

基于 Rust 编写的 Python 扩展，提供高性能的系统性能数据采集。
"""

import os
import sys
from pathlib import Path

# 导入 Rust 扩展模块
from .perfdog import *

# 保存原始 Monitor 类
_RustMonitor = Monitor

def _find_hwinfo_path():
    """搜索 HWiNFO64.EXE 路径"""
    # 可能的路径列表
    possible_paths = [
        # 模块目录下的 HWiNFO64
        Path(__file__).parent / "HWiNFO64" / "HWiNFO64.EXE",
        # 项目根目录下的 HWiNFO64（开发模式）
        Path(__file__).parent.parent.parent / "HWiNFO64" / "HWiNFO64.EXE",
        # 当前工作目录下的 HWiNFO64
        Path.cwd() / "HWiNFO64" / "HWiNFO64.EXE",
    ]

    for path in possible_paths:
        if path.exists():
            return str(path)

    return None

def Monitor(
    interval=1.0,
    duration=None,
    enable_hwinfo=False,
    enable_pdh=True,
    enable_sysinfo=True,
    hwinfo_path=None,
    process_filter=None,
    top_n_cpu=None,
    top_n_gpu=None,
):
    """创建性能监控实例

    用于采集系统性能数据，支持 CPU/GPU/内存/网络等指标。

    示例:
        with Monitor(enable_hwinfo=True) as m:
            time.sleep(10)
        result = m.get_result()

    参数:
        enable_hwinfo: 是否启用 HWiNFO（获取温度/功耗数据）
        hwinfo_path: HWiNFO64 路径，默认自动搜索
    """
    # 如果启用 hwinfo 但未指定路径，自动搜索
    if enable_hwinfo and hwinfo_path is None:
        hwinfo_path = _find_hwinfo_path()

    return _RustMonitor(
        interval=interval,
        duration=duration,
        enable_hwinfo=enable_hwinfo,
        enable_pdh=enable_pdh,
        enable_sysinfo=enable_sysinfo,
        hwinfo_path=hwinfo_path,
        process_filter=process_filter,
        top_n_cpu=top_n_cpu,
        top_n_gpu=top_n_gpu,
    )

__version__ = "0.1.0"