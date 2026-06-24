# syntax=docker/dockerfile:1
FROM ubuntu:24.04

# 替换为内地的 Ubuntu 镜像源 (适用于 Ubuntu 24.04 DEB822 格式)
RUN sed -i 's/ports.ubuntu.com\/ubuntu-ports/mirrors.tuna.tsinghua.edu.cn\/ubuntu-ports/g' /etc/apt/sources.list.d/ubuntu.sources

# 启用多架构支持并配置正确的镜像源
RUN dpkg --add-architecture amd64 && \
    sed -i 's/Types: deb/Types: deb\nArchitectures: arm64/g' /etc/apt/sources.list.d/ubuntu.sources && \
    echo "Types: deb\nArchitectures: amd64\nURIs: http://mirrors.tuna.tsinghua.edu.cn/ubuntu/\nSuites: noble noble-updates noble-backports noble-security\nComponents: main universe restricted multiverse\nSigned-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg" > /etc/apt/sources.list.d/ubuntu-amd64.sources

# 安装交叉编译工具链及编译依赖
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y \
    g++-x86-64-linux-gnu \
    libc6-amd64-cross \
    build-essential \
    cmake \
    clang \
    pkg-config \
    libssl-dev \
    libssl-dev:amd64 \
    libatomic1 \
    python3 \
    python3-aiohttp \
    sqlite3 \
    redis-server \
    default-mysql-server \
    default-mysql-client \
    postgresql \
    postgresql-client \
    curl \
    procps \
    lsof \
    perl \
    git \
    wine64:amd64 \
    wine:amd64 \
    nsis \
    zip \
    mingw-w64 \
    python3-pip \
    && rm -rf /var/lib/apt/lists/* \
    && ln -s /usr/lib/wine/wine64 /usr/bin/wine64 || true

# 安装 Zig 和 cargo-zigbuild 用于 Windows 交叉编译
RUN pip3 install --break-system-packages ziglang cargo-zigbuild

# 预配置 MySQL 和 PostgreSQL 数据目录
RUN mkdir -p /run/mysqld && chown mysql:mysql /run/mysqld && \
    mkdir -p /var/run/postgresql && chown postgres:postgres /var/run/postgresql

# 安装 Rust (使用原生 ARM64/AMD64)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.92.0 && \
    /root/.cargo/bin/rustup component add llvm-tools-preview
ENV PATH="/root/.cargo/bin:${PATH}"

# 安装 cargo-llvm-cov (自动区分架构)
RUN ARCH=$(uname -m) && \
    if [ "$ARCH" = "aarch64" ]; then DL_ARCH="aarch64"; else DL_ARCH="x86_64"; fi && \
    curl -LsSf "https://github.com/taiki-e/cargo-llvm-cov/releases/latest/download/cargo-llvm-cov-${DL_ARCH}-unknown-linux-gnu.tar.gz" | tar xzf - -C /root/.cargo/bin/

# 设置交叉链接器环境变量
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc
ENV PKG_CONFIG_PATH_x86_64_unknown_linux_gnu=/usr/lib/x86_64-linux-gnu/pkgconfig
ENV PKG_CONFIG_ALLOW_CROSS_x86_64_unknown_linux_gnu=1
ENV OPENSSL_DIR=/usr
ENV OPENSSL_INCLUDE_DIR=/usr/include/x86_64-linux-gnu
ENV OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu

# 添加交叉编译目标
RUN rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-gnu

WORKDIR /workspace

