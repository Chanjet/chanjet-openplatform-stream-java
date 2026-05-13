# syntax=docker/dockerfile:1
FROM ubuntu:24.04

# 替换为内地的 Ubuntu 镜像源 (可选)
RUN sed -i 's/archive.ubuntu.com/mirrors.tuna.tsinghua.edu.cn/g' /etc/apt/sources.list && \
    sed -i 's/security.ubuntu.com/mirrors.tuna.tsinghua.edu.cn/g' /etc/apt/sources.list

# 安装交叉编译工具链及编译依赖
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y \
    g++-x86-64-linux-gnu \
    libc6-amd64-cross \
    build-essential \
    cmake \
    clang \
    pkg-config \
    libssl-dev \
    libatomic1 \
    python3 \
    python3-aiohttp \
    sqlite3 \
    redis-server \
    default-mysql-client \
    postgresql \
    postgresql-client \
    curl \
    procps \
    lsof \
    perl \
    git \
    && rm -rf /var/lib/apt/lists/*

# 安装 Rust (使用原生 ARM64)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.92.0
ENV PATH="/root/.cargo/bin:${PATH}"

# 设置交叉链接器环境变量
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc

# 添加交叉编译目标
RUN rustup target add x86_64-unknown-linux-gnu

WORKDIR /workspace
