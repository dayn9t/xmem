#!/bin/bash
# Python 包发布脚本

set -e

cd "$(dirname "$0")/../crates/xmem-python"

echo "==> 检查 maturin..."
if ! command -v maturin &> /dev/null; then
    echo "安装 maturin..."
    pip install --user maturin[patchelf]
fi

echo "==> 构建 Python 包..."
maturin build --release

echo "==> 测试安装..."
pip install --force-reinstall target/wheels/*.whl

echo "==> 运行测试..."
python -c "import xmem; print('xmem version:', xmem.__version__)"

echo ""
echo "构建成功! Wheel 文件位于: target/wheels/"
echo ""
echo "发布到 PyPI:"
echo "  maturin publish"
