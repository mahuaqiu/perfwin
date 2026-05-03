#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
perfdog 基本使用示例

展示如何使用 perfdog 进行系统性能监控（包含 HWiNFO 数据采集）。

注意：此示例需要在 Windows 上运行，且 HWiNFO64 需启用共享内存功能。
"""

import sys

# 检查平台
if sys.platform != "win32":
    print("错误：perfdog 只能在 Windows 上运行")
    print(f"当前平台: {sys.platform}")
    sys.exit(1)

import perfdog
import time


def example_system_info():
    """系统信息采集示例（包含 HWiNFO 温度/功耗数据）"""
    print("\n=== 系统信息采集示例 ===")
    print("注意: HWiNFO64 需启用共享内存功能")
    print("      HWiNFO Settings -> General -> Shared Memory Support")

    with perfdog.Monitor(
        interval=1.0,
        duration=10,
        enable_hwinfo=True,
        enable_sysinfo=True,
        enable_pdh=True,
        top_n_cpu=10,
    ) as monitor:
        time.sleep(11)

        result = monitor.get_result()
        print(f"\n采集到 {len(result)} 个样本")

        for i, sample in enumerate(result.samples):
            print(f"\n--- 样本 {i+1} ---")
            print(f"时间: {sample.timestamp}")

            # 系统信息
            if sample.system:
                s = sample.system
                print(f"系统总 CPU 使用率: {s.cpu.percent:.1f}%")
                print(f"系统总 GPU 使用率: {s.gpu.percent:.1f}%")

                # 温度（保留1位小数）
                if s.cpu.temperature:
                    print(f"CPU 温度: {s.cpu.temperature:.1f} C")
                if s.gpu.temperature:
                    print(f"GPU 温度: {s.gpu.temperature:.1f} C")

                # 功耗（保留2位小数）
                if s.cpu.power:
                    print(f"CPU 功耗: {s.cpu.power:.2f} W")
                if s.gpu.power:
                    print(f"GPU 功耗: {s.gpu.power:.2f} W")
                if s.system_power:
                    print(f"系统功耗: {s.system_power:.2f} W")

                print(f"内存使用率: {s.memory.percent:.1f}% ({s.memory.used_mb:.0f}/{s.memory.total_mb:.0f} MB)")

                # 网络（转换为 KB/s）
                if s.network.upload_speed or s.network.download_speed:
                    print(f"上传速度: {s.network.upload_speed/1024:.2f} KB/s")
                    print(f"下载速度: {s.network.download_speed/1024:.2f} KB/s")

                # 电池（笔记本）
                if s.battery.charge_level:
                    print(f"电池电量: {s.battery.charge_level:.1f}%")

            # Top N CPU 进程
            if sample.top_n_cpu:
                print("\n进程 CPU 占用前 10:")
                for proc in sample.top_n_cpu[:10]:
                    print(f"  {proc.name}({proc.pid}): {proc.cpu_percent:.1f}%")


def main():
    """运行示例"""
    print("perfdog 使用示例")
    print("=" * 50)

    try:
        example_system_info()
    except Exception as e:
        print(f"系统信息示例失败: {e}")
        import traceback
        traceback.print_exc()


if __name__ == "__main__":
    main()