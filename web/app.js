// CloudVault Web Admin - Main Application

console.log('=== CloudVault Web Starting ===');

// Configurable API base URL - defaults to current host:8080, overridable via settings
const host = window.location.hostname;
let API_BASE = localStorage.getItem('cloudvault_api') || `http://${host}:8080`;

console.log('API_BASE:', API_BASE);

// Function to change API URL
function setApiBase(url) {
    API_BASE = url;
    localStorage.setItem('cloudvault_api', url);
    console.log('API Base changed to:', API_BASE);
    // Reload to apply
    window.location.reload();
}

// State
let token = localStorage.getItem('cloudvault_token');
let currentUser = null;
console.log('Initial token from storage:', token ? 'exists' : 'none');
let files = [];
let shares = [];
let users = [];
let selectedFile = null;
let isAdmin = false;
let currentFolderId = null;
let folderStack = [];
let fileSearchQuery = '';
let transferRecords = [];
const CHUNK_SIZE = 1024 * 1024;

// DOM Elements
const loginModal = document.getElementById('loginModal');
const registerModal = document.getElementById('registerModal');
const app = document.getElementById('app');
const userInfo = document.getElementById('userInfo');
const fileList = document.getElementById('fileList');
const folderBreadcrumb = document.getElementById('folderBreadcrumb');
const searchInput = document.querySelector('.search-field input');
const shareList = document.getElementById('shareList');
const transferList = document.getElementById('transferList');
const userList = document.getElementById('userList');
const contextMenu = document.getElementById('contextMenu');
const toast = document.getElementById('toast');
const videoPlayerView = document.getElementById('videoPlayerView');
const videoPlayer = document.getElementById('videoPlayer');
const videoPlayerTitle = document.getElementById('videoPlayerTitle');
const videoPlayerHint = document.getElementById('videoPlayerHint');

// Initialize
document.addEventListener('DOMContentLoaded', () => {
    if (token) {
        checkAuth();
    } else {
        showLogin();
    }
    setupEventListeners();
});

// Setup Event Listeners
function setupEventListeners() {
    // Auth forms
    document.getElementById('loginForm').addEventListener('submit', handleLogin);
    document.getElementById('registerForm').addEventListener('submit', handleRegister);
    document.getElementById('showRegister').addEventListener('click', (e) => {
        e.preventDefault();
        loginModal.style.display = 'none';
        registerModal.style.display = 'flex';
    });
    document.getElementById('showLogin').addEventListener('click', (e) => {
        e.preventDefault();
        registerModal.style.display = 'none';
        loginModal.style.display = 'flex';
    });
    document.getElementById('logoutBtn').addEventListener('click', handleLogout);
    document.getElementById('settingsBtn').addEventListener('click', showSettingsModal);
    document.getElementById('closePlayerBtn').addEventListener('click', closeVideoPlayer);

    // File operations
    document.getElementById('createFolderBtn').addEventListener('click', () => createFolder());
    document.getElementById('uploadBtn').addEventListener('click', () => document.getElementById('fileInput').click());
    document.getElementById('fileInput').addEventListener('change', handleFileUpload);
    const clearTransfersBtn = document.getElementById('clearTransfersBtn');
    if (clearTransfersBtn) {
        clearTransfersBtn.addEventListener('click', clearCompletedTransfers);
    }
    if (searchInput) {
        searchInput.addEventListener('input', () => {
            fileSearchQuery = searchInput.value.trim().toLowerCase();
            renderFiles();
        });
    }

    // Navigation
    document.querySelectorAll('.nav-item').forEach(item => {
        item.addEventListener('click', (e) => {
            e.preventDefault();
            switchView(item.dataset.view);
        });
    });

    // Context menu
    document.addEventListener('click', () => contextMenu.classList.add('hidden'));
    document.getElementById('ctxOpen').addEventListener('click', () => openFile(selectedFile));
    document.getElementById('ctxRename').addEventListener('click', () => promptRename(selectedFile));
    document.getElementById('ctxShare').addEventListener('click', () => showShareModal(selectedFile));
    document.getElementById('ctxMove').addEventListener('click', () => showMoveModal(selectedFile));
    document.getElementById('ctxDelete').addEventListener('click', () => deleteFile(selectedFile));

    // Move modal
    document.getElementById('moveToRootBtn').addEventListener('click', () => moveFile(selectedFile, null));
    document.getElementById('cancelMoveBtn').addEventListener('click', () => document.getElementById('moveModal').classList.add('hidden'));

    // Share modal
    document.getElementById('createShareBtn').addEventListener('click', createShare);
    document.getElementById('cancelShareBtn').addEventListener('click', () => document.getElementById('shareModal').classList.add('hidden'));
}

