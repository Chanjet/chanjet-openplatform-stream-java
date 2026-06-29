package crypto

import (
	"crypto/aes"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"errors"
	"fmt"
	"strings"
	"unicode"
)

// HmacSha256 计算 HMAC-SHA256 签名，返回小写 16 进制字符串
func HmacSha256(data, secret string) string {
	h := hmac.New(sha256.New, []byte(secret))
	h.Write([]byte(data))
	return hex.EncodeToString(h.Sum(nil))
}

// SanitizeKey 清理密钥字符串中的不可见字符
func SanitizeKey(s string) string {
	var builder strings.Builder
	for _, c := range s {
		if c != '\u200B' && c != '\u200C' && c != '\u200D' && c != '\uFEFF' && !unicode.IsControl(c) {
			builder.WriteRune(c)
		}
	}
	return strings.TrimSpace(builder.String())
}

// AesDecrypt 执行 AES-128-ECB 解密
func AesDecrypt(encryptedBase64, decryptKey string) (string, error) {
	decryptKey = SanitizeKey(decryptKey)
	if decryptKey == "" {
		return "", errors.New("invalid decryptKey for AES decryption")
	}

	var key []byte
	if len(decryptKey) == 32 {
		decoded, err := hex.DecodeString(decryptKey)
		if err != nil {
			return "", fmt.Errorf("failed to decode 32-character decryption key as hex: %w", err)
		}
		key = decoded
	} else {
		key = []byte(decryptKey)
	}

	if len(key) != 16 {
		return "", fmt.Errorf("AES-128 key must be 16 bytes (or 32 hex characters), got %d", len(key))
	}
	ciphertext, err := base64.StdEncoding.DecodeString(encryptedBase64)
	if err != nil {
		return "", fmt.Errorf("base64 decode failed: %w", err)
	}

	block, err := aes.NewCipher(key)
	if err != nil {
		return "", fmt.Errorf("create aes cipher failed: %w", err)
	}

	bs := block.BlockSize()
	if len(ciphertext)%bs != 0 {
		return "", errors.New("ciphertext is not a multiple of the block size")
	}

	plaintext := make([]byte, len(ciphertext))
	for i := 0; i < len(ciphertext); i += bs {
		block.Decrypt(plaintext[i:i+bs], ciphertext[i:i+bs])
	}

	// 移除 PKCS5/PKCS7 填充
	plaintext, err = pkcs7Unpad(plaintext, bs)
	if err != nil {
		return "", fmt.Errorf("unpad failed: %w", err)
	}

	return string(plaintext), nil
}

func pkcs7Unpad(data []byte, blockSize int) ([]byte, error) {
	length := len(data)
	if length == 0 {
		return nil, errors.New("unpad error: data is empty")
	}
	unpadding := int(data[length-1])
	if unpadding > blockSize || unpadding == 0 {
		return nil, errors.New("unpad error: invalid padding size")
	}
	// 验证填充的字节是否正确
	for i := 0; i < unpadding; i++ {
		if int(data[length-1-i]) != unpadding {
			return nil, errors.New("unpad error: invalid padding content")
		}
	}
	return data[:(length - unpadding)], nil
}
