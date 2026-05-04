# -*- coding: utf-8 -*-
"""
perfwin Python 绑定测试

注意：此测试需要在 Windows 上运行，因为 perfwin 是 Windows 专用库。
在非 Windows 平台上，所有测试将被跳过。
"""

import pytest
import sys
import os

# 检查平台，非 Windows 跳过所有测试
pytestmark = pytest.mark.skipif(
    sys.platform != "win32",
    reason="perfwin 只能在 Windows 上运行"
)

# 尝试导入 perfwin，如果失败则跳过测试
try:
    import perfwin
    import time
except ImportError:
    pytestmark = pytest.mark.skip(reason="perfwin 模块未安装")


def test_interval_validation():
    """测试 interval 参数校验"""
    # interval < 1.0 应该报错
    with pytest.raises(ValueError, match="采集间隔不能小于 1 秒"):
        perfwin.Monitor(interval=0.5)


def test_list_processes():
    """测试获取进程列表"""
    processes = perfwin.list_processes()
    assert len(processes) > 0
    # 检查格式 (pid, name)
    for pid, name in processes:
        assert isinstance(pid, int)
        assert isinstance(name, str)
        assert pid >= 0  # PID 可能为 0（系统空闲进程）


def test_monitor_basic():
    """测试基本监控功能"""
    with perfwin.Monitor(
        interval=1.0,
        duration=5,
        enable_pdh=True,
        enable_sysinfo=True,
        top_n_cpu=5,
    ) as monitor:
        time.sleep(2)  # 等待采集

        result = monitor.get_result()
        assert len(result.samples) > 0

        for sample in result.samples:
            assert sample.timestamp is not None
            # system 必须返回
            assert sample.system is not None
            assert sample.top_n_cpu is not None
            assert len(sample.top_n_cpu) <= 5

            for proc in sample.top_n_cpu:
                assert proc.pid > 0
                assert proc.name
                assert proc.cpu_percent >= 0


def test_process_filter_by_name():
    """测试按进程名筛选"""
    current_name = os.path.basename(sys.executable).lower()

    with perfwin.Monitor(
        interval=1.0,
        duration=5,
        process_filter=perfwin.ProcessFilter(name=current_name),
    ) as monitor:
        time.sleep(2)

        result = monitor.get_result()
        for sample in result.samples:
            if sample.processes:
                for proc in sample.processes:
                    assert proc.name.lower() == current_name


def test_process_filter_by_names():
    """测试按多个进程名筛选"""
    current_name = os.path.basename(sys.executable).lower()

    with perfwin.Monitor(
        interval=1.0,
        duration=5,
        process_filter=perfwin.ProcessFilter(names=[current_name, "explorer.exe"]),
    ) as monitor:
        time.sleep(2)

        result = monitor.get_result()
        for sample in result.samples:
            if sample.processes:
                names = [proc.name.lower() for proc in sample.processes]
                assert current_name in names


def test_process_filter_by_pid():
    """测试按 PID 筛选"""
    my_pid = os.getpid()

    with perfwin.Monitor(
        interval=1.0,
        duration=5,
        process_filter=perfwin.ProcessFilter(pids=[my_pid]),
    ) as monitor:
        time.sleep(2)

        result = monitor.get_result()
        for sample in result.samples:
            assert sample.processes is not None
            assert len(sample.processes) == 1
            assert sample.processes[0].pid == my_pid


def test_invalid_pid_returns_placeholder():
    """测试无效 PID 返回占位数据"""
    invalid_pid = 99999999  # 不存在的 PID

    with perfwin.Monitor(
        interval=1.0,
        duration=5,
        process_filter=perfwin.ProcessFilter(pids=[invalid_pid]),
    ) as monitor:
        time.sleep(2)

        result = monitor.get_result()
        for sample in result.samples:
            assert sample.processes is not None
            assert len(sample.processes) == 1
            assert sample.processes[0].pid == invalid_pid
            assert sample.processes[0].cpu_percent == 0.0
            assert sample.processes[0].working_set_mb == 0.0


def test_aggregated_data():
    """测试汇总数据"""
    current_name = os.path.basename(sys.executable).lower()

    with perfwin.Monitor(
        interval=1.0,
        duration=5,
        process_filter=perfwin.ProcessFilter(name=current_name),
        enable_aggregation=True,
    ) as monitor:
        time.sleep(2)

        result = monitor.get_result()
        for sample in result.samples:
            if sample.aggregated:
                assert len(sample.aggregated) > 0
                agg = sample.aggregated[0]
                assert agg.name.lower() == current_name
                assert agg.process_count > 0
                assert len(agg.pids) == agg.process_count