// Auth Functions
async function handleLogin(e) {
    e.preventDefault();
    const username = document.getElementById('loginUsername').value;
    const password = document.getElementById('loginPassword').value;

    try {
        console.log('Attempting login for:', username);
        const res = await fetch(`${API_BASE}/api/auth/login`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ username, password })
        });
        console.log('Response status:', res.status);
        const data = await res.json();
        console.log('Response data:', data);

        if (!res.ok) throw new Error(data.message || `登录失败 (${res.status})`);

        console.log('Login successful!');
        console.log('Token from response:', data.token ? `${data.token.substring(0, 30)}...` : 'EMPTY');
        console.log('User from response:', data.user);
        
        token = data.token;
        currentUser = data.user;
        
        console.log('Setting token to localStorage:', token.substring(0, 30) + '...');
        localStorage.setItem('cloudvault_token', token);
        localStorage.setItem('cloudvault_user', JSON.stringify(currentUser));
        
        console.log('Verifying localStorage:', localStorage.getItem('cloudvault_token') ? 'exists' : 'NOT SET');
        
        showApp();
    } catch (err) {
        console.error('Login error:', err);
        showToast(err.message, 'error');
    }
}

async function handleRegister(e) {
    e.preventDefault();
    const username = document.getElementById('regUsername').value;
    const password = document.getElementById('regPassword').value;

    try {
        const res = await fetch(`${API_BASE}/api/auth/register`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ username, password })
        });
        const data = await res.json();

        if (!res.ok) throw new Error(data.message || '注册失败');

        showToast('注册成功，请登录', 'success');
        registerModal.style.display = 'none';
        loginModal.style.display = 'flex';
    } catch (err) {
        showToast(err.message, 'error');
    }
}

function handleLogout() {
    token = null;
    currentUser = null;
    localStorage.removeItem('cloudvault_token');
    localStorage.removeItem('cloudvault_user');
    showLogin();
}

async function checkAuth() {
    console.log('checkAuth called, token:', token ? 'exists' : 'none');
    if (!token) {
        console.log('No token, showing login');
        showLogin();
        return;
    }
    try {
        const res = await fetch(`${API_BASE}/api/auth/me`, {
            headers: { 'Authorization': `Bearer ${token}` }
        });
        console.log('checkAuth response status:', res.status);
        if (res.ok) {
            currentUser = await res.json();
            console.log('Auth successful, user:', currentUser.username);
            showApp();
        } else {
            console.log('Auth failed, clearing token');
            localStorage.removeItem('cloudvault_token');
            token = null;
            showLogin();
        }
    } catch (err) {
        console.error('checkAuth error:', err);
        showLogin();
    }
}

// UI Functions
function showLogin() {
    loginModal.style.display = 'flex';
    registerModal.style.display = 'none';
    app.classList.add('hidden');
}

function showApp() {
    loginModal.style.display = 'none';
    registerModal.style.display = 'none';
    app.classList.remove('hidden');
    userInfo.textContent = `${currentUser.username} (${formatBytes(currentUser.storage_used)} / ${formatBytes(currentUser.storage_quota)})`;
    updateStorageMeter();
    isAdmin = !!currentUser.is_admin;
    document.body.classList.toggle('is-admin', isAdmin);
    // If a non-admin was left on the users view, fall back to files
    if (!isAdmin) {
        switchView('files');
    }
    loadFiles();
}

function switchView(view) {
    document.querySelectorAll('.nav-item').forEach(item => {
        item.classList.toggle('active', item.dataset.view === view);
    });
    document.querySelectorAll('.view').forEach(v => v.classList.add('hidden'));

    switch (view) {
        case 'files':
            document.getElementById('filesView').classList.remove('hidden');
            break;
        case 'shares':
            document.getElementById('sharesView').classList.remove('hidden');
            loadShares();
            break;
        case 'transfers':
            document.getElementById('transfersView').classList.remove('hidden');
            renderTransfers();
            break;
        case 'users':
            document.getElementById('usersView').classList.remove('hidden');
            loadUsers();
            break;
    }
}

