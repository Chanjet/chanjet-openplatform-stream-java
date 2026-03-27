package search

import (
	"embed"
	"io"
	"os"
	"path/filepath"
)

//go:embed assets/models/* assets/lib/darwin-arm64/*
var embeddedFiles embed.FS

// ExtractEmbeddedResources 将嵌入在二进制中的资源释放到本地磁盘
func ExtractEmbeddedResources(targetLibDir, targetModelDir string) error {
	// 1. 递归释放 assets 目录下的所有内容
	return walkAndExtract("assets", filepath.Dir(targetLibDir)) // 释放到 baseDir
}

func walkAndExtract(srcPath, targetBase string) error {
	entries, err := embeddedFiles.ReadDir(srcPath)
	if err != nil {
		return err
	}

	for _, entry := range entries {
		currSrcPath := filepath.Join(srcPath, entry.Name())
		// 计算目标路径，移除 "assets/" 前缀
		relPath, _ := filepath.Rel("assets", currSrcPath)
		currDstPath := filepath.Join(targetBase, relPath)

		if entry.IsDir() {
			if err := os.MkdirAll(currDstPath, 0755); err != nil {
				return err
			}
			if err := walkAndExtract(currSrcPath, targetBase); err != nil {
				return err
			}
		} else {
			if err := extractFile(currSrcPath, currDstPath); err != nil {
				return err
			}
		}
	}
	return nil
}

func extractFile(srcPath, dstPath string) error {
	// 如果文件已存在，跳过释放（除非是强制更新）
	if _, err := os.Stat(dstPath); err == nil {
		return nil
	}

	if err := os.MkdirAll(filepath.Dir(dstPath), 0755); err != nil {
		return err
	}

	srcFile, err := embeddedFiles.Open(srcPath)
	if err != nil {
		return err
	}
	defer srcFile.Close()

	dstFile, err := os.OpenFile(dstPath, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0755)
	if err != nil {
		return err
	}
	defer dstFile.Close()

	_, err = io.Copy(dstFile, srcFile)
	return err
}
