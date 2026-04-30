import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import * as crypto from 'crypto';
import { CryptoUtils } from '../src/crypto-utils.js';

describe('CryptoUtils', () => {
    const secret = '<DUMMY_SECRET_32>'; // 32 chars

    test('hmacSha256 should match expected hex', () => {
        const data = 'test-data';
        const sign = CryptoUtils.hmacSha256(data, secret);
        expect(sign).toHaveLength(64);
        expect(sign).toMatch(/^[0-9a-f]+$/);
    });

    test('aesDecrypt should decrypt valid payload', () => {
        const encryptKey = '<DUMMY_KEY_16>';
        const cipher = crypto.createCipheriv('aes-128-ecb', Buffer.from(encryptKey), null);
        let enc = cipher.update('{"hello":"world"}', 'utf8', 'base64');
        enc += cipher.final('base64');

        const decrypted = CryptoUtils.aesDecrypt(enc, encryptKey);
        expect(JSON.parse(decrypted)).toEqual({ hello: 'world' });
    });
});