function showToast(message, type = 'success') {
    toast.textContent = message;
    toast.className = `toast ${type}`;
    toast.classList.remove('hidden');
    setTimeout(() => toast.classList.add('hidden'), 4000);
}

function showSettingsModal() {
    const modal = document.getElementById('settingsModal');
    const input = document.getElementById('apiUrlInput');
    input.value = API_BASE;
    modal.classList.remove('hidden');
    
    document.getElementById('saveApiBtn').onclick = () => {
        const url = input.value.trim();
        if (url) {
            setApiBase(url);
        }
    };
    
    document.getElementById('cancelApiBtn').onclick = () => {
        modal.classList.add('hidden');
    };
}

function showContextMenu(e, file) {
    e.preventDefault();
    selectedFile = file;
    contextMenu.style.left = `${e.clientX}px`;
    contextMenu.style.top = `${e.clientY}px`;
    contextMenu.classList.remove('hidden');
}

// File Functions
async function loadFiles() {
    console.log('loadFiles called, token:', token ? 'exists' : 'none');
    try {
        const res = await fetch(`${API_BASE}/api/files`, { headers: getAuth() });
        console.log('loadFiles response status:', res.status);
        if (res.status === 401) {
            showToast('登录已过期，请重新登录', 'error');
            handleLogout();
            return;
        }
        files = await res.json();
        renderFiles();
    } catch (err) {
        console.error('loadFiles error:', err);
        showToast('加载文件失败', 'error');
    }
}

function renderFiles() {
    const visibleFiles = files.filter(file => {
        const inCurrentFolder = (file.parent_id || null) === currentFolderId;
        const matchesSearch = !fileSearchQuery || file.name.toLowerCase().includes(fileSearchQuery);
        return inCurrentFolder && matchesSearch;
    });
    renderBreadcrumb();

    if (visibleFiles.length === 0) {
        fileList.innerHTML = `
            <div class="empty-state">
                <div class="icon">□</div>
                <p>${fileSearchQuery ? '没有匹配的文件' : currentFolderId ? '此文件夹为空' : '暂无文件，上传一个文件或创建文件夹开始使用'}</p>
            </div>
        `;
        return;
    }

    fileList.innerHTML = `
        <div class="file-list-header">
            <span>名称</span>
            <span>大小</span>
            <span>操作</span>
        </div>
        ${visibleFiles.map(file => `
        <div class="file-item" data-id="${file.id}" data-folder="${file.is_folder}" title="${escapeAttribute(file.name)}">
            <span class="file-icon">${file.is_folder ? '▣' : getFileIcon(file.mime_type)}</span>
            <span class="file-name">${escapeHtml(file.name)}</span>
            <span class="file-meta">${file.is_folder ? '文件夹' : formatBytes(file.size)}</span>
            <div class="file-actions">
                ${!file.is_folder ? `<button class="action-btn" onclick="downloadFile('${file.id}')">下载</button>` : ''}
                <button class="action-btn danger" onclick="deleteFileById('${file.id}')">删除</button>
            </div>
        </div>
        `).join('')}
    `;

    // Add click handlers
    fileList.querySelectorAll('.file-item').forEach(item => {
        item.addEventListener('click', (e) => {
            if (e.target.closest('.action-btn')) return;
            const file = files.find(f => f.id === item.dataset.id);
            if (item.dataset.folder === 'true') {
                openFile(file);
            }
        });
        item.addEventListener('dblclick', (e) => {
            if (e.target.closest('.action-btn')) return;
            const file = files.find(f => f.id === item.dataset.id);
            if (!file) return;
            if (file.is_folder) {
                openFile(file);
            } else if (isVideoFile(file)) {
                openVideoPlayer(file);
            }
        });
        item.addEventListener('contextmenu', (e) => {
            const file = files.find(f => f.id === item.dataset.id);
            showContextMenu(e, file);
        });
    });
}

