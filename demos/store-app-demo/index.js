const express = require('express');
const axios = require('axios');
require('dotenv').config();

const app = express();
app.use(express.json());

const PORT = process.env.PORT || 5000;
const COWEN_PROXY_URL = process.env.COWEN_PROXY_URL || 'http://127.0.0.1:8080';

// Configuration
const APP_KEY = process.env.COWEN_APP_KEY || '<YOUR_APP_KEY>';
const REDIRECT_URI = process.env.REDIRECT_URI || 'http://localhost:5000/callback';

// 1. Demo Home Page: Initiate Authorization
// -----------------------------------------
app.get('/', (req, res) => {
    const authUrl = `https://open.chanjet.com/user/v2/authorize?client_id=${APP_KEY}&response_type=code&scope=all&state=demo_${Date.now()}&redirect_uri=${encodeURIComponent(REDIRECT_URI)}`;
    
    res.send(`
        <!DOCTYPE html>
        <html lang="zh-CN">
        <head>
            <meta charset="UTF-8">
            <title>Cowen Store App Demo</title>
            <style>
                body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; background: #f4f7f9; color: #333; display: flex; justify-content: center; align-items: center; min-height: 100vh; margin: 0; }
                .card { background: white; padding: 2.5rem; border-radius: 16px; box-shadow: 0 20px 40px rgba(0,0,0,0.08); text-align: center; max-width: 480px; width: 90%; }
                .logo { font-size: 3rem; margin-bottom: 1rem; }
                h1 { font-size: 1.75rem; margin-bottom: 1rem; color: #111; }
                p { color: #666; line-height: 1.6; margin-bottom: 2rem; font-size: 1.05rem; }
                .btn { display: inline-block; background: #0070f3; color: white; padding: 0.9rem 2rem; border-radius: 8px; text-decoration: none; font-weight: 600; font-size: 1.1rem; transition: all 0.2s; box-shadow: 0 4px 14px 0 rgba(0,118,255,0.39); }
                .btn:hover { background: #0060df; transform: translateY(-2px); box-shadow: 0 6px 20px rgba(0,118,255,0.23); }
                .footer { margin-top: 3rem; font-size: 0.85rem; color: #bbb; border-top: 1px solid #eee; pt: 1.5rem; }
            </style>
        </head>
        <body>
            <div class="card">
                <div class="logo">🚀</div>
                <h1>Cowen Store App Demo</h1>
                <p>这是一个模拟 ISV 应用的集成演示。点击下方按钮引导租户管理员授权，Cowen Sidecar 将自动拦截授权码并为您完成后续所有令牌维护工作。</p>
                <a href="${authUrl}" class="btn">立即发起授权同步</a>
                <div class="footer">Powered by Cowen Architecture</div>
            </div>
        </body>
        </html>
    `);
});

// 2. OAuth2 Callback: Receive Auth Code from Platform
// --------------------------------------------------
app.get('/callback', (req, res) => {
    const { code, state } = req.query;
    res.send(`
        <!DOCTYPE html>
        <html lang="zh-CN">
        <head>
            <meta charset="UTF-8">
            <title>授权跳转成功</title>
            <style>
                body { font-family: -apple-system, sans-serif; background: #f8fafc; display: flex; justify-content: center; align-items: center; min-height: 100vh; margin: 0; }
                .card { background: white; padding: 2.5rem; border-radius: 16px; box-shadow: 0 10px 25px rgba(0,0,0,0.05); max-width: 600px; width: 90%; }
                h1 { color: #10b981; margin-bottom: 1rem; display: flex; align-items: center; gap: 0.5rem; }
                p { color: #475569; margin-bottom: 1.5rem; }
                .params { background: #f1f5f9; padding: 1.25rem; border-radius: 8px; font-family: monospace; border: 1px solid #e2e8f0; margin-bottom: 1.5rem; }
                .param-item { margin-bottom: 0.5rem; font-size: 0.9rem; }
                .param-label { color: #64748b; font-weight: bold; width: 60px; display: inline-block; }
                .tip { padding: 1rem; background: #f0f9ff; border-left: 4px solid #3b82f6; font-size: 0.9rem; color: #1e40af; margin-bottom: 2rem; line-height: 1.5; }
                .actions { display: flex; gap: 1rem; }
                .btn-test { background: #0f172a; color: white; padding: 0.75rem 1.5rem; border-radius: 6px; text-decoration: none; font-weight: 500; font-size: 0.95rem; }
            </style>
        </head>
        <body>
            <div class="card">
                <h1><svg width="24" height="24" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path d="M5 13l4 4L19 7"></path></svg> 授权跳转成功！</h1>
                <p>应用已接收到来自畅捷通开放平台的回调数据：</p>
                <div class="params">
                    <div class="param-item"><span class="param-label">Code:</span> ${code}</div>
                    <div class="param-item"><span class="param-label">State:</span> ${state}</div>
                </div>
                <div class="tip">
                    <strong>Cowen 运行机制提示：</strong><br>
                    虽然您在这里看到了 Code，但由于 Cowen 开启了 Stream Bridge，它已经**先于您的业务系统**自动拦截并交换了 PermanentAuthCode。您无需在后端手动调用 exchange 接口。
                </div>
                <div class="actions">
                    <a href="/api-test?orgId=${state.startsWith('demo_') ? 'YOUR_ORG_ID' : state}" class="btn-test">尝试调用 OpenAPI &rarr;</a>
                    <a href="/" style="color: #64748b; text-decoration: none; align-self: center; font-size: 0.9rem;">返回首页</a>
                </div>
            </div>
        </body>
        </html>
    `);
});

// 3. Webhook Endpoint: Receive messages forwarded by cowen
// --------------------------------------------------------
// Cowen will POST messages here when configured with --webhook-target
app.post('/webhook', (req, res) => {
    const orgId = req.header('orgId'); // Cowen automatically injects orgId in headers
    const message = req.body;

    console.log(`\n[Webhook Received] Tenant: ${orgId}`);
    console.log('Message Content:', JSON.stringify(message, null, 2));

    // Acknowledge receipt
    res.status(200).send('OK');
});

// 2. API Test Endpoint: Call OpenAPI via Cowen Proxy
// --------------------------------------------------
// This simulates your backend calling a Chanjet OpenAPI
app.get('/api-test', async (req, res) => {
    const targetOrgId = req.query.orgId || 'demo_tenant_001';
    
    console.log(`\n[API Call] Triggering request for tenant: ${targetOrgId}`);

    try {
        // We call the LOCAL Cowen Proxy. 
        // Cowen will handle token retrieval and injection based on the x-org-id header.
        const response = await axios.get(`${COWEN_PROXY_URL}/v1/user/info`, {
            headers: {
                'x-org-id': targetOrgId // [Standard] Tell Cowen which tenant context to use
            }
        });

        res.json({
            success: true,
            orgId: targetOrgId,
            data: response.data
        });
    } catch (error) {
        console.error('[API Error]', error.response ? error.response.data : error.message);
        res.status(500).json({
            success: false,
            message: 'Failed to call OpenAPI via Proxy',
            error: error.message
        });
    }
});

app.get('/', (req, res) => {
    res.send('Cowen Store App Demo is running. Use /api-test?orgId=xxx to test.');
});

app.listen(PORT, () => {
    console.log(`================================================`);
    console.log(`  Cowen Store App Demo (ISV Application)        `);
    console.log(`  Listening on port: ${PORT}                    `);
    console.log(`  Configured Proxy:  ${COWEN_PROXY_URL}         `);
    console.log(`================================================`);
});
