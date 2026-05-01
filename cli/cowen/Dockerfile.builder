# syntax=docker/dockerfile:1
FROM docker.io/library/rust:latest

# 安装交叉编译工具链及测试所需的系统依赖
RUN apt-get update && apt-get install -y \
    g++-x86-64-linux-gnu \
    libc6-amd64-cross \
    python3 \
    python3-aiohttp \
    sqlite3 \
    redis-server \
    curl \
    procps \
    lsof \
    perl \
    && rm -rf /var/lib/apt/lists/*

# 添加编译目标
RUN rustup target add x86_64-unknown-linux-gnu