function renderBreadcrumb() {
    if (!folderBreadcrumb) return;

    const parts = [
        '<button class="breadcrumb-item" data-index="-1">根目录</button>',
        ...folderStack.map((folder, index) => (
            `<span class="breadcrumb-separator">/</span><button class="breadcrumb-item" data-index="${index}">${escapeHtml(folder.name)}</button>`
        )),
    ];

    folderBreadcrumb.innerHTML = parts.join('');
    folderBreadcrumb.querySelectorAll('.breadcrumb-item').forEach(item => {
        item.addEventListener('click', () => {
            const index = Number(item.dataset.index);
            if (index === -1) {
                currentFolderId = null;
                folderStack = [];
            } else {
                folderStack = folderStack.slice(0, index + 1);
                currentFolderId = folderStack[index].id;
            }
            renderFiles();
        });
    });
}

async function createFolder() {
    const name = prompt('请输入文件夹名称：');
    if (!name) return;

    try {
        const res = await fetch(`${API_BASE}/api/files`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', ...getAuth() },
            body: JSON.stringify({ name, parent_id: currentFolderId, is_folder: true })
        });
        if (!res.ok) {
            const data = await res.json();
            throw new Error(data.message || '创建失败');
        }
        showToast('文件夹创建成功', 'success');
        loadFiles();
    } catch (err) {
        showToast(err.message, 'error');
    }
}

async function handleFileUpload(e) {
    const file = e.target.files[0];
    if (!file) return;

    startUploadTransfer(file, currentFolderId);
    e.target.value = '';
}

function openFile(file) {
    if (file.is_folder) {
        currentFolderId = file.id;
        folderStack.push({ id: file.id, name: file.name });
        renderFiles();
    } else if (isVideoFile(file)) {
        openVideoPlayer(file);
    } else {
        downloadFile(file.id, file.name);
    }
}

function openVideoPlayer(file) {
    if (!token) {
        showToast('请先登录后播放视频', 'error');
        return;
    }

    videoPlayerTitle.textContent = file.name;
    videoPlayerHint.classList.add('hidden');
    videoPlayerHint.textContent = '';
    videoPlayer.src = `${API_BASE}/api/files/${file.id}/download?access_token=${encodeURIComponent(token)}`;
    videoPlayerView.classList.remove('hidden');
    videoPlayer.load();

    videoPlayer.play().catch(() => {
        videoPlayerHint.textContent = '浏览器已阻止自动播放，请点击播放按钮继续。';
        videoPlayerHint.classList.remove('hidden');
    });
}

function closeVideoPlayer() {
    videoPlayer.pause();
    videoPlayer.removeAttribute('src');
    videoPlayer.load();
    videoPlayerView.classList.add('hidden');
}

function promptRename(file) {
    const newName = prompt('请输入新名称：', file.name);
    if (!newName || newName === file.name) return;

    renameFile(file.id, newName);
}

async function renameFile(id, newName) {
    try {
        const res = await fetch(`${API_BASE}/api/files/${id}`, {
            method: 'PATCH',
            headers: { 'Content-Type': 'application/json', ...getAuth() },
            body: JSON.stringify({ name: newName })
        });
        if (!res.ok) {
            const data = await res.json();
            throw new Error(data.message || '重命名失败');
        }
        showToast('重命名成功', 'success');
        loadFiles();
    } catch (err) {
        showToast(err.message, 'error');
    }
}

async function deleteFileById(id) {
    if (!confirm('确定要删除吗？')) return;
    await deleteFile({ id });
}

async function deleteFile(file) {
    try {
        const res = await fetch(`${API_BASE}/api/files/${file.id}`, {
            method: 'DELETE',
            headers: getAuth()
        });
        if (!res.ok) {
            const data = await res.json();
            throw new Error(data.message || '删除失败');
        }
        showToast('删除成功', 'success');
        loadFiles();
    } catch (err) {
        showToast(err.message, 'error');
    }
}

async function downloadFile(id, filename) {
    const file = files.find(item => item.id === id);
    startDownloadTransfer({
        id,
        name: filename || file?.name || 'download',
        size: file?.size || 0,
    });
}

function getDownloadName(response) {
    const disposition = response.headers.get('Content-Disposition');
    if (!disposition) return null;

    const utf8Match = disposition.match(/filename\\*=UTF-8''([^;]+)/i);
    if (utf8Match) return decodeURIComponent(utf8Match[1]);

    const asciiMatch = disposition.match(/filename="?([^"]+)"?/i);
    return asciiMatch ? asciiMatch[1] : null;
}

