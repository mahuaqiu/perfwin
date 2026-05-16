"""
perfwin - Windows 系统性能监控模块

基于 Rust 编写的 Python 扩展，提供高性能的系统性能数据采集。
"""

import os
import sys
from pathlib import Path

# 导入 Rust 扩展模块
from .perfwin import *

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
    enable_pdh=True,
    enable_sysinfo=True,
    hwinfo_path=None,
    process_filter=None,
    top_n_cpu=None,
    top_n_gpu=None,
    enable_aggregation=True,
):
    """创建性能监控实例

    用于采集系统性能数据，支持 CPU/GPU/内存/网络等指标。
    HWiNFO 强制启用以获取温度/功耗数据。

    示例:
        with Monitor() as m:
            time.sleep(10)
        result = m.get_result()

    参数:
        interval: 采样间隔（秒），最小值 1.0
        duration: 监控时长（秒），None 表示无限
        enable_pdh: 是否启用 GPU 采集
        enable_sysinfo: 是否启用系统信息采集
        hwinfo_path: HWiNFO64 路径，默认自动搜索
        process_filter: 进程筛选器
        top_n_cpu: Top N CPU 进程数量
        top_n_gpu: Top N GPU 进程数量
        enable_aggregation: 是否生成汇总数据（进程名筛选时有效）
    """
    # interval 校验
    if interval < 1.0:
        raise ValueError("采集间隔不能小于 1 秒")

    # 自动搜索 HWiNFO 路径
    if hwinfo_path is None:
        hwinfo_path = _find_hwinfo_path()

    return _RustMonitor(
        interval=interval,
        duration=duration,
        enable_pdh=enable_pdh,
        enable_sysinfo=enable_sysinfo,
        hwinfo_path=hwinfo_path,
        process_filter=process_filter,
        top_n_cpu=top_n_cpu,
        top_n_gpu=top_n_gpu,
        enable_aggregation=enable_aggregation,
        _module_path=str(Path(__file__).parent),
    )

__version__ = "0.3.4"