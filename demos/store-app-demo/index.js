const express = require('express');
const axios = require('axios');
const path = require('path');
require('dotenv').config();

const app = express();

// --------------------------------------------------------
// 1. Setup & Configuration
// --------------------------------------------------------
app.use(express.json());
app.set('view engine', 'ejs');
app.set('views', path.join(__dirname, 'views'));

const PORT = process.env.PORT || 5001;
const COWEN_PROXY_URL = process.env.COWEN_PROXY_URL || 'http://127.0.0.1:8080';

// ISV Application Credentials
const APP_KEY = process.env.COWEN_APP_KEY || '<YOUR_APP_KEY>';
const OPENAPI_URL = process.env.COWEN_OPENAPI_URL || 'https://openapi.chanjet.com';
const MARKET_URL = process.env.COWEN_MARKET_URL || 'https://market.chanjet.com';

// --------------------------------------------------------
// 2. UI Routes (Logic separated into EJS templates)
// --------------------------------------------------------

// Helper to get current base URL (Redirect URI)
function getRedirectUri(req) {
    const protocol = req.headers['x-forwarded-proto'] || req.protocol;
    const host = req.get('host');
    return `${protocol}://${host}/callback`;
}

// Landing Page: Initiate Authorization
app.get('/', (req, res) => {
    const redirectUri = getRedirectUri(req);
    const authUrl = `${MARKET_URL}/user/v2/authorize?client_id=${APP_KEY}&response_type=code&scope=all&state=demo_${Date.now()}&redirect_uri=${encodeURIComponent(redirectUri)}`;
    res.render('index', { authUrl });
});

// 2. OAuth2 Callback: Receive Auth Code from Platform
// --------------------------------------------------
app.get('/callback', async (req, res) => {
    const { code, state } = req.query;

    console.log(`\n[Callback] Received code from platform. State: ${state}`);

    try {
        // [主动模式] 业务系统显式调用 Cowen Proxy 换取令牌
        // 逻辑说明：
        // 1. 虽然 Cowen 能够通过 Stream 自动回收，但业务系统主动换票可以立即同步获得 orgId。
        // 2. Cowen Proxy 会拦截此请求，自动补全 AppSecret 并将结果归档。
        const response = await axios.post(`${COWEN_PROXY_URL}/oauth2/token`,
            new URLSearchParams({
                grant_type: 'authorization_code',
                code: code,
                redirect_uri: getRedirectUri(req)
            }).toString(),
            {
                headers: { 'Content-Type': 'application/x-www-form-urlencoded' }
            }
        );

        const tokenData = response.data;
        const orgId = tokenData.org_id || tokenData.orgId; // 从平台返回中提取租户 ID

        console.log(`[Exchange Success] Tenant Identified: ${orgId}`);

        res.render('callback', {
            code,
            state,
            orgId,
            tokenData
        });
    } catch (error) {
        // 异常处理：
        // 如果 Cowen 的 Stream Bridge 已经先一步完成了换票，这里的 Code 可能会报错“已使用”。
        // 此时我们尝试从 state 中恢复，或者等待 Webhook 通知。
        const errorDetail = error.response ? error.response.data : error.message;
        console.warn(`[Exchange Skip/Fail] Code might be consumed by Stream Bridge or expired:`, errorDetail);

        res.render('callback', {
            code,
            state,
            orgId: state.startsWith('demo_') ? 'Pending (Wait for Webhook)' : state,
            error: errorDetail
        });
    }
});

// --------------------------------------------------------
// 3. Logic Routes (Backend processing)
// --------------------------------------------------------

// Webhook Receiver: Receive push messages from Cowen
app.post('/webhook', (req, res) => {
    // Cowen injects orgId info into headers (if available) or it's in the message body
    const orgId = req.headers['x-org-id'] || req.headers['orgid'] || 'unknown';
    const message = req.body;

    console.log(`\n[Webhook Received] Tenant: ${orgId}`);
    console.log('Message Content:', JSON.stringify(message, null, 2));

    res.status(200).send('OK');
});

// API Test Endpoint: Call OpenAPI via Cowen Proxy
app.get('/api-test', async (req, res) => {
    const targetOrgId = req.query.orgId || 'demo_tenant_001';

    console.log(`\n[API Call] Triggering request for tenant: ${targetOrgId}`);

    try {
        // We call the LOCAL Cowen Proxy sidecar
        // Cowen handles token injection based on the x-org-id header.
        const response = await axios.get(`${COWEN_PROXY_URL}/accounting/openapi/cc/book/findByEnterpriseId?queryType=BINDING_TO_THIRD_PLATFORM`, {
            headers: {
                'x-org-id': targetOrgId
            }
        });

        res.json({
            success: true,
            orgId: targetOrgId,
            data: response.data
        });
    } catch (error) {
        const errorData = error.response ? error.response.data : error.message;
        console.error('[API Error]', errorData);
        res.status(500).json({
            success: false,
            message: 'Failed to call OpenAPI via Proxy',
            details: errorData
        });
    }
});

// --------------------------------------------------------
// 4. Server Initialization
// --------------------------------------------------------
app.listen(PORT, () => {
    console.log(`================================================`);
    console.log(`  Cowen Store App Demo (ISV Application)        `);
    console.log(`  Listening on port: ${PORT}                    `);
    console.log(`  Configured Proxy:  ${COWEN_PROXY_URL}         `);
    console.log(`  Home Page:         http://localhost:${PORT}   `);
    console.log(`================================================`);
});
