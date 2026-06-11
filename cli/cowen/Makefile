BINARY=cowen
PKG=cli/cowen
OUTPUT_DIR=../../bin

# 从 Cargo.toml 自动提取版本号
ifeq ($(OS),Windows_NT)
    VERSION := $(shell powershell -NoProfile -Command "(Get-Content crates/app/cowen-cli/Cargo.toml | Select-String '^version =').ToString().Split('\"')[1].Trim()")
else
    VERSION := $(shell grep '^version =' crates/app/cowen-cli/Cargo.toml | cut -d '"' -f 2)
endif

# 缓存目录与镜像定义
CARGO_CACHE_DIR=$(HOME)/.cache/cowen-cargo
BUILDER_IMAGE=localhost/cowen-builder
MAX_PARALLEL ?= 1

# 检测当前主机环境
ifeq ($(OS),Windows_NT)
    HOST_OS := windows
    HOST_ARCH := x86_64
else
    UNAME_S := $(shell uname -s)
    UNAME_M := $(shell uname -m)
    ifeq ($(UNAME_S),Darwin)
        HOST_OS := macos
    else
        HOST_OS := linux
    endif
    ifeq ($(UNAME_M),x86_64)
        HOST_ARCH := x86_64
    else ifeq ($(UNAME_M),arm64)
        HOST_ARCH := aarch64
    else ifeq ($(UNAME_M),aarch64)
        HOST_ARCH := aarch64
    else
        HOST_ARCH := $(UNAME_M)
    endif
endif
# --- 默认构建目标 (Default Build Target) ---
build:
ifeq ($(HOST_OS),macos)
	@$(MAKE) macos-aarch64
else ifeq ($(HOST_OS),linux)
ifeq ($(HOST_ARCH),x86_64)
	@$(MAKE) linux-x86_64
else
	@$(MAKE) linux-aarch64
endif
else ifeq ($(HOST_OS),windows)
	@$(MAKE) windows-x86_64
endif

# --- 默认打包目标 (Default Package Target) ---
package:
ifeq ($(HOST_OS),macos)
	@$(MAKE) package-macos-aarch64
else ifeq ($(HOST_OS),linux)
ifeq ($(HOST_ARCH),x86_64)
	@$(MAKE) package-linux-x86_64
else
	@$(MAKE) package-linux-aarch64
endif
else ifeq ($(HOST_OS),windows)
	@$(MAKE) package-windows-x86_64
endif

# 授权配置 (Authentication)
OFFICIAL_APP_KEY = 3NWdEbmu    # 正式环境 AppKey (Production)
PREVIEW_APP_KEY  ?= RjP3LPoI   # 预览/集成测试环境 AppKey (Preview/INTE)


# --- Include Modular Makefiles ---
include make/macros.mk
include make/build.mk
include make/package.mk
include make/test.mk
include make/db.mk