function createTransfer(type, name, total) {
    const transfer = {
        id: `${type}-${Date.now()}-${Math.random().toString(16).slice(2)}`,
        type,
        name,
        total,
        transferred: 0,
        speed: 0,
        status: 'queued',
        message: '等待中',
        controller: null,
        chunks: [],
        file: null,
        fileId: null,
        parentId: null,
        uploadId: null,
        startedAt: Date.now(),
        updatedAt: Date.now(),
    };

    transferRecords.unshift(transfer);
    renderTransfers();
    return transfer;
}

async function startUploadTransfer(file, parentId) {
    const transfer = createTransfer('upload', file.name, file.size);
    transfer.file = file;
    transfer.parentId = parentId;

    switchView('transfers');

    try {
        transfer.status = 'running';
        transfer.message = '初始化上传';
        renderTransfers();

        const initRes = await fetch(`${API_BASE}/api/files/uploads/init`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', ...getAuth() },
            body: JSON.stringify({
                filename: file.name,
                parent_id: parentId,
                size: file.size,
            }),
        });

        if (!initRes.ok) {
            const data = await initRes.json();
            throw new Error(data.message || '上传初始化失败');
        }

        const initData = await initRes.json();
        transfer.uploadId = initData.upload_id;
        transfer.transferred = initData.uploaded_bytes || 0;
        await runUploadTransfer(transfer);
    } catch (err) {
        if (transfer.status !== 'paused') {
            markTransferError(transfer, err.message);
        }
    }
}

async function runUploadTransfer(transfer) {
    if (!transfer.file || !transfer.uploadId) return;

    transfer.status = 'running';
    transfer.message = '上传中';
    let lastBytes = transfer.transferred;
    let lastTime = performance.now();
    renderTransfers();

    while (transfer.transferred < transfer.total) {
        if (transfer.status !== 'running') return;

        const offset = transfer.transferred;
        const chunk = transfer.file.slice(offset, Math.min(offset + CHUNK_SIZE, transfer.total));
        transfer.controller = new AbortController();

        try {
            const res = await fetch(`${API_BASE}/api/files/uploads/${transfer.uploadId}/chunk?offset=${offset}`, {
                method: 'POST',
                headers: getAuth(),
                body: chunk,
                signal: transfer.controller.signal,
            });

            if (!res.ok) {
                const data = await res.json();
                throw new Error(data.message || '上传分片失败');
            }

            const data = await res.json();
            transfer.transferred = data.uploaded_bytes;
            updateTransferSpeed(transfer, lastBytes, lastTime);
            lastBytes = transfer.transferred;
            lastTime = performance.now();
            renderTransfers();
        } catch (err) {
            if (transfer.status === 'paused' || err.name === 'AbortError') return;
            throw err;
        }
    }

    transfer.message = '合并文件';
    renderTransfers();

    const completeRes = await fetch(`${API_BASE}/api/files/uploads/${transfer.uploadId}/complete`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...getAuth() },
        body: JSON.stringify({
            filename: transfer.file.name,
            parent_id: transfer.parentId,
            size: transfer.file.size,
        }),
    });

    if (!completeRes.ok) {
        const data = await completeRes.json();
        throw new Error(data.message || '完成上传失败');
    }

    transfer.status = 'done';
    transfer.message = '上传完成';
    transfer.speed = 0;
    renderTransfers();
    loadFiles();
}

async function startDownloadTransfer(file) {
    const transfer = createTransfer('download', file.name, file.size);
    transfer.fileId = file.id;
    transfer.chunks = [];

    switchView('transfers');
    await runDownloadTransfer(transfer);
}