def test_duration_auto_stop():
    """测试 duration 自动停止"""
    with perfwin.Monitor(
        interval=1.0,
        duration=4,  # 4 秒后自动停止
        top_n_cpu=5,
    ) as monitor:
        time.sleep(5)  # 等待超过 duration

        result = monitor.get_result()
        assert len(result.samples) >= 3  # 至少有 3 秒的数据


def test_monitor_context_manager():
    """测试上下文管理器模式"""
    monitor = perfwin.Monitor(interval=1.0, duration=5)

    # 手动启动和停止
    monitor.start()
    assert monitor.is_running()
    time.sleep(2)
    monitor.stop()
    assert not monitor.is_running()

    # 上下文管理器模式
    with perfwin.Monitor(interval=1.0, duration=5) as m:
        assert m.is_running()
        time.sleep(1)

    assert not m.is_running()


def test_monitor_result_iteration():
    """测试结果迭代"""
    with perfwin.Monitor(
        interval=1.0,
        duration=3,
        top_n_cpu=3,
    ) as monitor:
        time.sleep(2)

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
    with perfwin.Monitor(
        interval=1.0,
        duration=3,
        top_n_cpu=3,
    ) as monitor:
        time.sleep(2)

    result = monitor.get_result()
    dicts = result.to_dicts()

    assert len(dicts) == len(result)

    for d in dicts:
        assert "timestamp" in d
        assert "system" in d  # system 必须返回
        if "top_n_cpu" in d and d["top_n_cpu"]:
            for proc in d["top_n_cpu"]:
                assert "pid" in proc
                assert "name" in proc
                assert "cpu_percent" in proc


def test_process_filter_by_regex():
    """测试按进程名正则表达式筛选"""
    with perfwin.Monitor(
        interval=1.0,
        duration=3,
        process_filter=perfwin.ProcessFilter(name_regex=r".*\.exe"),
    ) as monitor:
        time.sleep(2)

        result = monitor.get_result()
        for sample in result.samples:
            if sample.processes:
                for proc in sample.processes:
                    assert proc.name.endswith(".exe")


def test_process_filter_invalid():
    """测试无效的进程筛选器参数"""
    with pytest.raises(ValueError):
        perfwin.ProcessFilter()  # 必须提供 pids, name, names 或 name_regex


def test_top_n_gpu():
    """测试 Top N GPU 进程获取"""
    with perfwin.Monitor(
        interval=1.0,
        duration=3,
        top_n_gpu=5,
    ) as monitor:
        time.sleep(2)

        result = monitor.get_result()
        for sample in result.samples:
            if sample.top_n_gpu:
                assert len(sample.top_n_gpu) <= 5


def test_system_info():
    """测试系统信息采集"""
    with perfwin.Monitor(
        interval=1.0,
        duration=3,
        enable_sysinfo=True,
        enable_pdh=True,
    ) as monitor:
        time.sleep(2)

        result = monitor.get_result()
        for sample in result.samples:
            # system 必须返回
            assert sample.system is not None

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
    with perfwin.Monitor(
        interval=1.0,
        duration=5,
        top_n_cpu=3,
    ) as monitor:
        time.sleep(2)

        # 缓冲区应该有数据
        assert monitor.buffer_len() > 0

        # 获取结果后缓冲区应清空
        result = monitor.get_result()
        assert monitor.buffer_len() == 0
        assert len(result) > 0


def test_interval_property():
    """测试 interval 属性"""
    monitor = perfwin.Monitor(interval=1.5, duration=5)
    assert monitor.interval == 1.5


def test_duration_property():
    """测试 duration 属性"""
    monitor = perfwin.Monitor(interval=1.0, duration=10.0)
    assert monitor.duration == 10.0

    monitor_no_duration = perfwin.Monitor(interval=1.0)
    assert monitor_no_duration.duration is None


def test_enable_flags():
    """测试启用标志属性"""
    monitor = perfwin.Monitor(
        interval=1.0,
        enable_pdh=True,
        enable_sysinfo=True,
        enable_aggregation=True,
    )

    assert monitor.enable_pdh is True
    assert monitor.enable_sysinfo is True
    assert monitor.enable_aggregation is True