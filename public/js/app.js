// ─── API Configuration ─────────────────────────────────────────────────────────

const API_BASE = '/api';

// ─── Token Management ──────────────────────────────────────────────────────────

function getToken() {
    return localStorage.getItem('writespace_token');
}

function setToken(token) {
    localStorage.setItem('writespace_token', token);
}

function removeToken() {
    localStorage.removeItem('writespace_token');
}

// ─── JWT Decode ────────────────────────────────────────────────────────────────

function getUserFromToken() {
    const token = getToken();
    if (!token) return null;

    try {
        const parts = token.split('.');
        if (parts.length !== 3) return null;

        const payload = parts[1];
        const padded = payload.replace(/-/g, '+').replace(/_/g, '/');
        const decoded = atob(padded);
        const parsed = JSON.parse(decoded);

        if (parsed.exp && parsed.exp * 1000 < Date.now()) {
            removeToken();
            return null;
        }

        return {
            id: parsed.sub,
            username: parsed.username,
            role: parsed.role,
        };
    } catch (e) {
        removeToken();
        return null;
    }
}

// ─── API Request Wrapper ───────────────────────────────────────────────────────

async function apiRequest(endpoint, options = {}) {
    const url = `${API_BASE}${endpoint}`;
    const headers = {
        'Content-Type': 'application/json',
    };

    const token = getToken();
    if (token) {
        headers['Authorization'] = `Bearer ${token}`;
    }

    const config = {
        headers,
        ...options,
    };

    if (config.body && typeof config.body === 'object' && !(config.body instanceof FormData)) {
        config.body = JSON.stringify(config.body);
    }

    const response = await fetch(url, config);

    if (response.status === 204) {
        return { ok: true, status: 204, data: null };
    }

    let data = null;
    try {
        data = await response.json();
    } catch (e) {
        // response had no JSON body
    }

    if (!response.ok) {
        const errorMessage = (data && data.error) ? data.error : `Request failed with status ${response.status}`;
        const error = new Error(errorMessage);
        error.status = response.status;
        error.data = data;
        throw error;
    }

    return { ok: true, status: response.status, data };
}

// ─── Auth Guards ───────────────────────────────────────────────────────────────

function requireAuth() {
    const user = getUserFromToken();
    if (!user) {
        window.location.href = '/login.html';
        return null;
    }
    return user;
}

function requireAdmin() {
    const user = requireAuth();
    if (!user) return null;

    if (user.role !== 'admin') {
        window.location.href = '/index.html';
        return null;
    }
    return user;
}

// ─── Logout ────────────────────────────────────────────────────────────────────

function logout() {
    removeToken();
    window.location.href = '/index.html';
}

// ─── Date Formatting ───────────────────────────────────────────────────────────

function formatDate(dateString) {
    const date = new Date(dateString);
    const options = {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
    };
    return date.toLocaleDateString('en-US', options);
}

// ─── Avatar / Badge Creation ───────────────────────────────────────────────────

function createAvatar(displayName, role) {
    const container = document.createElement('div');
    container.className = 'inline-flex items-center justify-center w-8 h-8 rounded-full text-xs font-bold flex-shrink-0';

    if (role === 'admin') {
        container.className += ' bg-amber-100 text-amber-700 border border-amber-300';
        container.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M10 2a1 1 0 01.832.445l2.084 3.126 3.584.836a1 1 0 01.52 1.598L14.5 10.88l.54 3.67a1 1 0 01-1.46 1.04L10 13.625l-3.58 1.965a1 1 0 01-1.46-1.04l.54-3.67L2.98 8.005a1 1 0 01.52-1.598l3.584-.836L9.168 2.445A1 1 0 0110 2z" clip-rule="evenodd"/></svg>';
        container.title = `${displayName} (Admin)`;
    } else {
        container.className += ' bg-blue-100 text-blue-700 border border-blue-300';
        const initials = getInitials(displayName);
        container.textContent = initials;
        container.title = displayName;
    }

    return container;
}