async function runDownloadTransfer(transfer) {
    transfer.status = 'running';
    transfer.message = '下载中';
    let lastBytes = transfer.transferred;
    let lastTime = performance.now();
    renderTransfers();

    try {
        while (transfer.transferred < transfer.total) {
            if (transfer.status !== 'running') return;

            const start = transfer.transferred;
            const end = Math.min(start + CHUNK_SIZE - 1, transfer.total - 1);
            transfer.controller = new AbortController();

            const res = await fetch(`${API_BASE}/api/files/${transfer.fileId}/download`, {
                headers: {
                    ...getAuth(),
                    Range: `bytes=${start}-${end}`,
                },
                signal: transfer.controller.signal,
            });

            if (!(res.ok || res.status === 206)) {
                throw new Error('下载失败');
            }

            const buffer = await res.arrayBuffer();
            transfer.chunks.push(buffer);
            transfer.transferred += buffer.byteLength;
            updateTransferSpeed(transfer, lastBytes, lastTime);
            lastBytes = transfer.transferred;
            lastTime = performance.now();
            renderTransfers();
        }

        const blob = new Blob(transfer.chunks);
        saveBlob(blob, transfer.name);
        transfer.status = 'done';
        transfer.message = '下载完成';
        transfer.speed = 0;
        renderTransfers();
    } catch (err) {
        if (transfer.status === 'paused' || err.name === 'AbortError') return;
        markTransferError(transfer, err.message);
    }
}

function pauseTransfer(id) {
    const transfer = transferRecords.find(item => item.id === id);
    if (!transfer || transfer.status !== 'running') return;

    transfer.status = 'paused';
    transfer.message = '已暂停';
    transfer.speed = 0;
    if (transfer.controller) transfer.controller.abort();
    renderTransfers();
}

function resumeTransfer(id) {
    const transfer = transferRecords.find(item => item.id === id);
    if (!transfer || transfer.status !== 'paused') return;

    if (transfer.type === 'upload') {
        runUploadTransfer(transfer).catch(err => markTransferError(transfer, err.message));
    } else {
        runDownloadTransfer(transfer);
    }
}

function cancelTransfer(id) {
    const transfer = transferRecords.find(item => item.id === id);
    if (!transfer) return;

    transfer.status = 'cancelled';
    transfer.message = '已取消';
    transfer.speed = 0;
    if (transfer.controller) transfer.controller.abort();
    renderTransfers();
}

function clearCompletedTransfers() {
    transferRecords = transferRecords.filter(item => !['done', 'error', 'cancelled'].includes(item.status));
    renderTransfers();
}

function updateTransferSpeed(transfer, lastBytes, lastTime) {
    const now = performance.now();
    const seconds = Math.max((now - lastTime) / 1000, 0.001);
    transfer.speed = Math.max(0, (transfer.transferred - lastBytes) / seconds);
    transfer.updatedAt = Date.now();
}

function markTransferError(transfer, message) {
    transfer.status = 'error';
    transfer.message = message || '传输失败';
    transfer.speed = 0;
    renderTransfers();
}

function renderTransfers() {
    if (!transferList) return;

    if (transferRecords.length === 0) {
        transferList.innerHTML = `
            <div class="empty-state">
                <div class="icon">⇅</div>
                <p>暂无传输记录</p>
            </div>
        `;
        return;
    }

    transferList.innerHTML = transferRecords.map(transfer => {
        const percent = transfer.total > 0 ? Math.min(100, (transfer.transferred / transfer.total) * 100) : 0;
        const verb = transfer.type === 'upload' ? '上传' : '下载';
        const canPause = transfer.status === 'running';
        const canResume = transfer.status === 'paused';
        const canCancel = ['queued', 'running', 'paused'].includes(transfer.status);

        return `
            <div class="transfer-item">
                <div class="transfer-icon">${transfer.type === 'upload' ? '⇧' : '⇩'}</div>
                <div class="transfer-main">
                    <div class="transfer-title-row">
                        <strong title="${escapeAttribute(transfer.name)}">${escapeHtml(transfer.name)}</strong>
                        <span>${verb} · ${transferStatusText(transfer.status)}</span>
                    </div>
                    <div class="transfer-progress"><span style="width:${percent}%"></span></div>
                    <div class="transfer-meta">
                        <span>${formatBytes(transfer.transferred)} / ${formatBytes(transfer.total)}</span>
                        <span>${transfer.speed > 0 ? `${formatBytes(transfer.speed)}/s` : transfer.message}</span>
                    </div>
                </div>
                <div class="transfer-actions">
                    ${canPause ? `<button class="action-btn" onclick="pauseTransfer('${transfer.id}')">暂停</button>` : ''}
                    ${canResume ? `<button class="action-btn" onclick="resumeTransfer('${transfer.id}')">继续</button>` : ''}
                    ${canCancel ? `<button class="action-btn danger" onclick="cancelTransfer('${transfer.id}')">取消</button>` : ''}
                </div>
            </div>
        `;
    }).join('');
}

