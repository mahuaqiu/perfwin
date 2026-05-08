#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
perfwin Python 绑定测试
"""

import sys
import pytest

# 检查平台
if sys.platform != "win32":
    pytest.skip("perfwin 仅支持 Windows", allow_module_level=True)

import perfwin
import time


def test_list_processes():
    """测试获取进程列表"""
    processes = perfwin.list_processes()
    assert len(processes) > 0, "应该至少有一个进程"

    # 验证数据结构
    for pid, name in processes[:10]:
        assert isinstance(pid, int), "PID 应该是整数"
        assert isinstance(name, str), "进程名应该是字符串"
        assert len(name) > 0, "进程名不应为空"


def test_interval_validation():
    """测试 interval 参数校验"""
    # interval 必须 >= 1.0
    with pytest.raises(ValueError) as exc_info:
        perfwin.Monitor(interval=0.5)
    # 错误信息是"采集间隔不能小于 1 秒"
    assert "不能小于" in str(exc_info.value) or "1 秒" in str(exc_info.value)


def test_hwinfo_raw_basic():
    """测试 hwinfo_raw 基本功能"""
    with perfwin.Monitor(interval=1.0, duration=2) as monitor:
        time.sleep(3)

    result = monitor.get_result()
    assert len(result.samples) > 0, "应该有采样数据"

    # 验证 hwinfo_raw 字段
    sample = result.samples[0]
    assert sample.hwinfo_raw is not None, "hwinfo_raw 不应为 None"
    assert len(sample.hwinfo_raw) > 100, "应该至少有 100 个传感器"

    # 验证数据结构
    sample_count = 0
    for name, data in sample.hwinfo_raw.items():
        sample_count += 1
        if sample_count > 10:  # 只检查前 10 个
            break

        assert "value" in data, f"传感器 {name} 应该有 value 字段"
        assert "unit" in data, f"传感器 {name} 应该有 unit 字段"
        assert isinstance(data["value"], (int, float)), f"传感器 {name} 的 value 应该是数字"
        assert isinstance(data["unit"], str), f"传感器 {name} 的 unit 应该是字符串"


def test_process_filter_by_name():
    """测试按进程名筛选"""
    process_filter = perfwin.ProcessFilter(name="python.exe")

    with perfwin.Monitor(
        interval=1.0,
        duration=2,
        process_filter=process_filter,
    ) as monitor:
        time.sleep(3)

    result = monitor.get_result()
    assert len(result.samples) > 0, "应该有采样数据"

    # 验证 hwinfo_raw 字段存在
    for sample in result.samples:
        assert sample.hwinfo_raw is not None
        assert len(sample.hwinfo_raw) > 100


def test_aggregation():
    """测试汇总数据"""
    process_filter = perfwin.ProcessFilter(name="python.exe")

    with perfwin.Monitor(
        interval=1.0,
        duration=2,
        process_filter=process_filter,
        enable_aggregation=True,
    ) as monitor:
        time.sleep(3)

    result = monitor.get_result()

    # 检查汇总数据
    has_aggregation = False
    for sample in result.samples:
        if sample.aggregated:
            has_aggregation = True
            for agg in sample.aggregated:
                assert agg.name == "python.exe"
                assert agg.process_count > 0
                assert len(agg.pids) == agg.process_count

    # 如果没有 python.exe 进程，至少应该验证 hwinfo_raw
    if not has_aggregation:
        assert result.samples[0].hwinfo_raw is not None


def test_top_n():
    """测试 Top N 进程"""
    with perfwin.Monitor(
        interval=1.0,
        duration=2,
        top_n_cpu=5,
        top_n_gpu=5,
    ) as monitor:
        time.sleep(3)

    result = monitor.get_result()

    for sample in result.samples[:2]:
        # 验证 hwinfo_raw
        assert sample.hwinfo_raw is not None
        assert len(sample.hwinfo_raw) > 100

        # 验证 Top N
        if sample.top_n_cpu:
            assert len(sample.top_n_cpu) <= 5
            for proc in sample.top_n_cpu:
                assert proc.name
                assert proc.pid > 0

        if sample.top_n_gpu:
            assert len(sample.top_n_gpu) <= 5
            for proc in sample.top_n_gpu:
                assert proc.name
                assert proc.pid > 0


def test_to_dict():
    """测试转换为字典"""
    with perfwin.Monitor(interval=1.0, duration=2) as monitor:
        time.sleep(3)

    result = monitor.get_result()
    dicts = result.to_dicts()

    assert len(dicts) > 0

    # 验证字典结构
    last_dict = dicts[-1]
    assert "hwinfo_raw" in last_dict
    assert isinstance(last_dict["hwinfo_raw"], dict)
    assert len(last_dict["hwinfo_raw"]) > 100


if __name__ == "__main__":
    pytest.main([__file__, "-v"])