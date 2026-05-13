# syntax=docker/dockerfile:1
FROM ubuntu:24.04

# 替换为内地的 Ubuntu 镜像源 (可选)
# RUN sed -i 's/archive.ubuntu.com/mirrors.tuna.tsinghua.edu.cn/g' /etc/apt/sources.list && \
#     sed -i 's/security.ubuntu.com/mirrors.tuna.tsinghua.edu.cn/g' /etc/apt/sources.list

# 安装编译依赖
RUN apt-get update && apt-get install -y \
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
    postgresql-client \
    curl \
    procps \
    lsof \
    perl \
    git \
    && rm -rf /var/lib/apt/lists/*

# 安装 Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# 添加编译目标
RUN rustup target add x86_64-unknown-linux-gnu
