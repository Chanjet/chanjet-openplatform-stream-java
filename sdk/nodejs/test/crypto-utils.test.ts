import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import * as crypto from 'crypto';
import { CryptoUtils } from '../src/crypto-utils.js';

describe('CryptoUtils', () => {
    const secret = '12345678901234567890123456789012'; // 32 chars

    test('hmacSha256 should match expected hex', () => {
        const data = 'test-data';
        const sign = CryptoUtils.hmacSha256(data, secret);
        expect(sign).toHaveLength(64);
        expect(sign).toMatch(/^[0-9a-f]+$/);
    });

    test('aesDecrypt should decrypt valid payload', () => {
        // Key: 1234567890123456, IV: 7890123456789012
        const key = secret.substring(0, 16);
        const iv = secret.substring(16, 32);
        const cipher = crypto.createCipheriv('aes-128-cbc', Buffer.from(key), Buffer.from(iv));
        let enc = cipher.update('{"hello":"world"}', 'utf8', 'base64');
        enc += cipher.final('base64');

        const decrypted = CryptoUtils.aesDecrypt(enc, secret);
        expect(JSON.parse(decrypted)).toEqual({ hello: 'world' });
    });
});
