FROM rust:1.74.0-bookworm as build

WORKDIR /app

ARG CARGO_TERM_COLOR=always

COPY . .

ARG SQLX_OFFLINE=true

RUN mkdir bin

RUN --mount=type=cache,id=rust-build,target=/app/target \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    echo $CARGO_TERM_COLOR && \
    cargo build --target-dir /app/target \
    && cp /app/target/debug/mare-website /app/bin

FROM debian:bookworm

WORKDIR /app

COPY --from=build /app/bin/mare-website /app/bin/mare-website

# COPY .env /app

# RUN apt-get update && apt install -y openssl

CMD [ "/app/bin/mare-website" ]
