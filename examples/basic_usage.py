#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
perfdog 使用示例 - 监控 quark 进程

监控所有 quark 进程的系统性能数据，每5秒输出一次。
"""

import sys

# 检查平台
if sys.platform != "win32":
    print("错误：perfdog 只能在 Windows 上运行")
    print(f"当前平台: {sys.platform}")
    sys.exit(1)

import perfdog
import time


def monitor_quark():
    """监控 quark 进程"""
    print("=== quark 进程监控 ===")
    print("每 5 秒输出一次数据")
    print("=" * 50)

    # 使用正则表达式匹配所有 quark 进程
    process_filter = perfdog.ProcessFilter(name_regex=r"quark.*\.exe")

    with perfdog.Monitor(
        interval=5.0,  # 采集间隔 5 秒
        duration=None,  # 无限时长
        enable_hwinfo=True,
        enable_sysinfo=True,
        enable_pdh=True,
        process_filter=process_filter,
    ) as monitor:
        while True:
            time.sleep(5)

            result = monitor.get_result()
            print(f"\n采集到 {len(result)} 个样本")

            for sample in result.samples:
                print(f"\n时间: {sample.timestamp}")

                # 系统信息
                if sample.system:
                    s = sample.system
                    print(f"系统总 CPU 使用率: {s.cpu.percent:.1f}%")
                    print(f"系统总 GPU 使用率: {s.gpu.percent:.1f}%")

                    if s.cpu.temperature:
                        print(f"CPU 温度: {s.cpu.temperature:.1f} C")
                    if s.cpu.power:
                        print(f"CPU 功耗: {s.cpu.power:.2f} W")
                    if s.gpu.temperature:
                        print(f"GPU 温度: {s.gpu.temperature:.1f} C")
                    if s.gpu.power:
                        print(f"GPU 功耗: {s.gpu.power:.2f} W")

                    print(f"内存使用率: {s.memory.percent:.1f}%")

                    if s.network.upload_speed or s.network.download_speed:
                        print(f"上传速度: {s.network.upload_speed/1024:.2f} KB/s")
                        print(f"下载速度: {s.network.download_speed/1024:.2f} KB/s")

                # quark 进程信息
                if sample.processes:
                    print("\nquark 进程:")
                    for proc in sample.processes:
                        print(f"  {proc.name} (PID {proc.pid})")
                        print(f"    CPU: {proc.cpu_percent:.1f}%")
                        print(f"    内存: {proc.working_set_mb:.1f} MB")
                        print(f"    GPU: {proc.gpu_percent:.1f}%")
                        print(f"    GPU显存: {proc.gpu_memory_mb:.1f} MB")


if __name__ == "__main__":
    try:
        monitor_quark()
    except KeyboardInterrupt:
        print("\n监控已停止")
    except Exception as e:
        print(f"错误: {e}")
        import traceback
        traceback.print_exc()