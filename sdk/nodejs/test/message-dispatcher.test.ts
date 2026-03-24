import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import * as crypto from 'crypto';
import { MessageDispatcher } from '../src/message-dispatcher.js';
import { EventFrame } from '../src/protocol.js';

describe('MessageDispatcher', () => {
    const secret = '12345678901234567890123456789012';
    let dispatcher: MessageDispatcher;

    beforeEach(() => {
        dispatcher = new MessageDispatcher();
    });

    test('should dispatch plain business message', async () => {
        const handler = jest.fn((_msg: any) => Promise.resolve(true));
        dispatcher.register('TEST_TYPE', handler as any);

        const frame: EventFrame = {
            msg_type: 'event',
            msg_id: '1',
            app_key: 'ak',
            target_client_id: 'c1',
            payload: JSON.stringify({ msgType: 'TEST_TYPE', data: 'hello' }),
            timestamp: Date.now()
        };

        const result = await dispatcher.dispatch(frame, secret);
        expect(result).toBe(true);
        expect(handler).toHaveBeenCalled();
    });

    test('should decrypt and dispatch encrypted message', async () => {
        const handler = jest.fn((_msg: any) => Promise.resolve(true));
        dispatcher.register('SECURE_TYPE', handler as any);

        const businessMsg = JSON.stringify({ msgType: 'SECURE_TYPE', secret_data: 'shhh' });
        
        const encryptKey = '1234567890123456';
        const cipher = crypto.createCipheriv('aes-128-ecb', Buffer.from(encryptKey), null);
        let encrypted = cipher.update(businessMsg, 'utf8', 'base64');
        encrypted += cipher.final('base64');

        const frame: EventFrame = {
            msg_type: 'event',
            msg_id: '2',
            app_key: 'ak',
            target_client_id: 'c1',
            payload: JSON.stringify({ encryptMsg: encrypted }),
            timestamp: Date.now()
        };

        const result = await dispatcher.dispatch(frame, encryptKey);
        expect(result).toBe(true);
        expect(handler).toHaveBeenCalled();
    });

    test('should route APP_NOTICE correctly', async () => {
        const handler = jest.fn((_msg: any) => Promise.resolve(true));
        dispatcher.onAppNotice('Order', handler as any);

        const frame: EventFrame = {
            msg_type: 'event',
            msg_id: '3',
            app_key: 'ak',
            target_client_id: 'c1',
            payload: JSON.stringify({ 
                msgType: 'APP_NOTICE', 
                bizContent: { boName: 'Order', action: 'create' } 
            }),
            timestamp: Date.now()
        };

        const result = await dispatcher.dispatch(frame, secret);
        expect(result).toBe(true);
        expect(handler).toHaveBeenCalled();
    });
});
