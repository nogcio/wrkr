FROM rust:1-alpine AS chef
WORKDIR /app
ENV RUSTFLAGS="-C target-feature=-crt-static"
RUN apk add --no-cache \
		build-base \
		protobuf \
		luajit-dev \
		pkgconfig
RUN cargo install cargo-chef --locked

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY wrkr/Cargo.toml wrkr/Cargo.toml
COPY wrkr-core/Cargo.toml wrkr-core/Cargo.toml
COPY wrkr-lua/Cargo.toml wrkr-lua/Cargo.toml
COPY wrkr-testserver/Cargo.toml wrkr-testserver/Cargo.toml
COPY wrkr-testserver/build.rs wrkr-testserver/build.rs
COPY wrkr-tools-compare-perf/Cargo.toml wrkr-tools-compare-perf/Cargo.toml
COPY wrkr-value/Cargo.toml wrkr-value/Cargo.toml

# `cargo chef prepare` runs `cargo metadata`, which requires each workspace member
# to have at least one target file present. We create minimal stubs here so we can
# keep the planner input limited to manifests (better cache hit-rate).
RUN mkdir -p \
		wrkr/src \
		wrkr-core/src \
		wrkr-lua/src \
		wrkr-testserver/src \
		wrkr-tools-compare-perf/src \
		wrkr-value/src \
	&& printf 'fn main() {}\n' > wrkr/src/main.rs \
	&& printf 'pub fn _docker_planner_stub() {}\n' > wrkr-core/src/lib.rs \
	&& printf 'pub fn _docker_planner_stub() {}\n' > wrkr-lua/src/lib.rs \
	&& printf 'fn main() {}\n' > wrkr-tools-compare-perf/src/main.rs \
	&& printf 'pub fn _docker_planner_stub() {}\n' > wrkr-value/src/lib.rs \
	&& printf 'pub fn _docker_planner_stub() {}\n' > wrkr-testserver/src/lib.rs \
	&& printf 'fn main() {}\n' > wrkr-testserver/src/main.rs
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ENV CARGO_TARGET_DIR=/app/target-cache
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN --mount=type=cache,target=/usr/local/cargo/registry \
	--mount=type=cache,target=/usr/local/cargo/git \
	--mount=type=cache,target=/app/target-cache \
	cargo chef cook --release --recipe-path recipe.json

# Build application
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
	--mount=type=cache,target=/usr/local/cargo/git \
	--mount=type=cache,target=/app/target-cache \
	cargo build --release -p wrkr --locked \
	&& mkdir -p /app/bin \
	&& cp /app/target-cache/release/wrkr /app/bin/wrkr

# Runtime stage
FROM alpine:3.23
RUN apk add --no-cache ca-certificates luajit protobuf \
	&& addgroup -S wrkr \
	&& adduser -S wrkr -G wrkr

COPY --from=builder /app/bin/wrkr /usr/local/bin/wrkr
USER wrkr
ENTRYPOINT ["/usr/local/bin/wrkr"]