function getInitials(name) {
    if (!name) return '?';
    const parts = name.trim().split(/\s+/);
    if (parts.length === 1) {
        return parts[0].charAt(0).toUpperCase();
    }
    return (parts[0].charAt(0) + parts[parts.length - 1].charAt(0)).toUpperCase();
}

// ─── Navigation Bar ────────────────────────────────────────────────────────────

function createNav() {
    const user = getUserFromToken();
    const nav = document.createElement('nav');
    nav.className = 'bg-white shadow-sm border-b border-gray-200';

    const inner = document.createElement('div');
    inner.className = 'max-w-5xl mx-auto px-4 sm:px-6 lg:px-8 flex items-center justify-between h-16';

    // Logo / Brand
    const brand = document.createElement('a');
    brand.href = '/index.html';
    brand.className = 'text-xl font-bold text-gray-900 hover:text-blue-600 transition-colors';
    brand.textContent = 'WriteSpace';
    inner.appendChild(brand);

    // Nav links container
    const links = document.createElement('div');
    links.className = 'flex items-center space-x-4';

    // Home link
    const homeLink = createNavLink('/index.html', 'Home');
    links.appendChild(homeLink);

    if (user) {
        // Posts link (all posts)
        const postsLink = createNavLink('/posts.html', 'Posts');
        links.appendChild(postsLink);

        // New Post link
        const newPostLink = createNavLink('/new-post.html', 'New Post');
        links.appendChild(newPostLink);

        // Admin links
        if (user.role === 'admin') {
            const adminLink = createNavLink('/admin.html', 'Admin');
            adminLink.className += ' text-amber-600 hover:text-amber-700';
            links.appendChild(adminLink);
        }

        // Separator
        const separator = document.createElement('span');
        separator.className = 'text-gray-300';
        separator.textContent = '|';
        links.appendChild(separator);

        // User info
        const userInfo = document.createElement('span');
        userInfo.className = 'text-sm text-gray-600 flex items-center space-x-2';

        const avatar = createAvatar(user.username, user.role);
        userInfo.appendChild(avatar);

        const userName = document.createElement('span');
        userName.className = 'hidden sm:inline';
        userName.textContent = user.username;
        userInfo.appendChild(userName);

        links.appendChild(userInfo);

        // Logout button
        const logoutBtn = document.createElement('button');
        logoutBtn.className = 'text-sm text-red-600 hover:text-red-800 font-medium transition-colors cursor-pointer';
        logoutBtn.textContent = 'Logout';
        logoutBtn.addEventListener('click', function (e) {
            e.preventDefault();
            logout();
        });
        links.appendChild(logoutBtn);
    } else {
        // Login link
        const loginLink = createNavLink('/login.html', 'Login');
        links.appendChild(loginLink);

        // Register link
        const registerLink = createNavLink('/register.html', 'Register');
        registerLink.className = 'inline-flex items-center px-3 py-1.5 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 transition-colors';
        links.appendChild(registerLink);
    }

    inner.appendChild(links);
    nav.appendChild(inner);

    return nav;
}

function createNavLink(href, text) {
    const link = document.createElement('a');
    link.href = href;
    link.className = 'text-sm text-gray-600 hover:text-gray-900 font-medium transition-colors';
    link.textContent = text;
    return link;
}

// ─── Utility: Show Error Message ───────────────────────────────────────────────

function showError(container, message) {
    const div = document.createElement('div');
    div.className = 'bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-md text-sm';
    div.textContent = message;
    container.prepend(div);
    setTimeout(function () {
        if (div.parentNode) {
            div.remove();
        }
    }, 5000);
}

function showSuccess(container, message) {
    const div = document.createElement('div');
    div.className = 'bg-green-50 border border-green-200 text-green-700 px-4 py-3 rounded-md text-sm';
    div.textContent = message;
    container.prepend(div);
    setTimeout(function () {
        if (div.parentNode) {
            div.remove();
        }
    }, 5000);
}