function transferStatusText(status) {
    const labels = {
        queued: '等待中',
        running: '进行中',
        paused: '已暂停',
        done: '已完成',
        error: '失败',
        cancelled: '已取消',
    };
    return labels[status] || status;
}

function saveBlob(blob, filename) {
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    window.URL.revokeObjectURL(url);
}

function showMoveModal(file) {
    contextMenu.classList.add('hidden');
    selectedFile = file;

    const folderTree = document.getElementById('folderTree');
    const folders = files.filter(f => f.is_folder && f.id !== file.id);

    if (folders.length === 0) {
        folderTree.innerHTML = '<div style="padding:1rem;color:#64748b;">没有可用的文件夹</div>';
    } else {
        folderTree.innerHTML = folders.map(f => `
            <div class="folder-tree-item" data-id="${f.id}">
                📁 ${escapeHtml(f.name)}
            </div>
        `).join('');

        folderTree.querySelectorAll('.folder-tree-item').forEach(item => {
            item.addEventListener('click', () => {
                folderTree.querySelectorAll('.folder-tree-item').forEach(i => i.classList.remove('selected'));
                item.classList.add('selected');
            });
        });
    }

    document.getElementById('moveModal').classList.remove('hidden');
}

async function moveFile(file, targetParentId) {
    document.getElementById('moveModal').classList.add('hidden');

    try {
        const res = await fetch(`${API_BASE}/api/files/${file.id}`, {
            method: 'PATCH',
            headers: { 'Content-Type': 'application/json', ...getAuth() },
            body: JSON.stringify({ parent_id: targetParentId || '' })
        });
        if (!res.ok) {
            const data = await res.json();
            throw new Error(data.message || '移动失败');
        }
        showToast('移动成功', 'success');
        loadFiles();
    } catch (err) {
        showToast(err.message, 'error');
    }
}

// Share Functions
async function loadShares() {
    try {
        const res = await fetch(`${API_BASE}/api/shares`, { headers: getAuth() });
        shares = await res.json();
        renderShares();
    } catch (err) {
        showToast('加载分享失败', 'error');
    }
}

function renderShares() {
    if (shares.length === 0) {
        shareList.innerHTML = `
            <div class="empty-state">
                <div class="icon">🔗</div>
                <p>暂无分享链接</p>
            </div>
        `;
        return;
    }

    shareList.innerHTML = shares.map(share => `
        <div class="share-item">
            <span class="icon">🔗</span>
            <span class="share-token">${share.token.substring(0, 16)}...</span>
            <span class="file-meta">权限: ${share.permissions}</span>
            <button class="action-btn" onclick="copyShareLink('${share.token}')">复制链接</button>
            <button class="action-btn danger" onclick="deleteShare('${share.id}')">删除</button>
        </div>
    `).join('');
}

function showShareModal(file) {
    contextMenu.classList.add('hidden');
    selectedFile = file;
    document.getElementById('shareFileName').textContent = `为 "${file.name}" 创建分享`;
    document.getElementById('shareModal').classList.remove('hidden');
}

async function createShare() {
    const permissions = document.getElementById('sharePermissions').value;

    try {
        const res = await fetch(`${API_BASE}/api/shares`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', ...getAuth() },
            body: JSON.stringify({
                file_id: selectedFile.id,
                permissions,
                expires_at: null
            })
        });

        if (!res.ok) {
            const data = await res.json();
            throw new Error(data.message || '创建分享失败');
        }

        const share = await res.json();
        copyShareLink(share.token);
        document.getElementById('shareModal').classList.add('hidden');
        loadShares();
    } catch (err) {
        showToast(err.message, 'error');
    }
}

function copyShareLink(token) {
    const url = `${window.location.origin}/share/${token}`;
    // Also try to copy the API access URL
    const apiUrl = `${API_BASE}/api/shares/public/${token}`;
    navigator.clipboard.writeText(apiUrl).then(() => {
        showToast(`分享链接已复制: ${apiUrl}`, 'success');
    }).catch(() => {
        prompt('复制分享链接：', apiUrl);
    });
}

