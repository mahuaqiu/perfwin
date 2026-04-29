# -*- coding: utf-8 -*-
"""
perfdog Python 绑定测试

注意：此测试需要在 Windows 上运行，因为 perfdog 是 Windows 专用库。
在非 Windows 平台上，所有测试将被跳过。
"""

import pytest
import sys
import os

# 检查平台，非 Windows 跳过所有测试
pytestmark = pytest.mark.skipif(
    sys.platform != "win32",
    reason="perfdog 只能在 Windows 上运行"
)

# 尝试导入 perfdog，如果失败则跳过测试
try:
    import perfdog
    import time
except ImportError:
    pytestmark = pytest.mark.skip(reason="perfdog 模块未安装")


def test_monitor_basic():
    """测试基本监控功能"""
    with perfdog.Monitor(
        interval=0.5,
        duration=10,
        enable_hwinfo=False,  # 测试环境可能没有 HWiNFO
        enable_pdh=True,
        enable_sysinfo=True,
        top_n_cpu=5,
    ) as monitor:
        time.sleep(2)  # 等待采集

        result = monitor.get_result()
        assert len(result.samples) > 0

        for sample in result.samples:
            assert sample.timestamp is not None
            assert sample.top_n_cpu is not None
            assert len(sample.top_n_cpu) <= 5

            for proc in sample.top_n_cpu:
                assert proc.pid > 0
                assert proc.name
                assert proc.cpu_percent >= 0


def test_process_filter_by_name():
    """测试按进程名筛选"""
    current_name = os.path.basename(sys.executable).lower()

    with perfdog.Monitor(
        interval=0.5,
        duration=5,
        process_filter=perfdog.ProcessFilter(name=current_name),
    ) as monitor:
        time.sleep(1)

        result = monitor.get_result()
        for sample in result.samples:
            if sample.processes:
                for proc in sample.processes:
                    assert proc.name.lower() == current_name


def test_process_filter_by_pid():
    """测试按 PID 筛选"""
    my_pid = os.getpid()

    with perfdog.Monitor(
        interval=0.5,
        duration=5,
        process_filter=perfdog.ProcessFilter(pids=[my_pid]),
    ) as monitor:
        time.sleep(1)

        result = monitor.get_result()
        for sample in result.samples:
            assert sample.processes is not None
            assert len(sample.processes) == 1
            assert sample.processes[0].pid == my_pid


def test_invalid_pid_returns_placeholder():
    """测试无效 PID 返回占位数据"""
    invalid_pid = 99999999  # 不存在的 PID

    with perfdog.Monitor(
        interval=0.5,
        duration=5,
        process_filter=perfdog.ProcessFilter(pids=[invalid_pid]),
    ) as monitor:
        time.sleep(1)

        result = monitor.get_result()
        for sample in result.samples:
            assert sample.processes is not None
            assert len(sample.processes) == 1
            assert sample.processes[0].pid == invalid_pid
            assert sample.processes[0].cpu_percent == 0.0
            assert sample.processes[0].working_set_mb == 0.0


def test_duration_auto_stop():
    """测试 duration 自动停止"""
    with perfdog.Monitor(
        interval=0.5,
        duration=2,  # 2 秒后自动停止
        top_n_cpu=5,
    ) as monitor:
        time.sleep(3)  # 等待超过 duration

        result = monitor.get_result()
        assert len(result.samples) >= 2  # 至少有 2 秒的数据


def test_monitor_context_manager():
    """测试上下文管理器模式"""
    monitor = perfdog.Monitor(interval=0.5, duration=5)

    # 手动启动和停止
    monitor.start()
    assert monitor.is_running()
    time.sleep(1)
    monitor.stop()
    assert not monitor.is_running()

    # 上下文管理器模式
    with perfdog.Monitor(interval=0.5, duration=5) as m:
        assert m.is_running()
        time.sleep(0.5)

    assert not m.is_running()


