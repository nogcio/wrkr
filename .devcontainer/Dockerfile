FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

USER root

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        git \
        linux-tools-common \
        linux-tools-generic \
        pkg-config \
        protobuf-compiler \
        libluajit-5.1-dev \
        tar \
        wrk \
    && rm -rf /var/lib/apt/lists/*
    
USER vscode

ENV PATH=/home/vscode/.cargo/bin:/home/vscode/.local/bin:${PATH}

# Rust toolchain (pinned by rust-toolchain.toml as well)
RUN curl -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain 1.92.0 \
    && rustup component add rustfmt clippy
