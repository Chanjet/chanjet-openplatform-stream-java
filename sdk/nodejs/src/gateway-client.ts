import WebSocket from 'ws';
import os from 'os';
import { CryptoUtils } from './crypto-utils.js';
import { EventFrame, AckFrame } from './protocol.js';
import { MessageDispatcher } from './message-dispatcher.js';

export interface GatewayClientOptions {
    appKey: string;
    appSecret: string;
    encryptKey?: string;
    gatewayUrl: string;
    clientId?: string;
    reconnectOptions?: {
        maxDelay?: number;
        initialDelay?: number;
    };
}

export type EventHandler = (event: EventFrame) => Promise<boolean> | boolean;

/**
 * 畅捷通 Stream Gateway 客户端 SDK。
 */
export class GatewayClient {
    private readonly appKey: string;
    private readonly appSecret: string;
    private readonly encryptKey: string;
    private readonly gatewayUrl: string;
    private readonly clientId: string;
    
    private ws: WebSocket | null = null;
    private running = false;
    private connected = false;
    private attempt = 0;
    private eventHandler: EventHandler | null = null;
    private messageDispatcher: MessageDispatcher | null = null;
    private reconnectTimer: NodeJS.Timeout | null = null;

    constructor(options: GatewayClientOptions) {
        this.appKey = options.appKey;
        this.appSecret = options.appSecret;
        this.encryptKey = options.encryptKey || options.appSecret;
        this.gatewayUrl = options.gatewayUrl;
        this.clientId = options.clientId || `${this.appKey}@${os.hostname()}`;
    }

    onEvent(handler: EventHandler) {
        this.eventHandler = handler;
    }

    /**
     * 配置业务消息分发器。
     */
    useDispatcher(dispatcher: MessageDispatcher) {
        this.messageDispatcher = dispatcher;
    }

    async start() {
        if (this.running) return;
        this.running = true;
        await this.connect();
    }

    stop() {
        this.running = false;
        this.connected = false;
        if (this.reconnectTimer) {
            clearTimeout(this.reconnectTimer);
            this.reconnectTimer = null;
        }
        if (this.ws) {
            this.ws.close(1000, 'SDK Stop');
            this.ws = null;
        }
    }

    isConnected() {
        return this.connected;
    }

    private async connect() {
        if (!this.running) return;

        try {
            console.log(`[GatewayClient] Attempting to connect (Attempt: ${this.attempt + 1})...`);
            
            const nonce = await this.fetchNonce();
            if (!nonce) return;

            const sign = CryptoUtils.hmacSha256(`${this.appKey}&${nonce}`, this.appSecret);
            const connectUrl = this.gatewayUrl.replace(/^http/, 'ws') + 
                `/connect?app_key=${this.appKey}&nonce=${nonce}&sign=${sign}&client_id=${this.clientId}`;

            this.ws = new WebSocket(connectUrl, { handshakeTimeout: 10000 });

            this.ws.on('open', () => {
                console.log('[GatewayClient] WebSocket connected.');
                this.connected = true;
                this.attempt = 0;
            });

            this.ws.on('message', async (data) => {
                try {
                    const text = data.toString();
                    const root = JSON.parse(text);
                    const msgType = root.msg_type;

                    if (msgType === 'event') {
                        const frame = root as EventFrame;
                        let success = false;
                        
                        if (this.messageDispatcher) {
                            success = await this.messageDispatcher.dispatch(frame, this.encryptKey);
                        } else if (this.eventHandler) {
                            success = await this.eventHandler(frame);
                        }
                        
                        this.sendAck(frame.msg_id, success);
                    } else if (msgType === 'ping') {
                        this.ws?.send(JSON.stringify({ msg_type: 'pong' }));
                    }
                } catch (e) {
                    console.error('[GatewayClient] Error processing message:', e);
                }
            });

            this.ws.on('close', (code, reason) => {
                console.warn(`[GatewayClient] WebSocket closed: ${code} - ${reason}`);
                this.connected = false;
                if (this.running) {
                    this.handleReconnect(code === 1008 ? 403 : 503);
                }
            });

            this.ws.on('error', (err) => {
                console.error('[GatewayClient] WebSocket error:', err.message);
                this.connected = false;
                if (this.running) {
                    this.handleReconnect(503);
                }
            });

        } catch (e: any) {
            console.error('[GatewayClient] Connection failed:', e.message);
            this.handleReconnect(503);
        }
    }

    private async fetchNonce(): Promise<string | null> {
        try {
            const url = this.gatewayUrl.replace(/^ws/, 'http') + `/v1/ws/challenge?app_key=${this.appKey}`;
            const signPrefix = CryptoUtils.hmacSha256(this.appKey, this.appSecret).substring(0, 16);
            
            const response = await fetch(url, {
                headers: { 'X-CJT-PreAuth': signPrefix }
            });

            if (response.ok) {
                const body = await response.json();
                return body.data.nonce;
            } else {
                console.warn(`[GatewayClient] Fetch nonce failed: ${response.status}`);
                this.handleReconnect(response.status);
                return null;
            }
        } catch (e: any) {
            console.error('[GatewayClient] Error fetching nonce:', e.message);
            this.handleReconnect(503);
            return null;
        }
    }

    private handleReconnect(statusCode: number) {
        if (!this.running) return;

        let delay: number;
        if (statusCode === 503 || statusCode === 429) {
            delay = 5000 + Math.random() * 10000;
            console.log(`[GatewayClient] Gateway busy (${statusCode}), standby mode. Reconnect in ${Math.round(delay)}ms`);
        } else if (statusCode === 401 || statusCode === 403) {
            console.error(`[GatewayClient] Auth failed (${statusCode}). Stopping.`);
            this.running = false;
            return;
        } else {
            delay = Math.min(60000, 1000 * Math.pow(2, this.attempt++));
            console.log(`[GatewayClient] Connection failed (${statusCode}), backoff mode. Reconnect in ${Math.round(delay)}ms`);
        }

        if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
        this.reconnectTimer = setTimeout(() => this.connect(), delay);
    }

    private sendAck(msgId: string, success: boolean) {
        if (!this.ws || !this.connected) return;
        const ack: AckFrame = {
            msg_id: msgId,
            code: success ? 200 : 500,
            message: success ? 'success' : 'failed',
            timestamp: Date.now()
        };
        this.ws.send(JSON.stringify(ack));
    }
}
