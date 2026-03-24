import { GatewayClient, MessageDispatcher } from '@chanjet/connector-sdk';
import 'dotenv/config'; // 自动加载当前目录下的 .env 文件

const APP_KEY = process.env.APP_KEY;
const APP_SECRET = process.env.APP_SECRET;
const ENCRYPT_KEY = process.env.ENCRYPT_KEY;
const GATEWAY_URL = process.env.GATEWAY_URL || 'http://localhost:8080';
const CLIENT_ID = process.env.CLIENT_ID || 'nodejs-demo-client';

if (!APP_KEY || !APP_SECRET || !ENCRYPT_KEY) {
    console.error('❌ [Error] 缺少必要配置：请确保 .env 文件已正确配置 APP_KEY, APP_SECRET, ENCRYPT_KEY');
    process.exit(1);
}

// 1. 初始化分发器
const dispatcher = new MessageDispatcher();

// 注册处理器
dispatcher.register('TEST_DATA', (msg) => {
    console.log('✅ [Demo] 收到业务消息:', msg);
    return true;
});

dispatcher.onAppTicket((msg) => {
    if (msg.bizContent && msg.bizContent.appTicket) {
        console.log(`🎫 [Demo] 收到应用票据 (AppTicket) [msgId: ${msg.id}]:`, msg.bizContent.appTicket);
    } else {
        console.warn('⚠️ [Demo] 收到 APP_TICKET 消息，但未在 bizContent 中找到 appTicket 字段。', JSON.stringify(msg));
    }
    return true;
});

dispatcher.onEntAuthCode((msg) => {
    if (msg.bizContent && msg.bizContent.tempAuthCode) {
        console.log('🔑 [Demo] 收到临时授权码 (tempAuthCode):', msg.bizContent.tempAuthCode);
        console.log('🏳️  [Demo] 携带的 State:', msg.bizContent.state || 'none');
    } else {
        console.warn('⚠️ [Demo] 收到 TEMP_AUTH_CODE 消息，但未在 bizContent 中找到 tempAuthCode。', JSON.stringify(msg));
    }
    return true;
});

dispatcher.onOrderStatus((msg) => {
    if (msg.bizContent) {
        const orderNo = msg.bizContent.orderNo;
        const payTotal = msg.bizContent.detail ? msg.bizContent.detail.payTotal : 'unknown';
        console.log(`💰 [Demo] 收到订单支付成功消息: 订单号=${orderNo}, 实付金额=${payTotal}`);
        if (msg.bizContent.detail && msg.bizContent.detail.orderItems) {
            console.log(`📦 包含商品数: ${msg.bizContent.detail.orderItems.length}`);
        }
    } else {
        console.warn('⚠️ [Demo] 收到 PAY_ORDER_SUCCESS 消息，但 bizContent 为空。', JSON.stringify(msg));
    }
    return true;
});

dispatcher.onAppCancelOpen((msg) => {
    if (msg.bizContent) {
        const { appId, orgId, completedTime } = msg.bizContent;
        console.log(`❌ [Demo] 收到应用取消开通消息: AppId=${appId}, OrgId=${orgId}, 完成时间=${completedTime}`);
    } else {
        console.warn('⚠️ [Demo] 收到 APP_CANCEL_OPEN 消息，但 bizContent 为空。', JSON.stringify(msg));
    }
    return true;
});

dispatcher.onEntUnauth((msg) => {
    if (msg.bizContent) {
        const { appId, orgId, completedTime } = msg.bizContent;
        console.log(`🚫 [Demo] 收到解除授权消息 (APP_CANCEL_AUTHORIZATION): AppId=${appId}, OrgId=${orgId}, 完成时间=${completedTime}`);
    } else {
        console.warn('⚠️ [Demo] 收到 APP_CANCEL_AUTHORIZATION 消息，但 bizContent 为空。', JSON.stringify(msg));
    }
    return true;
});

// 2. 初始化客户端并配置分发器 (传入独立的 ENCRYPT_KEY)
const client = new GatewayClient({
    appKey: APP_KEY,
    appSecret: APP_SECRET,
    encryptKey: ENCRYPT_KEY,
    gatewayUrl: GATEWAY_URL,
    clientId: CLIENT_ID
});

client.useDispatcher(dispatcher);

// 3. 启动
console.log('🚀 [Demo] 正在从环境变量启动 Node.js SDK Demo...');
console.log(`📍 AppKey: ${APP_KEY}`);
console.log(`🔑 消息秘钥: ${ENCRYPT_KEY.substring(0, 4)}****`); // 仅打印前几位进行验证
client.start();

process.on('SIGINT', () => {
    console.log('Stopping...');
    client.stop();
    process.exit(0);
});
