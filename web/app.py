"""Flask backend for ArxivCat Web version."""
import os
import sys
from pathlib import Path
from flask import Flask, render_template, request, jsonify
from flask_cors import CORS

# Add parent directory to path to import arxivcat
sys.path.insert(0, str(Path(__file__).parent.parent))

from arxivcat.core import (
    extract_arxiv_id,
    download_source,
    extract_body_from_dir,
    _strip_latex_comments,
)

try:
    from google import genai
    GEMINI_AVAILABLE = True
except ImportError:
    GEMINI_AVAILABLE = False

app = Flask(__name__)
CORS(app)

# Cache directories
BASE_DIR = Path(os.environ.get("APPDATA", Path.home())) / "ArxivCat"
DOWNLOADS_DIR = BASE_DIR / "downloads"
OUTPUTS_DIR = BASE_DIR / "outputs"
DOWNLOADS_DIR.mkdir(parents=True, exist_ok=True)
OUTPUTS_DIR.mkdir(parents=True, exist_ok=True)

# Gemini client
gemini_client = None
if GEMINI_AVAILABLE:
    api_key = os.environ.get("GEMINI_API_KEY")
    if api_key:
        gemini_client = genai.Client(api_key=api_key)


@app.route('/')
def index():
    """Serve the main page."""
    return render_template('index.html')


@app.route('/api/extract', methods=['POST'])
def extract():
    """Extract paper body and appendix from arXiv ID."""
    data = request.json
    url = data.get('url', '').strip()
    
    if not url:
        return jsonify({'error': '请输入 arXiv ID 或 URL'}), 400
    
    arxiv_id = extract_arxiv_id(url)
    if not arxiv_id:
        return jsonify({'error': '无法识别 arXiv ID'}), 400
    
    logs = []
    
    def log_fn(msg):
        logs.append(msg)
    
    try:
        # Download and extract
        paper_dir, folder_name = download_source(arxiv_id, DOWNLOADS_DIR, log=log_fn)
        
        if not paper_dir:
            return jsonify({
                'error': '下载失败',
                'logs': logs
            }), 500
        
        # Extract body and appendix
        result = extract_body_from_dir(paper_dir, OUTPUTS_DIR, folder_name, log=log_fn)
        
        if not result:
            return jsonify({
                'error': '提取失败',
                'logs': logs
            }), 500
        
        # Read extracted files
        output_dir = OUTPUTS_DIR / folder_name
        body_path = output_dir / "body.tex"
        appendix_path = output_dir / "appendix.tex"
        
        body = body_path.read_text(encoding='utf-8') if body_path.exists() else ""
        appendix = appendix_path.read_text(encoding='utf-8') if appendix_path.exists() else ""
        
        return jsonify({
            'success': True,
            'arxiv_id': arxiv_id,
            'body': body,
            'appendix': appendix,
            'has_appendix': bool(appendix),
            'logs': logs,
            'output_dir': str(output_dir)
        })
    
    except Exception as e:
        return jsonify({
            'error': f'处理出错: {str(e)}',
            'logs': logs
        }), 500


@app.route('/api/strip-comments', methods=['POST'])
def strip_comments():
    """Strip LaTeX comments from text."""
    data = request.json
    content = data.get('content', '')
    
    if not content:
        return jsonify({'error': '内容为空'}), 400
    
    try:
        import re
        stripped = _strip_latex_comments(content)
        stripped = re.sub(r'\n{3,}', '\n\n', stripped).strip()
        
        return jsonify({
            'success': True,
            'content': stripped
        })
    except Exception as e:
        return jsonify({'error': f'处理出错: {str(e)}'}), 500


@app.route('/api/chat', methods=['POST'])
def chat():
    """Chat with Gemini about the paper."""
    if not gemini_client:
        return jsonify({
            'error': 'Gemini API 未配置或不可用'
        }), 503
    
    data = request.json
    message = data.get('message', '').strip()
    context = data.get('context', '')
    history = data.get('history', [])
    
    if not message:
        return jsonify({'error': '消息为空'}), 400
    
    try:
        # Build prompt with context
        system_prompt = "你是一个学术论文阅读助手。用户正在阅读一篇论文，你需要帮助他们理解论文内容。"
        
        if context:
            system_prompt += f"\n\n当前论文内容（部分）：\n{context[:8000]}"
        
        # Build messages
        messages = [{"role": "user", "parts": [{"text": system_prompt}]}]
        
        # Add history
        for h in history[-10:]:  # Keep last 10 messages
            role = "user" if h.get('role') == 'user' else "model"
            messages.append({
                "role": role,
                "parts": [{"text": h.get('content', '')}]
            })
        
        # Add current message
        messages.append({
            "role": "user",
            "parts": [{"text": message}]
        })
        
        # Call Gemini
        response = gemini_client.models.generate_content(
            model='gemini-2.0-flash-lite',
            contents=messages
        )
        
        reply = response.text if hasattr(response, 'text') else str(response)
        
        return jsonify({
            'success': True,
            'reply': reply
        })
    
    except Exception as e:
        return jsonify({
            'error': f'Gemini 调用失败: {str(e)}'
        }), 500


if __name__ == '__main__':
    print("=" * 60)
    print("ArxivCat Web 版本启动中...")
    print("=" * 60)
    print(f"本地访问: http://localhost:5000")
    print(f"局域网访问: http://<你的IP>:5000")
    print()
    print("提示:")
    print("  - 手机和电脑连接同一 WiFi")
    print("  - 手机浏览器访问后点击'添加到主屏幕'")
    print("  - 就可以像 app 一样使用了")
    print("=" * 60)
    print()
    
    app.run(host='0.0.0.0', port=5000, debug=True)
