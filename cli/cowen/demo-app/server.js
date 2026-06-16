const express = require('express');
const axios = require('axios');
const cookieParser = require('cookie-parser');
const path = require('path');

const app = express();
const PORT = 13000;
const COWEN_PROXY_URL = 'http://127.0.0.1:18081';

app.use(cookieParser());
app.use(express.json());

// Serve static files from 'public' directory (accessible via /public/...)
app.use('/public', express.static(path.join(__dirname, 'public')));

// Root route - Render the premium dashboard
app.get('/', (req, res) => {
  const orgId = req.headers['x-org-id'];
  const userId = req.headers['x-user-id'];
  const appId = req.headers['x-app-id'];

  // If accessed directly (not via Ingress Gateway), show warning/instructions
  if (!orgId) {
    return res.status(400).send(`
      <!DOCTYPE html>
      <html lang="zh-CN">
      <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>接入指引 - Cowen 零信任网关</title>
        <link href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;600;700&display=swap" rel="stylesheet">
        <style>
          :root {
            --bg-color: #0b0f19;
            --card-bg: rgba(17, 24, 39, 0.7);
            --border-color: rgba(255, 255, 255, 0.08);
            --text-color: #f3f4f6;
            --text-muted: #9ca3af;
            --primary: #3b82f6;
            --primary-glow: rgba(59, 130, 246, 0.15);
            --accent: #f59e0b;
          }
          * { box-sizing: border-box; margin: 0; padding: 0; }
          body {
            font-family: 'Inter', sans-serif;
            background-color: var(--bg-color);
            color: var(--text-color);
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            overflow-x: hidden;
          }
          .glass-card {
            background: var(--card-bg);
            backdrop-filter: blur(16px);
            -webkit-backdrop-filter: blur(16px);
            border: 1px solid var(--border-color);
            border-radius: 24px;
            padding: 40px;
            max-width: 650px;
            width: 90%;
            box-shadow: 0 20px 50px rgba(0, 0, 0, 0.3);
            text-align: center;
          }
          h1 {
            font-size: 2.2rem;
            font-weight: 700;
            margin-bottom: 15px;
            background: linear-gradient(135deg, #60a5fa, #3b82f6);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
          }
          p {
            color: var(--text-muted);
            line-height: 1.6;
            margin-bottom: 25px;
            font-size: 1.05rem;
          }
          .alert {
            background: rgba(245, 158, 11, 0.1);
            border: 1px solid rgba(245, 158, 11, 0.2);
            border-radius: 12px;
            padding: 15px;
            margin-bottom: 25px;
            color: #fbbf24;
            font-size: 0.95rem;
            text-align: left;
            display: flex;
            align-items: flex-start;
            gap: 10px;
          }
          .flow-box {
            background: rgba(255, 255, 255, 0.02);
            border: 1px solid var(--border-color);
            border-radius: 16px;
            padding: 20px;
            margin-bottom: 30px;
            text-align: left;
          }
          .flow-title {
            font-weight: 600;
            margin-bottom: 12px;
            font-size: 0.95rem;
            text-transform: uppercase;
            letter-spacing: 0.05em;
            color: var(--text-color);
          }
          .flow-step {
            display: flex;
            align-items: center;
            gap: 12px;
            margin-bottom: 10px;
            font-size: 0.9rem;
          }
          .flow-step:last-child { margin-bottom: 0; }
          .step-num {
            background: var(--primary);
            color: #fff;
            width: 22px;
            height: 22px;
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 0.8rem;
            font-weight: 700;
          }
          .btn-primary {
            display: inline-block;
            background: var(--primary);
            color: #white;
            text-decoration: none;
            padding: 14px 28px;
            border-radius: 12px;
            font-weight: 600;
            font-size: 1rem;
            transition: all 0.3s ease;
            box-shadow: 0 4px 14px var(--primary-glow);
            border: none;
            cursor: pointer;
          }
          .btn-primary:hover {
            transform: translateY(-2px);
            box-shadow: 0 6px 20px rgba(59, 130, 246, 0.4);
            background: #2563eb;
          }
        </style>
      </head>
      <body>
        <div class="glass-card">
          <h1>访问被阻断 (Access Blocked)</h1>
          <p>您正试图直接访问 ISV 后端服务。为了系统的零信任安全性，我们强制要求通过 Ingress 网关进行身份认证和路由。</p>
          
          <div class="alert">
            <span>⚠️</span>
            <div>
              <strong>提示：</strong>直接访问后端将无法获取企业和用户的明文身份（<code>x-org-id</code>，<code>x-user-id</code>），导致所有的开放平台 API 代理调用失败。
            </div>
          </div>

          <div class="flow-box">
            <div class="flow-title">零信任链路拓扑:</div>
            <div class="flow-step">
              <span class="step-num">1</span>
              <span>浏览器发起请求到网关端口 <code>http://127.0.0.1:18080/</code></span>
            </div>
            <div class="flow-step">
              <span class="step-num">2</span>
              <span>Cowen Ingress Gateway 进行身份鉴权并注入身份头</span>
            </div>
            <div class="flow-step">
              <span class="step-num">3</span>
              <span>后端服务 (当前端口 13000) 安全接收并处理业务逻辑</span>
            </div>
          </div>

          <a href="http://127.0.0.1:18080/" class="btn-primary">通过网关入口访问</a>
        </div>
      </body>
      </html>
    `);
  }

  // Forward the dashboard, inject credentials via dynamic HTML scripting
  res.send(`
    <!DOCTYPE html>
    <html lang="zh-CN">
    <head>
      <meta charset="UTF-8">
      <meta name="viewport" content="width=device-width, initial-scale=1.0">
      <title>ISV Dashboard - Cowen Secure Gateway</title>
      <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;600;800&family=Inter:wght@300;400;600;700&display=swap" rel="stylesheet">
      <style>
        :root {
          --bg-color: #080c14;
          --card-bg: rgba(15, 23, 42, 0.4);
          --border-color: rgba(255, 255, 255, 0.05);
          --text-color: #f8fafc;
          --text-muted: #94a3b8;
          --primary: #3b82f6;
          --primary-glow: rgba(59, 130, 246, 0.25);
          --secondary: #10b981;
          --secondary-glow: rgba(16, 185, 129, 0.25);
          --accent: #8b5cf6;
        }
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body {
          font-family: 'Inter', sans-serif;
          background-color: var(--bg-color);
          background-image: 
            radial-gradient(at 0% 0%, rgba(59, 130, 246, 0.12) 0px, transparent 50%),
            radial-gradient(at 100% 100%, rgba(139, 92, 246, 0.12) 0px, transparent 50%);
          color: var(--text-color);
          min-height: 100vh;
          padding: 40px 20px;
          display: flex;
          justify-content: center;
          align-items: center;
        }
        .container {
          max-width: 900px;
          width: 100%;
        }
        .dashboard {
          background: var(--card-bg);
          backdrop-filter: blur(20px);
          -webkit-backdrop-filter: blur(20px);
          border: 1px solid var(--border-color);
          border-radius: 32px;
          padding: 40px;
          box-shadow: 0 25px 60px rgba(0, 0, 0, 0.4);
        }
        .header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          border-bottom: 1px solid rgba(255, 255, 255, 0.05);
          padding-bottom: 25px;
          margin-bottom: 30px;
        }
        .header h1 {
          font-family: 'Outfit', sans-serif;
          font-size: 2rem;
          font-weight: 800;
          background: linear-gradient(to right, #60a5fa, #a78bfa);
          -webkit-background-clip: text;
          -webkit-text-fill-color: transparent;
        }
        .status-badge {
          background: rgba(16, 185, 129, 0.1);
          border: 1px solid rgba(16, 185, 129, 0.2);
          color: #34d399;
          padding: 6px 14px;
          border-radius: 50px;
          font-size: 0.85rem;
          font-weight: 600;
          display: flex;
          align-items: center;
          gap: 6px;
        }
        .status-dot {
          width: 8px;
          height: 8px;
          background-color: var(--secondary);
          border-radius: 50%;
          animation: pulse 1.5s infinite;
        }
        @keyframes pulse {
          0% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.7); }
          70% { transform: scale(1); box-shadow: 0 0 0 6px rgba(16, 185, 129, 0); }
          100% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(16, 185, 129, 0); }
        }
        .grid {
          display: grid;
          grid-template-columns: 1fr 1fr;
          gap: 25px;
          margin-bottom: 30px;
        }
        .card {
          background: rgba(255, 255, 255, 0.02);
          border: 1px solid var(--border-color);
          border-radius: 20px;
          padding: 24px;
        }
        .card-title {
          font-size: 0.85rem;
          font-weight: 700;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          color: var(--text-muted);
          margin-bottom: 15px;
        }
        .info-row {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 12px;
          font-size: 0.95rem;
        }
        .info-row:last-child { margin-bottom: 0; }
        .info-label { color: var(--text-muted); }
        .info-value { font-family: monospace; font-weight: 600; color: #fff; }
        
        .action-panel {
          text-align: center;
          margin-bottom: 30px;
        }
        .btn-action {
          background: linear-gradient(135deg, var(--primary), var(--accent));
          color: #fff;
          border: none;
          padding: 16px 36px;
          border-radius: 16px;
          font-size: 1rem;
          font-weight: 700;
          cursor: pointer;
          transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
          box-shadow: 0 10px 20px var(--primary-glow);
          display: inline-flex;
          align-items: center;
          gap: 10px;
        }
        .btn-action:hover {
          transform: translateY(-3px);
          box-shadow: 0 15px 30px rgba(59, 130, 246, 0.4);
          filter: brightness(1.1);
        }
        .btn-action:active {
          transform: translateY(-1px);
        }
        
        .result-panel {
          background: rgba(0, 0, 0, 0.3);
          border: 1px solid var(--border-color);
          border-radius: 20px;
          padding: 24px;
          min-height: 200px;
          position: relative;
        }
        .result-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 15px;
          font-size: 0.9rem;
          color: var(--text-muted);
        }
        pre {
          background: transparent;
          color: #e2e8f0;
          font-family: 'Fira Code', monospace;
          font-size: 0.9rem;
          overflow-x: auto;
          white-space: pre-wrap;
          line-height: 1.5;
        }
        .loading-spinner {
          display: none;
          border: 3px solid rgba(255, 255, 255, 0.1);
          width: 24px;
          height: 24px;
          border-radius: 50%;
          border-left-color: #fff;
          animation: spin 1s linear infinite;
        }
        @keyframes spin {
          0% { transform: rotate(0deg); }
          100% { transform: rotate(360deg); }
        }
      </style>
    </head>
    <body>
      <div class="container">
        <div class="dashboard">
          <div class="header">
            <h1>ISV Dashboard</h1>
            <div style="display: flex; align-items: center; gap: 15px;">
              <div class="status-badge">
                <span class="status-dot"></span>
                网关鉴权已通过 (Active)
              </div>
              <a href="/logout" style="text-decoration: none; background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.2); color: #f87171; padding: 6px 14px; border-radius: 50px; font-size: 0.85rem; font-weight: 600; cursor: pointer; transition: all 0.3s;" onmouseover="this.style.background='rgba(239, 68, 68, 0.2)'" onmouseout="this.style.background='rgba(239, 68, 68, 0.1)'">
                退出登录 (Logout)
              </a>
            </div>
          </div>
          
          <div class="grid">
            <div class="card">
              <div class="card-title">网关注入的身份信息 (Ingress Headers)</div>
              <div class="info-row">
                <span class="info-label">企业 ID (x-org-id):</span>
                <span class="info-value">${orgId}</span>
              </div>
              <div class="info-row">
                <span class="info-label">用户 ID (x-user-id):</span>
                <span class="info-value">${userId}</span>
              </div>
              <div class="info-row">
                <span class="info-label">应用 ID (x-app-id):</span>
                <span class="info-value">${appId || 'N/A'}</span>
              </div>
            </div>
            
            <div class="card">
              <div class="card-title">网络拓扑状态 (Network Topology)</div>
              <div class="info-row">
                <span class="info-label">网关地址 (Ingress Port):</span>
                <span class="info-value">http://127.0.0.1:18080</span>
              </div>
              <div class="info-row">
                <span class="info-label">Upstream 地址:</span>
                <span class="info-value">http://127.0.0.1:13000</span>
              </div>
              <div class="info-row">
                <span class="info-label">Egress 代理:</span>
                <span class="info-value">http://127.0.0.1:18081</span>
              </div>
            </div>
          </div>

          <div class="card" style="margin-top: 25px;">
            <div class="card-title">API 调试面板 (API Operations)</div>
            <div class="action-panel" style="display: flex; justify-content: center; gap: 20px; flex-wrap: wrap; margin-bottom: 25px;">
              <button class="btn-action" id="fetch-user-btn">
                <div class="loading-spinner" id="btn-spinner"></div>
                <span>个人授权调用 (含 x-user-id)</span>
              </button>
              <button class="btn-action" id="fetch-user-org-btn" style="background: linear-gradient(135deg, var(--secondary), var(--accent)); box-shadow: 0 10px 20px var(--secondary-glow);">
                <div class="loading-spinner" id="btn-spinner-org"></div>
                <span>企业授权调用 (不含 x-user-id)</span>
              </button>
              <button class="btn-action" id="fetch-books-btn" style="background: linear-gradient(135deg, var(--accent), var(--primary)); box-shadow: 0 10px 20px rgba(139, 92, 246, 0.25);">
                <div class="loading-spinner" id="btn-spinner-books"></div>
                <span>获取账套列表 (GET /accounting/openapi/cc/book/findByEnterpriseId)</span>
              </button>
              <button class="btn-action" id="fetch-openapi-bypass-btn" style="background: linear-gradient(135deg, var(--primary), var(--secondary)); box-shadow: 0 10px 20px rgba(59, 130, 246, 0.25);">
                <div class="loading-spinner" id="btn-spinner-bypass"></div>
                <span>网关旁挂 OpenAPI 调用 (GET /openapi/accounting/cia/api/v1/user)</span>
              </button>
            </div>

            <div class="result-panel">
              <div class="result-header">
                <span>调用结果 (OpenAPI Result):</span>
                <span id="response-time"></span>
              </div>
              <pre id="result-display">// 点击上方按钮发起调用。请求将通过 Cowen Local Proxy (Egress) 自动加签鉴权并获取数据。</pre>
            </div>
          </div>

      <script>
        const fetchBtn = document.getElementById('fetch-user-btn');
        const fetchOrgBtn = document.getElementById('fetch-user-org-btn');
        const fetchBooksBtn = document.getElementById('fetch-books-btn');
        const fetchBypassBtn = document.getElementById('fetch-openapi-bypass-btn');
        const spinner = document.getElementById('btn-spinner');
        const spinnerOrg = document.getElementById('btn-spinner-org');
        const spinnerBooks = document.getElementById('btn-spinner-books');
        const spinnerBypass = document.getElementById('btn-spinner-bypass');
        const display = document.getElementById('result-display');
        const timeDisplay = document.getElementById('response-time');
        
        async function makeRequest(url, spinnerEl) {
          spinnerEl.style.display = 'block';
          display.textContent = '正在发起请求并进行自动令牌挂载...';
          timeDisplay.textContent = '';
          const startTime = performance.now();
          try {
            const res = await fetch(url, {
              headers: {
                'Accept': 'application/json',
                'X-Requested-With': 'XMLHttpRequest'
              }
            });
            const contentType = res.headers.get('content-type') || '';
            if (contentType.includes('text/html')) {
              const htmlText = await res.text();
              const endTime = performance.now();
              timeDisplay.textContent = \`耗时: \${Math.round(endTime - startTime)}ms (HTML)\`;
              display.textContent = \`[错误: 接口返回了 HTML 页面 (Status \${res.status})，通常代表未通过网关登录拦截，或后端报错]\\n\\n\` + htmlText;
              return;
            }
            const data = await res.json();
            const endTime = performance.now();
            timeDisplay.textContent = \`耗时: \${Math.round(endTime - startTime)}ms\`;
            display.textContent = JSON.stringify(data, null, 2);
          } catch (err) {
            display.textContent = '错误: ' + err.message;
          } finally {
            spinnerEl.style.display = 'none';
          }
        }

        fetchBtn.addEventListener('click', () => makeRequest('/api/user-info', spinner));
        fetchOrgBtn.addEventListener('click', () => makeRequest('/api/user-info-org', spinnerOrg));
        fetchBooksBtn.addEventListener('click', () => makeRequest('/api/books', spinnerBooks));
        fetchBypassBtn.addEventListener('click', () => makeRequest('/openapi/accounting/cia/api/v1/user', spinnerBypass));
      </script>
    </body>
    </html>
  `);
});

