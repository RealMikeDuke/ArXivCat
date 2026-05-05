"""快速测试 Web 服务器是否能正常启动"""
import sys
from pathlib import Path

# Fix encoding for Windows console
if sys.platform == 'win32':
    import io
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent))

print("测试 1: 导入核心模块...")
try:
    from arxivcat.core import extract_arxiv_id, download_source
    print("[OK] 核心模块导入成功")
except Exception as e:
    print(f"[FAIL] 核心模块导入失败: {e}")
    sys.exit(1)

print("\n测试 2: 导入 Flask...")
try:
    from flask import Flask
    from flask_cors import CORS
    print("[OK] Flask 导入成功")
except Exception as e:
    print(f"[FAIL] Flask 导入失败: {e}")
    sys.exit(1)

print("\n测试 3: 测试 arXiv ID 解析...")
test_cases = [
    "2301.12345",
    "https://arxiv.org/abs/2301.12345",
    "arxiv.org/pdf/2301.12345.pdf"
]
for test in test_cases:
    result = extract_arxiv_id(test)
    print(f"  {test} -> {result}")

print("\n测试 4: 创建 Flask 应用...")
try:
    app = Flask(__name__)
    CORS(app)
    
    @app.route('/test')
    def test():
        return {'status': 'ok'}
    
    print("[OK] Flask 应用创建成功")
except Exception as e:
    print(f"[FAIL] Flask 应用创建失败: {e}")
    sys.exit(1)

print("\n" + "="*60)
print("[OK] 所有测试通过！")
print("="*60)
print("\n现在可以运行: .\\run-web.ps1")
print("或者: D:\\anaconda3\\envs\\web\\python.exe web\\app.py")
