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

// DOM Elements
const loginModal = document.getElementById('loginModal');
const registerModal = document.getElementById('registerModal');
const app = document.getElementById('app');
const userInfo = document.getElementById('userInfo');
const fileList = document.getElementById('fileList');
const shareList = document.getElementById('shareList');
const userList = document.getElementById('userList');
const contextMenu = document.getElementById('contextMenu');
const toast = document.getElementById('toast');

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

    // File operations
    document.getElementById('createFolderBtn').addEventListener('click', () => createFolder());
    document.getElementById('uploadBtn').addEventListener('click', () => document.getElementById('fileInput').click());
    document.getElementById('fileInput').addEventListener('change', handleFileUpload);

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
    if (files.length === 0) {
        fileList.innerHTML = `
            <div class="empty-state">
                <div class="icon">📂</div>
                <p>暂无文件，上传一个文件或创建文件夹开始使用</p>
            </div>
        `;
        return;
    }

    fileList.innerHTML = files.map(file => `
        <div class="file-item" data-id="${file.id}" data-folder="${file.is_folder}">
            <span class="file-icon">${file.is_folder ? '📁' : getFileIcon(file.mime_type)}</span>
            <span class="file-name">${escapeHtml(file.name)}</span>
            <span class="file-meta">${file.is_folder ? '' : formatBytes(file.size)}</span>
            <div class="file-actions">
                ${!file.is_folder ? `<button class="action-btn" onclick="downloadFile('${file.id}')">下载</button>` : ''}
                <button class="action-btn danger" onclick="deleteFileById('${file.id}')">删除</button>
            </div>
        </div>
    `).join('');

    // Add click handlers
    fileList.querySelectorAll('.file-item').forEach(item => {
        item.addEventListener('click', (e) => {
            if (e.target.closest('.action-btn')) return;
            const file = files.find(f => f.id === item.dataset.id);
            if (item.dataset.folder === 'true') {
                openFile(file);
            }
        });
        item.addEventListener('contextmenu', (e) => {
            const file = files.find(f => f.id === item.dataset.id);
            showContextMenu(e, file);
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
            body: JSON.stringify({ name, is_folder: true })
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

    try {
        // Backend expects the raw file bytes with the name in the query string
        const res = await fetch(`${API_BASE}/api/files/upload?filename=${encodeURIComponent(file.name)}`, {
            method: 'POST',
            headers: getAuth(),
            body: file
        });

        if (!res.ok) {
            const data = await res.json();
            throw new Error(data.message || '上传失败');
        }
        showToast('文件上传成功', 'success');
        loadFiles();
    } catch (err) {
        showToast(err.message, 'error');
    }
    e.target.value = '';
}

function openFile(file) {
    if (file.is_folder) {
        // For now, just show a toast - could implement folder browsing later
        showToast(`正在打开文件夹: ${file.name}`, 'success');
    } else {
        downloadFile(file.id);
    }
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

async function downloadFile(id) {
    try {
        const res = await fetch(`${API_BASE}/api/files/${id}/download`, { headers: getAuth() });
        if (!res.ok) throw new Error('下载失败');

        const blob = await res.blob();
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = '';
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        window.URL.revokeObjectURL(url);
    } catch (err) {
        showToast(err.message, 'error');
    }
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
    if (!mimeType) return '📄';
    if (mimeType.startsWith('image/')) return '🖼️';
    if (mimeType.startsWith('video/')) return '🎬';
    if (mimeType.startsWith('audio/')) return '🎵';
    if (mimeType.includes('pdf')) return '📕';
    if (mimeType.includes('word') || mimeType.includes('document')) return '📝';
    if (mimeType.includes('sheet') || mimeType.includes('excel')) return '📊';
    if (mimeType.includes('zip') || mimeType.includes('archive')) return '📦';
    return '📄';
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

// Make functions globally accessible
window.downloadFile = downloadFile;
window.deleteFileById = deleteFileById;
window.copyShareLink = copyShareLink;
window.deleteShare = deleteShare;
window.deleteUser = deleteUser;