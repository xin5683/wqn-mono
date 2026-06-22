# Backend image for Wrong-Question-Notebook (Rust / Axum).
#
# Static musl build → tiny Alpine runtime image.
#
#   Build:  docker build -t wqn-backend .
#   Run:    docker run --rm -p 8080:8080 --env-file .env wqn-backend
#
# The crate embeds its SQLx migrations at compile time (sqlx::migrate!()), so
# neither a migrations directory nor a DATABASE_URL is required to build the
# image — migrations run automatically on startup against the configured DB.
#
# China-network friendly: Alpine apk uses the TUNA mirror, crates.io is proxied
# via the TUNA sparse mirror (.cargo/config.toml), and the utoipa-swagger-ui
# GitHub asset download is routed through a GitHub proxy.

# ---- chef: alpine base with cargo-chef for dependency caching ---------------
# ring (rustls / jsonwebtoken) needs perl + a C toolchain for its asm build;
# binutils provides `strip` to shrink the final binary;
# curl is needed by utoipa-swagger-ui's build script (downloads Swagger UI).
FROM rust:1-alpine AS chef
# Point Alpine's package manager at the TUNA mirror (default CDN is slow/flaky
# in China). Overridable via build arg.
ARG APK_MIRROR=mirrors.tuna.tsinghua.edu.cn
RUN if [ -n "$APK_MIRROR" ]; then \
      sed -i "s#dl-cdn.alpinelinux.org#$APK_MIRROR#g" /etc/apk/repositories; \
    fi \
 && apk add --no-cache perl make binutils curl
# Apply the crates.io mirror (committed in .cargo/config.toml) to CARGO_HOME so
# that cargo-chef install AND every later crate download (chef cook, build) use
# it — direct crates.io is slow/unreachable in some networks.
COPY .cargo/config.toml /usr/local/cargo/config.toml
# utoipa-swagger-ui's build script downloads Swagger UI from GitHub at build
# time; route it through a GitHub proxy (direct github.com is unreachable in
# China). The build script reads SWAGGER_UI_DOWNLOAD_URL.
ENV SWAGGER_UI_DOWNLOAD_URL=https://gh-proxy.com/https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip
RUN cargo install cargo-chef --locked
WORKDIR /app

# ---- planner: derive the dependency recipe from Cargo.toml / Cargo.lock ------
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- builder: cook dependencies first, then build the static binary ----------
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin wrong_question_notebook_rs \
 && strip target/release/wrong_question_notebook_rs

# ---- runtime: minimal Alpine image with a fully static binary ----------------
FROM alpine:3.20 AS runtime
ARG APK_MIRROR=mirrors.tuna.tsinghua.edu.cn
RUN if [ -n "$APK_MIRROR" ]; then \
      sed -i "s#dl-cdn.alpinelinux.org#$APK_MIRROR#g" /etc/apk/repositories; \
    fi \
 && apk add --no-cache ca-certificates \
 && addgroup -S wqn && adduser -S -G wqn wqn
WORKDIR /app
RUN mkdir -p /app/storage && chown -R wqn:wqn /app
COPY --from=builder /app/target/release/wrong_question_notebook_rs /usr/local/bin/wqn-backend
USER wqn
ENV APP_BIND_ADDR=0.0.0.0:8080 \
    LOCAL_STORAGE_ROOT=/app/storage
EXPOSE 8080
CMD ["wqn-backend"]
