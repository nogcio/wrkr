FROM rust:1-alpine AS chef
WORKDIR /app
ENV RUSTFLAGS="-C target-feature=-crt-static"
RUN apk add --no-cache \
		build-base \
		luajit-dev \
		pkgconfig
RUN cargo install cargo-chef --locked

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
COPY . .
RUN cargo build --release -p wrkr --locked

# Runtime stage
FROM alpine:3.20
RUN apk add --no-cache ca-certificates luajit \
	&& addgroup -S wrkr \
	&& adduser -S wrkr -G wrkr

COPY --from=builder /app/target/release/wrkr /usr/local/bin/wrkr
USER wrkr
ENTRYPOINT ["/usr/local/bin/wrkr"]