// Proxy target endpoint: Call Chanjet OpenAPI via Cowen Local Proxy (User Authorization)
app.get('/api/user-info', async (req, res) => {
  const orgId = req.headers['x-org-id'];
  const userId = req.headers['x-user-id'];

  if (!orgId) {
    return res.status(401).json({ error: 'x-org-id is missing in request headers' });
  }

  // Forward both org_id and user_id to the egress proxy
  const proxyHeaders = {
    'x-org-id': orgId
  };
  if (userId) {
    proxyHeaders['x-user-id'] = userId;
  }

  try {
    // 💡 Using the Cowen Local Proxy (Egress) at http://127.0.0.1:18081
    // We send a direct request to the proxy with the desired OpenAPI path,
    // and include 'x-org-id' and 'x-user-id' in the headers.
    const response = await axios.get(`${COWEN_PROXY_URL}/accounting/cia/api/v1/user`, {
      headers: proxyHeaders
    });

    res.json(response.data);
  } catch (error) {
    const status = error.response ? error.response.status : 500;
    const errorData = error.response ? error.response.data : { error: error.message };
    res.status(status).json({
      message: 'Failed to fetch user info from Chanjet OpenAPI',
      status: status,
      details: errorData
    });
  }
});

// Proxy target endpoint: Call Chanjet OpenAPI via Cowen Local Proxy (Enterprise/Org Authorization ONLY)
app.get('/api/user-info-org', async (req, res) => {
  const orgId = req.headers['x-org-id'];

  if (!orgId) {
    return res.status(401).json({ error: 'x-org-id is missing in request headers' });
  }

  // Forward ONLY org_id to the egress proxy to trigger Org-level authentication
  const proxyHeaders = {
    'x-org-id': orgId
  };

  try {
    const response = await axios.get(`${COWEN_PROXY_URL}/accounting/cia/api/v1/user`, {
      headers: proxyHeaders
    });

    res.json(response.data);
  } catch (error) {
    const status = error.response ? error.response.status : 500;
    const errorData = error.response ? error.response.data : { error: error.message };
    res.status(status).json({
      message: 'Failed to fetch user info from Chanjet OpenAPI (Org Auth)',
      status: status,
      details: errorData
    });
  }
});

