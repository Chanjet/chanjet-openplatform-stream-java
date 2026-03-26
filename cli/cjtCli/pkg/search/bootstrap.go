package search

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"runtime"

	ort "github.com/yalue/onnxruntime_go"
)

const (
	ModelBaseURL = "https://github.com/chanjet-open/models/releases/download/v1.0.0" // 示例地址
)

// EnsureEnvironmentReady 自动检测并从二进制中释放所需的 AI 运行时与模型
func EnsureEnvironmentReady() (string, string, string, error) {
	home, _ := os.UserHomeDir()
	baseDir := filepath.Join(home, ".cjtCli")
	libDir := filepath.Join(baseDir, "lib")
	modelDir := filepath.Join(baseDir, "models")

	// 1. 从二进制嵌入文件系统释放资源
	if err := ExtractEmbeddedResources(libDir, modelDir); err != nil {
		return "", "", "", fmt.Errorf("释放嵌入资源失败: %w", err)
	}

	// 2. 确定动态库文件名
	var libName string
	switch runtime.GOOS {
	case "darwin":
		libName = "libonnxruntime.dylib"
	case "windows":
		libName = "onnxruntime.dll"
	default:
		libName = "libonnxruntime.so"
	}
	
	// 架构特定的库路径
	libPath := filepath.Join(libDir, runtime.GOOS+"-"+runtime.GOARCH, libName)
	if _, err := os.Stat(libPath); os.IsNotExist(err) {
		libPath = filepath.Join(libDir, libName)
	}

	// 校验资产文件大小是否合法 (假定最小有效模型 > 1KB, tokenizer > 1KB)
	modelPath := filepath.Join(modelDir, "model_quantized.onnx")
	tokenizerPath := filepath.Join(modelDir, "tokenizer.json")

	if !isAssetValid(libPath, 1024*1024) || !isAssetValid(modelPath, 1024) || !isAssetValid(tokenizerPath, 1024) {
		return "", "", "", fmt.Errorf("AI 资产不完整或已损坏 (File size check failed)")
	}

	// 3. 关键：将运行时路径注入 ONNX 库
	ort.SetSharedLibraryPath(libPath)

	return libPath, modelPath, tokenizerPath, nil
}

func isAssetValid(path string, minSize int64) bool {
	info, err := os.Stat(path)
	if err != nil {
		return false
	}
	return info.Size() >= minSize
}

func downloadFile(filepath string, url string) error {
	resp, err := http.Get(url)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("服务器返回错误: %s", resp.Status)
	}

	out, err := os.Create(filepath)
	if err != nil {
		return err
	}
	defer out.Close()

	_, err = io.Copy(out, resp.Body)
	return err
}
