package crypto

import (
	"crypto/aes"
	"encoding/base64"
	"testing"
)

func TestHmacSha256(t *testing.T) {
	data := "test-data"
	secret := "test-secret"
	res := HmacSha256(data, secret)
	if len(res) != 64 {
		t.Errorf("Expected length 64, got %d", len(res))
	}
}

func TestAesDecrypt(t *testing.T) {
	key := "<DUMMY_KEY_0016>" // 16 bytes
	plaintext := `{"hello":"world"}`
	
	// 手动构造加密数据以验证解密
	block, err := aes.NewCipher([]byte(key))
	if err != nil {
		t.Fatalf("aes.NewCipher failed: %v", err)
	}
	bs := block.BlockSize()
	padded := pkcs7Pad([]byte(plaintext), bs)
	ciphertext := make([]byte, len(padded))
	for i := 0; i < len(padded); i += bs {
		block.Encrypt(ciphertext[i:i+bs], padded[i:i+bs])
	}
	encBase64 := base64.StdEncoding.EncodeToString(ciphertext)

	decrypted, err := AesDecrypt(encBase64, key)
	if err != nil {
		t.Fatalf("Decrypt failed: %v", err)
	}
	if decrypted != plaintext {
		t.Errorf("Expected %s, got %s", plaintext, decrypted)
	}
}

func pkcs7Pad(data []byte, blockSize int) []byte {
	padding := blockSize - len(data)%blockSize
	padtext := make([]byte, padding)
	for i := range padtext {
		padtext[i] = byte(padding)
	}
	return append(data, padtext...)
}
