// ArxivCat Web App - Main JavaScript

// State
let currentData = {
    body: '',
    appendix: '',
    logs: [],
    arxivId: '',
    hasAppendix: false
};

let currentView = 'body';
let chatHistory = [];

// DOM Elements
const arxivInput = document.getElementById('arxiv-input');
const extractBtn = document.getElementById('extract-btn');
const btnText = document.getElementById('btn-text');
const btnSpinner = document.getElementById('btn-spinner');
const statusBar = document.getElementById('status-bar');
const statusText = document.getElementById('status-text');
const viewControls = document.getElementById('view-controls');
const previewSection = document.getElementById('preview-section');
const previewTitle = document.getElementById('preview-title');
const previewStats = document.getElementById('preview-stats');
const previewText = document.getElementById('preview-text');
const logPanel = document.getElementById('log-panel');
const logContent = document.getElementById('log-content');
const chatMessages = document.getElementById('chat-messages');
const chatInput = document.getElementById('chat-input');
const chatSend = document.getElementById('chat-send');
const toast = document.getElementById('toast');

// Event Listeners
extractBtn.addEventListener('click', handleExtract);
arxivInput.addEventListener('keypress', (e) => {
    if (e.key === 'Enter') handleExtract();
});

document.querySelectorAll('.tab-btn').forEach(btn => {
    btn.addEventListener('click', () => switchView(btn.dataset.view));
});

document.getElementById('copy-btn').addEventListener('click', handleCopy);
document.getElementById('strip-btn').addEventListener('click', handleStripComments);
document.getElementById('log-toggle').addEventListener('click', toggleLog);
document.getElementById('log-close').addEventListener('click', () => logPanel.classList.add('hidden'));
document.getElementById('chat-reset').addEventListener('click', resetChat);
chatSend.addEventListener('click', handleChatSend);
chatInput.addEventListener('keypress', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleChatSend();
    }
});

// Extract Paper
async function handleExtract() {
    const url = arxivInput.value.trim();
    if (!url) {
        showToast('请输入 arXiv ID 或 URL', 'error');
        return;
    }

    setLoading(true);
    showStatus('处理中...', 'info');
    
    // Show log panel immediately when extraction starts
    logPanel.classList.remove('hidden');
    logContent.textContent = '';
    previewText.value = '';
    
    try {
        const response = await fetch('/api/extract', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ url })
        });

        const data = await response.json();

        if (!response.ok) {
            throw new Error(data.error || '提取失败');
        }

        currentData = {
            body: data.body || '',
            appendix: data.appendix || '',
            logs: data.logs || [],
            arxivId: data.arxiv_id || '',
            hasAppendix: data.has_appendix || false
        };

        // Update logs
        logContent.textContent = currentData.logs.join('\n');

        // Show controls and preview
        viewControls.classList.remove('hidden');
        previewSection.classList.remove('hidden');

        // Enable/disable appendix tab
        const appendixTab = document.querySelector('[data-view="appendix"]');
        if (currentData.hasAppendix) {
            appendixTab.disabled = false;
            appendixTab.style.opacity = '1';
        } else {
            appendixTab.disabled = true;
            appendixTab.style.opacity = '0.5';
        }

        // Switch to body view
        switchView('body');
        showStatus('提取完成', 'success');
        showToast('提取成功！', 'success');

    } catch (error) {
        showStatus('提取失败', 'error');
        showToast(error.message, 'error');
        console.error('Extract error:', error);
    } finally {
        setLoading(false);
    }
}

// Switch View (Body/Appendix)
function switchView(view) {
    currentView = view;
    
    // Update tabs
    document.querySelectorAll('.tab-btn').forEach(btn => {
        btn.classList.toggle('active', btn.dataset.view === view);
    });

    // Update preview
    const content = view === 'body' ? currentData.body : currentData.appendix;
    previewText.value = content;
    previewTitle.textContent = `${view}.tex`;
    updateStats(content);
}

// Update Stats
function updateStats(content) {
    const lines = content.split('\n').length;
    const chars = content.length;
    previewStats.textContent = `${lines} 行 · ${chars} 字符`;
}

