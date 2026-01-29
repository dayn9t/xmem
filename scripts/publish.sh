#!/bin/bash
# xmem 发布脚本

set -e

echo "==================================="
echo "xmem v0.1.0 发布脚本"
echo "==================================="
echo ""

# 检查当前目录
cd "$(dirname "$0")/.."
PROJECT_ROOT=$(pwd)

echo "项目根目录: $PROJECT_ROOT"
echo ""

# ============================================
# 1. crates.io 发布
# ============================================
echo "📦 步骤 1: 发布到 crates.io"
echo "-----------------------------------"
echo ""
echo "请确保你已经:"
echo "  1. 运行 'cargo login' 并输入 API token"
echo "  2. 在 https://crates.io/settings/profile 验证了邮箱"
echo ""
read -p "是否继续发布到 crates.io? (y/n) " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    cd "$PROJECT_ROOT/crates/xmem-core"
    echo "正在发布 xmem-core..."
    cargo publish
    echo "✅ xmem-core 发布成功!"
else
    echo "⏭️  跳过 crates.io 发布"
fi

echo ""

# ============================================
# 2. PyPI 发布
# ============================================
echo "🐍 步骤 2: 发布到 PyPI"
echo "-----------------------------------"
echo ""

# 检查 maturin
if ! command -v maturin &> /dev/null; then
    echo "❌ maturin 未安装"
    echo ""
    echo "请选择安装方式:"
    echo "  1. pipx install maturin"
    echo "  2. cargo install maturin"
    echo "  3. pip install --user maturin"
    echo ""
    read -p "是否现在安装 maturin? (1/2/3/n) " -n 1 -r
    echo ""

    case $REPLY in
        1)
            pipx install maturin
            ;;
        2)
            cargo install maturin
            ;;
        3)
            pip install --user maturin
            ;;
        *)
            echo "⏭️  跳过 PyPI 发布"
            exit 0
            ;;
    esac
fi

echo "检测到 maturin: $(which maturin)"
echo ""

cd "$PROJECT_ROOT/crates/xmem-python"

echo "构建 Python 包..."
maturin build --release

echo ""
echo "测试本地安装..."
pip install --force-reinstall target/wheels/*.whl
python3 -c "import xmem; print('✅ xmem 导入成功')"

echo ""
read -p "是否发布到 PyPI? (y/n) " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "正在发布到 PyPI..."
    echo ""
    echo "请确保你已经:"
    echo "  1. 在 https://pypi.org/manage/account/token/ 创建了 API token"
    echo "  2. 配置了 ~/.pypirc 或准备手动输入 token"
    echo ""
    maturin publish
    echo "✅ PyPI 发布成功!"
else
    echo "⏭️  跳过 PyPI 发布"
fi

echo ""

# ============================================
# 3. GitHub Release
# ============================================
echo "🚀 步骤 3: 创建 GitHub Release"
echo "-----------------------------------"
echo ""
echo "请访问以下链接创建 GitHub Release:"
echo ""
echo "  https://github.com/dayn9t/xmem/releases/new?tag=v0.1.0"
echo ""
echo "发布说明已准备在:"
echo "  $PROJECT_ROOT/docs/RELEASE_v0.1.0.md"
echo ""

# ============================================
# 完成
# ============================================
echo ""
echo "==================================="
echo "✅ 发布流程完成!"
echo "==================================="
echo ""
echo "验证发布:"
echo "  - crates.io: cargo search xmem-core"
echo "  - PyPI: pip install xmem"
echo "  - GitHub: https://github.com/dayn9t/xmem/releases"
echo ""
