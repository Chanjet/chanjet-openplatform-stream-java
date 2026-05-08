const express = require('express');
const axios = require('axios');
require('dotenv').config();

const app = express();
app.use(express.json());

const PORT = process.env.PORT || 5000;
const COWEN_PROXY_URL = process.env.COWEN_PROXY_URL || 'http://127.0.0.1:8080';

// 1. Webhook Endpoint: Receive messages forwarded by cowen
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
        // Cowen will handle token retrieval and injection based on the orgId header.
        const response = await axios.get(`${COWEN_PROXY_URL}/v1/user/info`, {
            headers: {
                'orgId': targetOrgId // Critical: Tell Cowen which tenant context to use
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
