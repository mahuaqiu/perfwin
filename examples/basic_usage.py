#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
perfdog 使用示例

演示 perfdog 模块的主要功能：
1. 获取系统所有进程列表
2. 按进程名/PID 筛选监控
3. 多进程名筛选
4. 汇总数据展示
5. Top N 进程排行
"""

import sys

# 检查平台
if sys.platform != "win32":
    print("错误：perfdog 只能在 Windows 上运行")
    print(f"当前平台: {sys.platform}")
    sys.exit(1)

import perfdog
import time


def example_list_processes():
    """示例 1: 获取系统所有进程列表"""
    print("\n" + "=" * 60)
    print("示例 1: 获取系统所有进程列表")
    print("=" * 60)

    processes = perfdog.list_processes()
    print(f"系统进程总数: {len(processes)}")
    print("\n前 10 个进程:")
    for pid, name in processes[:10]:
        print(f"  {name}: PID={pid}")


def example_process_filter_by_name():
    """示例 2: 按进程名筛选监控（含汇总数据）"""
    print("\n" + "=" * 60)
    print("示例 2: 按进程名筛选监控 - python.exe")
    print("=" * 60)

    # 按进程名筛选
    process_filter = perfdog.ProcessFilter(name="python.exe")

    with perfdog.Monitor(
        interval=1.0,  # 采集间隔 1 秒（最小值）
        duration=5,    # 监控 5 秒
        process_filter=process_filter,
        enable_aggregation=True,  # 启用汇总数据
    ) as monitor:
        time.sleep(6)

    result = monitor.get_result()
    print(f"\n采集到 {len(result)} 个样本")

    for sample in result.samples:
        print(f"\n时间: {sample.timestamp}")

        # 系统信息（每次必须返回）
        s = sample.system
        print(f"系统 CPU: {s.cpu.percent:.1f}% | GPU: {s.gpu.percent:.1f}% | 内存: {s.memory.percent:.1f}%")
        if s.cpu.temperature:
            print(f"CPU 温度: {s.cpu.temperature:.1f}°C | 功耗: {s.cpu.power:.1f}W")
        if s.gpu.temperature:
            print(f"GPU 温度: {s.gpu.temperature:.1f}°C | 功耗: {s.gpu.power:.1f}W")

        # 进程明细
        if sample.processes:
            print(f"\npython.exe 进程明细 ({len(sample.processes)} 个):")
            for proc in sample.processes:
                print(f"  PID {proc.pid}: CPU={proc.cpu_percent:.1f}%, MEM={proc.working_set_mb:.1f}MB, GPU={proc.gpu_percent:.1f}%")

        # 汇总数据
        if sample.aggregated:
            print(f"\n汇总数据:")
            for agg in sample.aggregated:
                print(f"  {agg.name}:")
                print(f"    进程数: {agg.process_count}")
                print(f"    PIDs: {agg.pids}")
                print(f"    CPU 总计: {agg.cpu_percent_total:.1f}%")
                print(f"    内存总计: {agg.working_set_mb_total:.1f}MB")
                print(f"    GPU 总计: {agg.gpu_percent_total:.1f}%")
                print(f"    句柄总计: {agg.handle_count_total}")


def example_process_filter_by_names():
    """示例 3: 多进程名筛选监控"""
    print("\n" + "=" * 60)
    print("示例 3: 多进程名筛选监控")
    print("=" * 60)

    # 多个进程名筛选
    process_filter = perfdog.ProcessFilter(names=["python.exe", "explorer.exe"])

    with perfdog.Monitor(
        interval=1.0,
        duration=5,
        process_filter=process_filter,
        enable_aggregation=True,
    ) as monitor:
        time.sleep(6)

    result = monitor.get_result()
    print(f"\n采集到 {len(result)} 个样本")

    for sample in result.samples[:2]:  # 只显示前2个样本
        print(f"\n时间: {sample.timestamp}")
        print(f"系统: CPU={sample.system.cpu.percent:.1f}%, GPU={sample.system.gpu.percent:.1f}%")

        if sample.aggregated:
            print("汇总:")
            for agg in sample.aggregated:
                print(f"  {agg.name}: {agg.process_count}个进程, CPU={agg.cpu_percent_total:.1f}%, MEM={agg.working_set_mb_total:.1f}MB")


def example_top_n_processes():
    """示例 4: Top N 进程排行"""
    print("\n" + "=" * 60)
    print("示例 4: Top 10 CPU/GPU 进程排行")
    print("=" * 60)

    with perfdog.Monitor(
        interval=1.0,
        duration=5,
        top_n_cpu=10,
        top_n_gpu=10,
    ) as monitor:
        time.sleep(6)

    result = monitor.get_result()
    print(f"\n采集到 {len(result)} 个样本")

    for sample in result.samples[:2]:
        print(f"\n时间: {sample.timestamp}")

        # Top N CPU 进程（同时显示 CPU 和 GPU）
        if sample.top_n_cpu:
            print("\nTop 10 CPU 进程:")
            print(f"{'进程名':<20} {'PID':<8} {'CPU':<8} {'GPU':<8}")
            print("-" * 44)
            for proc in sample.top_n_cpu:
                print(f"{proc.name:<20} {proc.pid:<8} {proc.cpu_percent:.1f}%   {proc.gpu_percent:.1f}%")

        # Top N GPU 进程
        if sample.top_n_gpu:
            print("\nTop 10 GPU 进程:")
            print(f"{'进程名':<20} {'PID':<8} {'GPU':<8} {'CPU':<8}")
            print("-" * 44)
            for proc in sample.top_n_gpu:
                print(f"{proc.name:<20} {proc.pid:<8} {proc.gpu_percent:.1f}%   {proc.cpu_percent:.1f}%")


def example_interval_validation():
    """示例 5: interval 参数校验"""
    print("\n" + "=" * 60)
    print("示例 5: interval 参数校验（最小 1 秒）")
    print("=" * 60)

    try:
        monitor = perfdog.Monitor(interval=0.5)
        print("错误：应该抛出异常")
    except ValueError as e:
        print(f"正确抛出异常: {e}")


def example_full_monitoring():
    """示例 6: 完整监控示例（所有功能）"""
    print("\n" + "=" * 60)
    print("示例 6: 完整监控示例")
    print("=" * 60)

    # 多进程名筛选 + Top N
    process_filter = perfdog.ProcessFilter(names=["python.exe"])

    with perfdog.Monitor(
        interval=1.0,
        duration=5,
        process_filter=process_filter,
        top_n_cpu=5,
        top_n_gpu=5,
        enable_aggregation=True,
    ) as monitor:
        time.sleep(6)

    result = monitor.get_result()

    # 输出最后一个样本的完整数据
    if result.samples:
        sample = result.samples[-1]
        print(f"\n=== 最终采样数据 ===")
        print(f"时间: {sample.timestamp}")

        # 系统级数据
        s = sample.system
        print(f"\n【系统级数据】")
        print(f"  CPU 使用率: {s.cpu.percent:.1f}%")
        print(f"  CPU 温度: {s.cpu.temperature:.1f}°C" if s.cpu.temperature else "  CPU 温度: N/A")
        print(f"  CPU 功耗: {s.cpu.power:.1f}W" if s.cpu.power else "  CPU 功耗: N/A")
        print(f"  GPU 使用率: {s.gpu.percent:.1f}%")
        print(f"  GPU 温度: {s.gpu.temperature:.1f}°C" if s.gpu.temperature else "  GPU 温度: N/A")
        print(f"  GPU 功耗: {s.gpu.power:.1f}W" if s.gpu.power else "  GPU 功耗: N/A")
        print(f"  内存使用率: {s.memory.percent:.1f}%")
        print(f"  上传速度: {s.network.upload_speed/1024:.1f} KB/s")
        print(f"  下载速度: {s.network.download_speed/1024:.1f} KB/s")

        # 进程汇总
        if sample.aggregated:
            print(f"\n【进程汇总】")
            for agg in sample.aggregated:
                print(f"  {agg.name}:")
                print(f"    进程数: {agg.process_count}, PIDs: {agg.pids}")
                print(f"    CPU 总计: {agg.cpu_percent_total:.1f}%")
                print(f"    内存总计: {agg.working_set_mb_total:.1f}MB")
                print(f"    提交内存总计: {agg.committed_memory_mb_total:.1f}MB")
                print(f"    GPU 总计: {agg.gpu_percent_total:.1f}%")
                print(f"    句柄总计: {agg.handle_count_total}")

        # Top N
        if sample.top_n_cpu:
            print(f"\n【Top 5 CPU 进程】")
            for proc in sample.top_n_cpu:
                print(f"  {proc.name}({proc.pid}): CPU={proc.cpu_percent:.1f}%, GPU={proc.gpu_percent:.1f}%")

    # 转换为字典格式
    print(f"\n【字典格式输出】")
    dicts = result.to_dicts()
    print(f"  样本数: {len(dicts)}")
    if dicts:
        last_dict = dicts[-1]
        print(f"  system.cpu.percent: {last_dict['system']['cpu']['percent']:.1f}%")
        if 'aggregated' in last_dict and last_dict['aggregated']:
            print(f"  aggregated[0].name: {last_dict['aggregated'][0]['name']}")


if __name__ == "__main__":
    print("=" * 60)
    print("perfdog 使用示例")
    print("=" * 60)

    try:
        # 运行所有示例
        example_list_processes()
        example_interval_validation()
        example_process_filter_by_name()
        example_process_filter_by_names()
        example_top_n_processes()
        example_full_monitoring()

        print("\n" + "=" * 60)
        print("所有示例运行完成！")
        print("=" * 60)

    except KeyboardInterrupt:
        print("\n\n监控已停止")
    except Exception as e:
        print(f"\n错误: {e}")
        import traceback
        traceback.print_exc()