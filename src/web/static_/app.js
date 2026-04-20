// Tab switching
document.querySelectorAll('.tab').forEach(tab => {
    tab.addEventListener('click', () => {
        document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
        document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));
        tab.classList.add('active');
        document.getElementById(tab.dataset.tab).classList.add('active');
        loadTab(tab.dataset.tab);
    });
});

async function api(path, method = 'GET', body = null) {
    const opts = { method, headers: { 'Content-Type': 'application/json' } };
    if (body) opts.body = JSON.stringify(body);
    const res = await fetch(path, opts);
    const text = await res.text();
    try { return { data: JSON.parse(text), status: res.status }; }
    catch { return { data: text, status: res.status }; }
}

async function loadTab(tab) {
    switch (tab) {
        case 'status': await loadStatus(); break;
        case 'memories': await loadMemories(); break;
        case 'agents': await loadAgents(); break;
        case 'skills': await loadSkills(); break;
        case 'channels': await loadChannels(); break;
        case 'tasks': await loadTasks(); break;
    }
}

// Status
async function loadStatus() {
    const { data, status } = await api('/api/status');
    if (status !== 200) return;
    const el = document.getElementById('status-details');
    el.innerHTML = `
        <div class="status-item"><div class="label">Bot</div><div class="value">${data.bot_name}</div></div>
        <div class="status-item"><div class="label">Memories</div><div class="value">${data.memories}</div></div>
        <div class="status-item"><div class="label">Active Agents</div><div class="value">${data.active_agents}</div></div>
        <div class="status-item"><div class="label">Skills</div><div class="value">${data.skills}</div></div>
        <div class="status-item"><div class="label">Uptime</div><div class="value">${data.uptime_text}</div></div>
    `;
}

// Memories
async function loadMemories() {
    const { data } = await api('/api/memories');
    const el = document.getElementById('memories-list');
    document.getElementById('memory-count').textContent = `(${data.length})`;
    if (data.length === 0) {
        el.innerHTML = '<div class="empty">メモリはありません</div>';
        return;
    }
    el.innerHTML = data.map(m => `
        <div class="list-item" data-id="${m.id}">
            <div class="info">
                <div class="name">${esc(m.content)}</div>
                <div class="detail">[${esc(m.category)}] ${esc(m.created_at)} <span class="badge">${m.id.substring(0, 8)}</span></div>
            </div>
            <button class="btn btn-danger btn-sm" onclick="deleteMemory('${m.id}')">Delete</button>
        </div>
    `).join('');
}

document.getElementById('memory-search').addEventListener('input', (e) => {
    const q = e.target.value.toLowerCase();
    document.querySelectorAll('#memories-list .list-item').forEach(item => {
        const text = item.textContent.toLowerCase();
        item.style.display = text.includes(q) ? '' : 'none';
    });
});

async function deleteMemory(id) {
    const { status } = await api(`/api/memories/${id}`, 'DELETE');
    if (status === 200) loadMemories();
}

// Agents
async function loadAgents() {
    const { data } = await api('/api/agents');
    const el = document.getElementById('agents-list');
    document.getElementById('agent-count').textContent = `(${data.length})`;
    if (data.length === 0) {
        el.innerHTML = '<div class="empty">実行中のエージェントはありません</div>';
        return;
    }
    el.innerHTML = data.map(a => `
        <div class="list-item">
            <div class="info">
                <div class="name">${esc(a.name)}</div>
                <div class="detail">${esc(a.id)}</div>
            </div>
            <span class="badge">${esc(a.status)}</span>
            <button class="btn btn-danger btn-sm" style="margin-left:0.5rem" onclick="stopAgent('${a.id}')">Stop</button>
        </div>
    `).join('');
}

async function stopAgent(id) {
    await api(`/api/agents/${id}/stop`, 'POST');
    loadAgents();
}

// Skills
async function loadSkills() {
    const { data } = await api('/api/skills');
    const el = document.getElementById('skills-list');
    if (data.length === 0) {
        el.innerHTML = '<div class="empty">スキルはありません</div>';
        return;
    }
    el.innerHTML = data.map(s => `
        <div class="list-item">
            <div class="info">
                <div class="name">${esc(s.name)}</div>
                <div class="detail">${esc(s.description)}</div>
            </div>
            <div style="display:flex;align-items:center">
                <input type="text" class="skill-input" placeholder="args" id="skill-arg-${s.name}">
                <button class="btn btn-primary btn-sm" onclick="executeSkill('${s.name}')">Execute</button>
            </div>
        </div>
        <div class="skill-result" id="skill-result-${s.name}"></div>
    `).join('');
}

async function executeSkill(name) {
    const arg = document.getElementById(`skill-arg-${name}`).value;
    const resultEl = document.getElementById(`skill-result-${name}`);
    resultEl.textContent = 'Executing...';
    resultEl.classList.add('visible');
    const { data, status } = await api(`/api/skills/${name}/execute`, 'POST', arg);
    resultEl.textContent = status === 200 ? data : `Error: ${data}`;
}

// Channels
async function loadChannels() {
    const { data } = await api('/api/channels');
    const el = document.getElementById('channels-list');
    if (data.length === 0) {
        el.innerHTML = '<div class="empty">常に返信するチャンネルはありません</div>';
        return;
    }
    el.innerHTML = data.map(c => `
        <div class="list-item">
            <div class="info">
                <div class="name">${esc(c.channel_id)}</div>
                <div class="detail">${c.always_respond ? 'Always responding' : ''}</div>
            </div>
            <button class="btn btn-danger btn-sm" onclick="removeChannel('${c.channel_id}')">Remove</button>
        </div>
    `).join('');
}

async function addChannel() {
    const input = document.getElementById('channel-add-input');
    const id = input.value.trim();
    if (!id) return;
    await api('/api/channels/add', 'POST', { channel_id: id });
    input.value = '';
    loadChannels();
}

async function removeChannel(id) {
    await api('/api/channels/remove', 'POST', { channel_id: id });
    loadChannels();
}

// Tasks
async function loadTasks() {
    const { data } = await api('/api/tasks');
    const el = document.getElementById('tasks-list');
    if (data.length === 0) {
        el.innerHTML = '<div class="empty">登録されたタスクはありません</div>';
        return;
    }
    el.innerHTML = data.map(t => `
        <div class="list-item">
            <div class="info">
                <div class="name">${esc(t.name)}</div>
                <div class="detail">cron: ${esc(t.cron_expression)} | next: ${esc(t.next_run || 'N/A')} | last: ${esc(t.last_run || 'never')}</div>
            </div>
            <span class="badge">${t.enabled ? 'active' : 'stopped'}</span>
        </div>
    `).join('');
}

function esc(s) {
    const d = document.createElement('div');
    d.textContent = s;
    return d.innerHTML;
}

// Load initial tab
loadTab('status');