// Proxy target endpoint: Call Chanjet OpenAPI via Cowen Local Proxy (Query Book List)
app.get('/api/books', async (req, res) => {
  const orgId = req.headers['x-org-id'];
  const userId = req.headers['x-user-id'];

  if (!orgId) {
    return res.status(401).json({ error: 'x-org-id is missing in request headers' });
  }

  // Forward both org_id and user_id to the egress proxy
  const proxyHeaders = {
    'x-org-id': orgId
  };
  if (userId) {
    proxyHeaders['x-user-id'] = userId;
  }

  try {
    const response = await axios.get(`${COWEN_PROXY_URL}/accounting/openapi/cc/book/findByEnterpriseId?queryType=BINDING_TO_THIRD_PLATFORM`, {
      headers: proxyHeaders
    });

    res.json(response.data);
  } catch (error) {
    const status = error.response ? error.response.status : 500;
    const errorData = error.response ? error.response.data : { error: error.message };
    res.status(status).json({
      message: 'Failed to fetch accounting book list from Chanjet OpenAPI',
      status: status,
      details: errorData
    });
  }
});

// Clear session cookie and redirect to root to trigger gateway auth re-login
app.get('/logout', (req, res) => {
  res.clearCookie('cowen_sess_id', { path: '/' });
  res.redirect('/public/logout.html');
});

// Start Node.js application
app.listen(PORT, '127.0.0.1', () => {
  console.log(`Node.js Demo App running at http://127.0.0.1:${PORT}`);
});

