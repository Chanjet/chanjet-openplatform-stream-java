# --- 本地开发编译提速优化 (Local Fast Build Optimizations) ---
# 如果不在 CI 环境下，且没有显式禁用，并且当前执行的命令中不包含 'package'，默认关闭 LTO 并恢复多线程代码生成以大幅提升本地编译速度
ifndef CI
ifndef DISABLE_FAST_BUILD
ifeq ($(findstring package,$(filter-out %-fast,$(MAKECMDGOALS))),)
    export CARGO_PROFILE_RELEASE_LTO ?= false
    export CARGO_PROFILE_RELEASE_CODEGEN_UNITS ?= 256
    
    # 自动探测 sccache 并启用
    SCCACHE_EXISTS := $(shell command -v sccache 2> /dev/null)
    ifdef SCCACHE_EXISTS
        export RUSTC_WRAPPER = sccache
    endif
endif
endif
endif

# --- 跨平台兼容性宏 ---
ifeq ($(OS),Windows_NT)
    RM = powershell -NoProfile -Command "$(foreach d,$(1),if(Test-Path $(d)){ Remove-Item -Recurse -Force $(d) };)"
    MD5 = powershell -NoProfile -Command "$$hash = (Get-FileHash -Algorithm MD5 $(1)).Hash.ToLower(); Set-Content -Path $(1).md5 -Value $$hash"
    SHA256 = powershell -NoProfile -Command "$$hash = (Get-FileHash -Algorithm SHA256 $(1)).Hash.ToLower(); Set-Content -Path $(1).sha256 -Value $$hash"
else
    RM = rm -rf $(1)
    MD5 = (md5 -q $(1) 2>/dev/null || md5sum $(1) | cut -d ' ' -f 1) > $(1).md5
    SHA256 = (shasum -a 256 $(1) 2>/dev/null || sha256sum $(1)) | cut -d ' ' -f 1 > $(1).sha256
endif

VERIFY_BIN = ./crates/app/cowen-cli/tests/e2e/scripts/verify-binary.sh $(1)

# 生产/测试环境配置
PROD_OPENAPI = https://openapi.chanjet.com
PROD_STREAM  = https://stream-open.chanapp.chanjet.com
TEST_OPENAPI = https://openapi.inte.chanjet.com
TEST_STREAM  = https://stream-open-chanapp.inte.chanjet.com

.PHONY: build macos-aarch64 linux-x86_64 linux-x86_64-with-docker linux-aarch64 windows-x86_64 windows-x86-cross windows-x86_64-cross \
        windows-plugin-x64 windows-plugin-x86 test-macos-aarch64 \
        test test-linux test-win test-macos check-cross \
        package package-macos-aarch64 package-linux-x86_64 package-linux-x86_64-with-docker \
        package-linux-aarch64 package-windows-x86_64 package-windows-x86_64-cross clean install run prepare-docker-image prepare-docker-machine stop-docker-machine

