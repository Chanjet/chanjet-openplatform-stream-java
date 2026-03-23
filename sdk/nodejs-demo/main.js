import { GatewayClient, MessageDispatcher } from '@chanjet/connector-sdk';

const APP_KEY = 'XxG8gJUT';
const APP_SECRET = 'your_app_secret_placeholder';
const GATEWAY_URL = 'http://localhost:8080';

// 1. 初始化客户端
const client = new GatewayClient({
    appKey: APP_KEY,
    appSecret: APP_SECRET,
    gatewayUrl: GATEWAY_URL,
    clientId: 'nodejs-demo-client'
});

// 2. 初始化分发器
const dispatcher = new MessageDispatcher();

// 注册处理器
dispatcher.register('TEST_DATA', (msg) => {
    console.log('✅ [Demo] 收到业务消息:', msg);
    return true;
});

dispatcher.onAppTicket((msg) => {
    console.log('✅ [Demo] 收到应用票据:', msg.appTicket);
    return true;
});

// 3. 关联分发
client.onEvent(async (event) => {
    console.log('📩 [Demo] 收到原始事件:', event.msg_id);
    return await dispatcher.dispatch(event, APP_SECRET);
});

// 4. 启动
console.log('🚀 [Demo] 正在启动 Node.js SDK Demo...');
client.start();

process.on('SIGINT', () => {
    console.log('Stopping...');
    client.stop();
    process.exit(0);
});