def test_monitor_result_iteration():
    """测试结果迭代"""
    with perfdog.Monitor(
        interval=0.5,
        duration=2,
        top_n_cpu=3,
    ) as monitor:
        time.sleep(1.5)

    result = monitor.get_result()

    # 测试 __len__
    assert len(result) > 0

    # 测试 __getitem__
    first_sample = result[0]
    assert first_sample.timestamp is not None

    # 测试 __iter__
    count = 0
    for sample in result:
        assert sample.timestamp is not None
        count += 1
    assert count == len(result)


def test_monitor_result_to_dicts():
    """测试结果转换为字典列表"""
    with perfdog.Monitor(
        interval=0.5,
        duration=2,
        top_n_cpu=3,
    ) as monitor:
        time.sleep(1.5)

    result = monitor.get_result()
    dicts = result.to_dicts()

    assert len(dicts) == len(result)

    for d in dicts:
        assert "timestamp" in d
        if "top_n_cpu" in d and d["top_n_cpu"]:
            for proc in d["top_n_cpu"]:
                assert "pid" in proc
                assert "name" in proc
                assert "cpu_percent" in proc


def test_process_filter_by_regex():
    """测试按进程名正则表达式筛选"""
    # 使用一个常见的进程名模式
    with perfdog.Monitor(
        interval=0.5,
        duration=3,
        process_filter=perfdog.ProcessFilter(name_regex=r".*\.exe"),
    ) as monitor:
        time.sleep(1)

        result = monitor.get_result()
        for sample in result.samples:
            if sample.processes:
                for proc in sample.processes:
                    assert proc.name.endswith(".exe")


def test_process_filter_invalid():
    """测试无效的进程筛选器参数"""
    with pytest.raises(ValueError):
        perfdog.ProcessFilter()  # 必须提供 pids, name 或 name_regex


def test_top_n_gpu():
    """测试 Top N GPU 进程获取"""
    with perfdog.Monitor(
        interval=0.5,
        duration=3,
        top_n_gpu=5,
    ) as monitor:
        time.sleep(1)

        result = monitor.get_result()
        for sample in result.samples:
            if sample.top_n_gpu:
                assert len(sample.top_n_gpu) <= 5


def test_system_info():
    """测试系统信息采集"""
    with perfdog.Monitor(
        interval=0.5,
        duration=3,
        enable_sysinfo=True,
        enable_pdh=True,
    ) as monitor:
        time.sleep(1)

        result = monitor.get_result()
        for sample in result.samples:
            if sample.system:
                # CPU 信息
                assert sample.system.cpu.percent >= 0
                assert sample.system.cpu.percent <= 100

                # 内存信息
                assert sample.system.memory.percent >= 0
                assert sample.system.memory.percent <= 100
                assert sample.system.memory.total_mb > 0
                assert sample.system.memory.used_mb >= 0


def test_buffer_len():
    """测试缓冲区长度"""
    with perfdog.Monitor(
        interval=0.3,
        duration=5,
        top_n_cpu=3,
    ) as monitor:
        time.sleep(0.5)

        # 缓冲区应该有数据
        assert monitor.buffer_len() > 0

        # 获取结果后缓冲区应清空
        result = monitor.get_result()
        assert monitor.buffer_len() == 0
        assert len(result) > 0


def test_interval_property():
    """测试 interval 属性"""
    monitor = perfdog.Monitor(interval=0.75, duration=5)
    assert monitor.interval == 0.75


def test_duration_property():
    """测试 duration 属性"""
    monitor = perfdog.Monitor(interval=0.5, duration=10.0)
    assert monitor.duration == 10.0

    monitor_no_duration = perfdog.Monitor(interval=0.5)
    assert monitor_no_duration.duration is None


def test_enable_flags():
    """测试启用标志属性"""
    monitor = perfdog.Monitor(
        interval=0.5,
        enable_hwinfo=False,
        enable_pdh=True,
        enable_sysinfo=True,
    )

    assert monitor.enable_hwinfo is False
    assert monitor.enable_pdh is True
    assert monitor.enable_sysinfo is True