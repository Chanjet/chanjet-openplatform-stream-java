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
	
	// 这里假设我们在 assets/lib 下按平台存放了文件
	// 例如 assets/lib/darwin-arm64/libonnxruntime.dylib
	libPath := filepath.Join(libDir, runtime.GOOS+"-"+runtime.GOARCH, libName)
	
	// 如果特定架构的库不存在，尝试降级到通用 lib 目录
	if _, err := os.Stat(libPath); os.IsNotExist(err) {
		libPath = filepath.Join(libDir, libName)
	}

	modelPath := filepath.Join(modelDir, "model_quantized.onnx")
	tokenizerPath := filepath.Join(modelDir, "tokenizer.json")

	// 3. 关键：将运行时路径注入 ONNX 库
	ort.SetSharedLibraryPath(libPath)

	return libPath, modelPath, tokenizerPath, nil
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
