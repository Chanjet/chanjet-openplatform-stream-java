import { GatewayClient, MessageDispatcher } from './dist/index.js';

// 1. 初始化客户端
const client = new GatewayClient({
    appKey: 'your_app_key',
    appSecret: 'your_app_secret_32_chars_long',
    // gatewayUrl: 'http://localhost:8080' // 可选，默认连接至畅捷通开放平台生产环境
});

// 2. 初始化分发器
const dispatcher = new MessageDispatcher();

// 注册业务处理器
dispatcher.onAppTicket((msg) => {
    console.log('收到应用票据:', msg.appTicket);
    return true;
});

dispatcher.onAppNotice('GoodsIssue', (msg) => {
    console.log('收到好系列销货单通知:', msg.bizContent);
    return true;
});

// 3. 关联分发逻辑
client.onEvent(async (event) => {
    return await dispatcher.dispatch(event, 'your_app_secret_32_chars_long');
});

// 4. 启动
client.start();

process.on('SIGINT', () => {
    client.stop();
    process.exit(0);
});
