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

const PORT = process.env.PORT || 5000;
const COWEN_PROXY_URL = process.env.COWEN_PROXY_URL || 'http://127.0.0.1:8080';

// ISV Application Credentials
const APP_KEY = process.env.COWEN_APP_KEY || '<YOUR_APP_KEY>';
const REDIRECT_URI = process.env.REDIRECT_URI || `http://localhost:${PORT}/callback`;

// --------------------------------------------------------
// 2. UI Routes (Logic separated into EJS templates)
// --------------------------------------------------------

// Landing Page: Initiate Authorization
app.get('/', (req, res) => {
    const authUrl = `https://market.chanjet.com/user/v2/authorize?client_id=${APP_KEY}&response_type=code&scope=all&state=demo_${Date.now()}&redirect_uri=${encodeURIComponent(REDIRECT_URI)}`;
    res.render('index', { authUrl });
});

// Auth Callback: Display received parameters
app.get('/callback', (req, res) => {
    const { code, state } = req.query;
    res.render('callback', { code, state });
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
        const response = await axios.get(`${COWEN_PROXY_URL}/v1/user/info`, {
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
