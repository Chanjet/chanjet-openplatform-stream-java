import WebSocket from 'ws';
import os from 'os';
import { CryptoUtils } from './crypto-utils.js';
import { EventFrame, AckFrame } from './protocol.js';
import { MessageDispatcher } from './message-dispatcher.js';
import { DlqProvider } from './dlq.js';

export interface GatewayClientOptions {
    appKey: string;
    appSecret: string;
    encryptKey?: string;
    gatewayUrl?: string;
    dlqProvider?: DlqProvider;
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
    private dlqProvider: DlqProvider | null = null;
    private reconnectTimer: NodeJS.Timeout | null = null;
    private heartbeatTimer: NodeJS.Timeout | null = null;

    constructor(options: GatewayClientOptions) {
        this.appKey = options.appKey;
        this.appSecret = options.appSecret;
        this.encryptKey = options.encryptKey || options.appSecret;
        this.gatewayUrl = options.gatewayUrl || 'https://stream-open.chanapp.chanjet.com';
        this.dlqProvider = options.dlqProvider || null;
        
        // 自动生成唯一 ClientId: appKey@hostname_pid_random
        const hostname = os.hostname();
        const pid = process.pid;
        const random = Math.random().toString(36).substring(2, 8);
        this.clientId = `${this.appKey}@${hostname}_${pid}_${random}`;
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
        if (this.heartbeatTimer) {
            clearTimeout(this.heartbeatTimer);
            this.heartbeatTimer = null;
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
                this.resetHeartbeat();
            });

            this.ws.on('message', async (data) => {
                this.resetHeartbeat();
                try {
                    const text = data.toString();
                    const root = JSON.parse(text);
                    const msgType = root.msg_type || root.msgType;

                    if (msgType === 'event') {
                        const frame = root as EventFrame;
                        let success = false;
                        let dlqStored = false;
                        
                        if (this.messageDispatcher) {
                            try {
                                success = await this.messageDispatcher.dispatch(frame, this.encryptKey);
                            } catch (e: any) {
                                console.error(`[GatewayClient] Event dispatch exception:`, e);
                                success = false;
                            }
                        } else if (this.eventHandler) {
                            try {
                                success = await this.eventHandler(frame);
                            } catch (e: any) {
                                console.error(`[GatewayClient] EventHandler exception:`, e);
                                success = false;
                            }
                        }

                        if (!success && this.dlqProvider) {
                            try {
                                await this.dlqProvider.store(frame.msg_id, text);
                                dlqStored = true;
                            } catch (e) {
                                console.error(`[GatewayClient] Failed to store message to DLQ:`, e);
                                // If DLQ fails, we must send 500 to let server retry
                                this.sendAck(frame.msg_id, false, `DLQ store failed: ${(e as Error).message}`);
                                return;
                            }
                        }

                        if (success && dlqStored && this.dlqProvider) {
                            try {
                                await this.dlqProvider.remove(frame.msg_id);
                            } catch (e) {
                                console.error(`[GatewayClient] Failed to remove message from DLQ:`, e);
                            }
                        }
                        
                        this.sendAck(frame.msg_id, success || dlqStored);
                    } else if (msgType === 'ping') {
                        this.ws?.send(JSON.stringify({ msg_type: 'pong' }));
                    } else if (msgType) {
                        // Top-level system messages
                        const msgId = root.msg_id || root.msgId || root.id || 'unknown';
                        let success = false;
                        if (this.messageDispatcher) {
                            try {
                                success = await this.messageDispatcher.dispatchValue(root);
                            } catch (e: any) {
                                console.error(`[GatewayClient] System message dispatch exception:`, e);
                                success = false;
                            }
                        }
                        if (msgId !== 'unknown') {
                            this.sendAck(msgId, success);
                        }
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

    private resetHeartbeat() {
        if (this.heartbeatTimer) clearTimeout(this.heartbeatTimer);
        this.heartbeatTimer = setTimeout(() => {
            console.warn('[GatewayClient] Heartbeat timeout. No messages received for 30s. Triggering reconnect.');
            if (this.ws) {
                this.ws.terminate();
            }
        }, 30000);
    }

    private sendAck(msgId: string, success: boolean, customMessage?: string) {
        if (!this.ws || !this.connected) return;
        const ack: AckFrame = {
            msg_id: msgId,
            code: success ? 200 : 500,
            message: customMessage || (success ? 'success' : 'failed'),
            timestamp: Date.now()
        };
        this.ws.send(JSON.stringify(ack));
    }
}
