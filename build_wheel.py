#!/usr/bin/env python
"""
perfwin 构建脚本 - 构建 wheel 包并打包 Python 源码和 HWiNFO64
"""

import zipfile
import subprocess
import sys
from pathlib import Path


def build_wheel():
    """使用 maturin 构建 wheel"""
    print("[1/3] 构建 wheel 包...")
    result = subprocess.run(["maturin", "build", "--release"], text=True)
    if result.returncode != 0:
        sys.exit(1)


def find_wheel():
    """查找生成的 wheel 文件"""
    wheels_dir = Path("target/wheels")
    wheels = list(wheels_dir.glob("perfwin-*.whl"))
    if not wheels:
        print("未找到 wheel 文件")
        sys.exit(1)
    return max(wheels, key=lambda p: p.stat().st_mtime)


def add_python_files(wheel_path):
    """添加 Python 源码和 HWiNFO64 到 wheel 包"""
    python_dir = Path("python/perfwin")

    print(f"[2/3] 添加 Python 文件和 HWiNFO64...")

    with zipfile.ZipFile(wheel_path, 'a') as whl:
        # 添加 Python 源码文件
        for f in python_dir.iterdir():
            if f.is_file() and f.suffix == '.py':
                arcname = f"perfwin/{f.name}"
                whl.write(str(f), arcname)
                print(f"      添加: {f.name}")

        # 添加 HWiNFO64 目录
        hwinfo_dir = python_dir / "HWiNFO64"
        if hwinfo_dir.exists():
            for f in hwinfo_dir.iterdir():
                if f.is_file():
                    arcname = f"perfwin/HWiNFO64/{f.name}"
                    whl.write(str(f), arcname)
                    print(f"      添加: HWiNFO64/{f.name}")

        # 重新生成 RECORD 文件
        records = []
        for info in whl.infolist():
            if info.filename != 'perfwin-0.1.0.dist-info/RECORD':
                records.append(f"{info.filename},,")
        records.append("perfwin-0.1.0.dist-info/RECORD,,")
        whl.writestr("perfwin-0.1.0.dist-info/RECORD", "\n".join(records))


def main():
    build_wheel()
    wheel_path = find_wheel()
    print(f"[3/3] Wheel 文件: {wheel_path}")
    add_python_files(wheel_path)

    size_mb = wheel_path.stat().st_size / (1024 * 1024)
    print(f"\n=== 构建完成 ===")
    print(f"文件: {wheel_path}")
    print(f"大小: {size_mb:.2f} MB")

    # 显示包内容摘要
    print(f"\n包内容:")
    with zipfile.ZipFile(wheel_path, 'r') as whl:
        for info in whl.infolist():
            if 'perfwin/' in info.filename and not info.filename.endswith('/'):
                size_kb = info.file_size / 1024
                print(f"  {info.filename}: {size_kb:.1f} KB")


if __name__ == "__main__":
    main()