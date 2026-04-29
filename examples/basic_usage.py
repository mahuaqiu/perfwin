#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
perfdog 基本使用示例

展示如何使用 perfdog 进行系统性能监控。

注意：此示例需要在 Windows 上运行。
"""

import sys

# 检查平台
if sys.platform != "win32":
    print("错误：perfdog 只能在 Windows 上运行")
    print(f"当前平台: {sys.platform}")
    sys.exit(1)

import perfdog
import time


def example_basic_monitoring():
    """基本监控示例"""
    print("\n=== 基本监控示例 ===")

    with perfdog.Monitor(
        interval=1.0,
        duration=60,
        top_n_cpu=10,
    ) as monitor:
        for i in range(10):
            time.sleep(5)
            result = monitor.get_result()

            for sample in result.samples:
                print(f"\n时间: {sample.timestamp}")

                if sample.top_n_cpu:
                    print("系统 CPU 占用前 10:")
                    for proc in sample.top_n_cpu:
                        print(f"  {proc.name}({proc.pid}): {proc.cpu_percent:.1f}%")


def example_process_filter_by_name():
    """按进程名筛选示例"""
    print("\n=== 按进程名筛选示例 ===")

    # 监控 python 进程
    with perfdog.Monitor(
        interval=1.0,
        duration=10,
        process_filter=perfdog.ProcessFilter(name="python.exe"),
    ) as monitor:
        time.sleep(3)

        result = monitor.get_result()
        print(f"采集到 {len(result)} 个样本")

        for sample in result.samples:
            if sample.processes:
                for proc in sample.processes:
                    print(f"进程: {proc.name} (PID: {proc.pid})")
                    print(f"  CPU: {proc.cpu_percent:.1f}%")
                    print(f"  内存: {proc.working_set_mb:.1f} MB")


def example_process_filter_by_pid():
    """按 PID 筛选示例"""
    print("\n=== 按 PID 筛选示例 ===")

    import os
    my_pid = os.getpid()
    print(f"当前进程 PID: {my_pid}")

    with perfdog.Monitor(
        interval=0.5,
        duration=5,
        process_filter=perfdog.ProcessFilter(pids=[my_pid]),
    ) as monitor:
        time.sleep(2)

        result = monitor.get_result()

        for sample in result.samples:
            if sample.processes:
                proc = sample.processes[0]
                print(f"CPU: {proc.cpu_percent:.1f}%, 内存: {proc.working_set_mb:.1f} MB")


def example_system_info():
    """系统信息采集示例"""
    print("\n=== 系统信息采集示例 ===")

    with perfdog.Monitor(
        interval=1.0,
        duration=10,
        enable_sysinfo=True,
        enable_pdh=True,
    ) as monitor:
        time.sleep(3)

        result = monitor.get_result()

        for sample in result.samples:
            print(f"\n时间: {sample.timestamp}")

            if sample.system:
                sys_info = sample.system
                print(f"CPU 使用率: {sys_info.cpu.percent:.1f}%")
                print(f"内存使用率: {sys_info.memory.percent:.1f}%")
                print(f"内存使用: {sys_info.memory.used_mb:.0f} / {sys_info.memory.total_mb:.0f} MB")

                if sys_info.cpu.temperature:
                    print(f"CPU 温度: {sys_info.cpu.temperature:.1f}°C")

                if sys_info.cpu.power:
                    print(f"CPU 功耗: {sys_info.cpu.power:.1f} W")


def example_manual_control():
    """手动控制示例"""
    print("\n=== 手动控制示例 ===")

    monitor = perfdog.Monitor(
        interval=0.5,
        duration=30,
        top_n_cpu=5,
        top_n_gpu=5,
    )

    # 手动启动
    monitor.start()
    print("监控已启动...")

    # 运行一段时间
    time.sleep(5)

    # 获取中间结果
    result = monitor.get_result()
    print(f"已采集 {len(result)} 个样本")

    # 继续运行
    time.sleep(5)

    # 手动停止
    monitor.stop()
    print("监控已停止")

    # 获取最终结果
    result = monitor.get_result()
    print(f"总样本数: {len(result)}")


def example_export_to_dict():
    """导出为字典示例"""
    print("\n=== 导出为字典示例 ===")

    with perfdog.Monitor(
        interval=1.0,
        duration=5,
        top_n_cpu=5,
        enable_sysinfo=True,
    ) as monitor:
        time.sleep(3)

    result = monitor.get_result()

    # 转换为字典列表，便于 JSON 序列化
    dicts = result.to_dicts()

    print(f"导出 {len(dicts)} 个样本")

    # 可以保存为 JSON
    import json
    json_str = json.dumps(dicts, indent=2, ensure_ascii=False)
    print(f"JSON 长度: {len(json_str)} 字符")

    # 显示第一个样本
    if dicts:
        print(f"第一个样本: {dicts[0]['timestamp']}")


def example_regex_filter():
    """正则表达式筛选示例"""
    print("\n=== 正则表达式筛选示例 ===")

    # 监控所有 .exe 进程中包含 "chrome" 的
    with perfdog.Monitor(
        interval=1.0,
        duration=5,
        process_filter=perfdog.ProcessFilter(name_regex=r"(?i)chrome.*\.exe"),
    ) as monitor:
        time.sleep(2)

        result = monitor.get_result()

        print(f"采集到 {len(result)} 个样本")
        for sample in result.samples:
            if sample.processes:
                print(f"找到 {len(sample.processes)} 个 Chrome 进程")
                for proc in sample.processes:
                    print(f"  {proc.name}: CPU {proc.cpu_percent:.1f}%")


def main():
    """运行所有示例"""
    print("perfdog 使用示例")
    print("=" * 50)

    # 运行基本示例
    try:
        example_basic_monitoring()
    except Exception as e:
        print(f"基本监控示例失败: {e}")

    try:
        example_process_filter_by_pid()
    except Exception as e:
        print(f"PID 筛选示例失败: {e}")

    try:
        example_system_info()
    except Exception as e:
        print(f"系统信息示例失败: {e}")

    try:
        example_export_to_dict()
    except Exception as e:
        print(f"导出示例失败: {e}")


if __name__ == "__main__":
    main()