// Copy to Clipboard
async function handleCopy() {
    try {
        await navigator.clipboard.writeText(previewText.value);
        showToast('已复制到剪贴板', 'success');
    } catch (error) {
        showToast('复制失败', 'error');
    }
}

// Strip Comments
async function handleStripComments() {
    const content = previewText.value;
    if (!content) return;

    try {
        const response = await fetch('/api/strip-comments', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ content })
        });

        const data = await response.json();

        if (!response.ok) {
            throw new Error(data.error || '处理失败');
        }

        previewText.value = data.content;
        updateStats(data.content);
        showToast('注释已去除', 'success');

    } catch (error) {
        showToast(error.message, 'error');
    }
}

// Toggle Log Panel
function toggleLog() {
    logPanel.classList.toggle('hidden');
}

// Chat Functions
async function handleChatSend() {
    const message = chatInput.value.trim();
    if (!message) return;

    // Add user message
    addChatMessage(message, 'user');
    chatInput.value = '';
    chatInput.style.height = 'auto';

    // Get current context
    const context = previewText.value;

    try {
        // Show typing indicator
        const typingId = addChatMessage('正在思考...', 'assistant', true);

        const response = await fetch('/api/chat', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                message,
                context,
                history: chatHistory
            })
        });

        const data = await response.json();

        // Remove typing indicator
        document.getElementById(typingId)?.remove();

        if (!response.ok) {
            throw new Error(data.error || 'Chat 失败');
        }

        // Add assistant reply
        addChatMessage(data.reply, 'assistant');

        // Update history
        chatHistory.push(
            { role: 'user', content: message },
            { role: 'assistant', content: data.reply }
        );

    } catch (error) {
        showToast(error.message, 'error');
        document.querySelector('.typing-indicator')?.remove();
    }
}

function addChatMessage(content, role, isTyping = false) {
    const messageDiv = document.createElement('div');
    const id = `msg-${Date.now()}`;
    messageDiv.id = id;
    messageDiv.className = `chat-message ${role} ${isTyping ? 'typing-indicator' : ''}`;
    
    const contentDiv = document.createElement('div');
    contentDiv.className = 'message-content';
    contentDiv.textContent = content;
    
    messageDiv.appendChild(contentDiv);
    chatMessages.appendChild(messageDiv);
    chatMessages.scrollTop = chatMessages.scrollHeight;
    
    return id;
}

function resetChat() {
    chatHistory = [];
    chatMessages.innerHTML = `
        <div class="chat-message assistant">
            <div class="message-content">
                你好！我可以帮你理解论文内容。提取论文后，你可以问我任何问题。
            </div>
        </div>
    `;
    showToast('对话已重置', 'info');
}

// UI Helpers
function setLoading(loading) {
    extractBtn.disabled = loading;
    arxivInput.disabled = loading;
    
    if (loading) {
        btnText.classList.add('hidden');
        btnSpinner.classList.remove('hidden');
    } else {
        btnText.classList.remove('hidden');
        btnSpinner.classList.add('hidden');
    }
}

function showStatus(text, type) {
    statusText.textContent = text;
    statusBar.className = `status-bar status-${type}`;
    statusBar.classList.remove('hidden');
}

function showToast(message, type = 'info') {
    toast.textContent = message;
    toast.className = `toast toast-${type}`;
    toast.classList.remove('hidden');
    
    setTimeout(() => {
        toast.classList.add('hidden');
    }, 3000);
}

// Auto-resize chat input
chatInput.addEventListener('input', function() {
    this.style.height = 'auto';
    this.style.height = Math.min(this.scrollHeight, 120) + 'px';
});

// PWA Install Prompt
let deferredPrompt;

window.addEventListener('beforeinstallprompt', (e) => {
    e.preventDefault();
    deferredPrompt = e;
    
    // Show install hint
    showToast('💡 可以将此应用添加到主屏幕', 'info');
});

window.addEventListener('appinstalled', () => {
    showToast('✅ 应用已安装到主屏幕', 'success');
    deferredPrompt = null;
});
