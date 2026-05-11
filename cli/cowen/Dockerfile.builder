# syntax=docker/dockerfile:1
FROM docker.io/library/rust:1.81-bullseye

# 替换为大陆镜像源 (清华大学 TUNA) 以加速下载
# Bullseye 使用传统的 sources.list 格式
RUN sed -i 's/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/g' /etc/apt/sources.list 2>/dev/null || true; \
    sed -i 's/security.debian.org/mirrors.tuna.tsinghua.edu.cn/g' /etc/apt/sources.list 2>/dev/null || true

# 使用缓存挂载加速 apt 安装
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y \
    g++-x86-64-linux-gnu \
    libc6-amd64-cross \
    python3 \
    python3-aiohttp \
    sqlite3 \
    redis-server \
    default-mysql-client \
    postgresql-client \
    curl \
    procps \
    lsof \
    perl

# 独立层级：添加编译目标
RUN rustup target add x86_64-unknown-linux-gnu