async function deleteShare(id) {
    if (!confirm('确定要删除这个分享链接吗？')) return;

    try {
        const res = await fetch(`${API_BASE}/api/shares/${id}`, {
            method: 'DELETE',
            headers: getAuth()
        });
        if (!res.ok) throw new Error('删除失败');
        showToast('分享链接已删除', 'success');
        loadShares();
    } catch (err) {
        showToast(err.message, 'error');
    }
}

// User Management (Admin Only)
async function loadUsers() {
    try {
        const res = await fetch(`${API_BASE}/api/users`, { headers: getAuth() });
        if (!res.ok) throw new Error('无权限访问');
        users = await res.json();
        renderUsers();
    } catch (err) {
        userList.innerHTML = `<div class="empty-state"><p>无法加载用户列表</p></div>`;
    }
}

function renderUsers() {
    if (users.length === 0) {
        userList.innerHTML = `<div class="empty-state"><p>暂无用户</p></div>`;
        return;
    }

    userList.innerHTML = users.map(user => `
        <div class="user-item">
            <span class="icon">👤</span>
            <div>
                <div class="file-name">${escapeHtml(user.username)}</div>
                <div class="user-email">${user.email || '无邮箱'}</div>
                <div class="user-stats">容量: ${formatBytes(user.storage_used)} / ${formatBytes(user.storage_quota)}</div>
            </div>
            <button class="action-btn danger" onclick="deleteUser('${user.id}')">删除</button>
        </div>
    `).join('');
}

async function deleteUser(id) {
    if (!confirm('确定要删除该用户吗？所有文件将被删除！')) return;

    try {
        const res = await fetch(`${API_BASE}/api/users/${id}`, {
            method: 'DELETE',
            headers: getAuth()
        });
        if (!res.ok) throw new Error('删除失败');
        showToast('用户已删除', 'success');
        loadUsers();
    } catch (err) {
        showToast(err.message, 'error');
    }
}

// Utility Functions
function getAuth() {
    const authHeader = { 'Authorization': `Bearer ${token}` };
    console.log('getAuth called, token:', token ? `${token.substring(0, 30)}...` : 'NONE');
    console.log('Auth header:', authHeader);
    return authHeader;
}

function getFileIcon(mimeType) {
    if (!mimeType) return '□';
    if (mimeType.startsWith('image/')) return '◫';
    if (mimeType.startsWith('video/')) return '▷';
    if (mimeType.startsWith('audio/')) return '♪';
    if (mimeType.includes('pdf')) return 'PDF';
    if (mimeType.includes('word') || mimeType.includes('document')) return 'DOC';
    if (mimeType.includes('sheet') || mimeType.includes('excel')) return 'XLS';
    if (mimeType.includes('zip') || mimeType.includes('archive')) return 'ZIP';
    return '□';
}

function isVideoFile(file) {
    if (file.mime_type && file.mime_type.startsWith('video/')) return true;

    const extension = file.name.split('.').pop()?.toLowerCase();
    return ['mp4', 'mkv', 'webm', 'mov', 'm4v', 'avi', 'mpeg', 'mpg', 'ogv'].includes(extension);
}

function updateStorageMeter() {
    const footerValue = document.querySelector('.sidebar-footer strong');
    const meter = document.querySelector('.storage-meter span');
    if (!currentUser || !footerValue || !meter) return;

    footerValue.textContent = `${formatBytes(currentUser.storage_used)} / ${formatBytes(currentUser.storage_quota)}`;
    const ratio = currentUser.storage_quota > 0 ? currentUser.storage_used / currentUser.storage_quota : 0;
    meter.style.width = `${Math.min(100, Math.max(0, ratio * 100))}%`;
}

function formatBytes(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

function escapeAttribute(text) {
    return escapeHtml(text).replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}

// Make functions globally accessible
window.downloadFile = downloadFile;
window.deleteFileById = deleteFileById;
window.copyShareLink = copyShareLink;
window.deleteShare = deleteShare;
window.deleteUser = deleteUser;
window.pauseTransfer = pauseTransfer;
window.resumeTransfer = resumeTransfer;
window.cancelTransfer = cancelTransfer;
