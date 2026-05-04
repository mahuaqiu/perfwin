# perfwin 构建与分发指南

## 构建要求

- Python 3.8+（推荐 3.12）
- Rust 工具链（rustup）
- maturin：`pip install maturin`

## 构建 Wheel 包

### 一键构建

```bash
python build_wheel.py
```

构建产物位于 `target/wheels/perfwin-0.1.0-cp312-cp312-win_amd64.whl`（约 11 MB）

### 构建内容

Wheel 包包含以下文件：

| 文件 | 说明 |
|------|------|
| `perfwin/__init__.py` | Python 模块入口 |
| `perfwin/perfwin.cp312-win_amd64.pyd` | Rust 扩展模块 |
| `perfwin/HWiNFO64/HWiNFO64.EXE` | 系统温度/功耗采集工具 |
| `perfwin/HWiNFO64/HWiNFO64.INI` | HWiNFO 配置文件 |

### 手动构建步骤

如需手动构建：

```bash
# 1. maturin 构建 wheel
maturin build --release

# 2. 添加 Python 源码和 HWiNFO64（build_wheel.py 自动完成）
python build_wheel.py
```

## 分发方式

### 方式一：直接分发 Wheel 文件

将 wheel 文件复制到目标机器，执行：

```bash
pip install perfwin-0.1.0-cp312-cp312-win_amd64.whl
```

### 方式二：多版本构建

针对不同 Python 版本构建：

```bash
maturin build --release --interpreter python3.10
maturin build --release --interpreter python3.11
maturin build --release --interpreter python3.12
```

然后分别运行 `build_wheel.py` 处理每个版本。

## 安装验证

```python
import perfwin

print(perfwin.__version__)                # 0.1.0
print(perfwin._find_hwinfo_path())        # 自动找到 HWiNFO64.EXE
print(len(perfwin.list_processes()))      # 返回进程数量
```

## 平台限制

| 限制项 | 说明 |
|--------|------|
| 操作系统 | 仅 Windows x64 |
| Python 版本 | wheel 文件版本特定（cp312 = Python 3.12） |
| HWiNFO | 已打包，无需用户单独安装 |

## 常见问题

### Q: 如何在其他 Python 版本上使用？

重新构建对应版本的 wheel：
```bash
maturin build --release --interpreter python3.10
python build_wheel.py
```

### Q: 安装后 HWiNFO64 找不到？

模块会自动搜索以下路径：
1. `site-packages/perfwin/HWiNFO64/HWiNFO64.EXE`（wheel 打包）
2. 项目目录（开发模式）

如果找不到，可手动指定：
```python
monitor = perfwin.Monitor(hwinfo_path="C:/path/to/HWiNFO64.EXE")
```

### Q: 如何发布到 PyPI？

```bash
pip install twine
twine upload target/wheels/*.whl
```

需要 PyPI 账号和 API token。