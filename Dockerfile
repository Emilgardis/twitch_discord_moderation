# syntax = docker/dockerfile:experimental
FROM --platform=$BUILDPLATFORM rust:1.51-alpine3.13 as planner
WORKDIR /app
RUN apk add --no-cache \
        musl-dev
RUN --mount=type=cache,target=$CARGO_HOME/bin cargo install cargo-chef --version 0.1.19
COPY . .
RUN ls -la
RUN cargo chef prepare  --recipe-path recipe.json

FROM rust:1.51-alpine3.13 as cacher
WORKDIR /app
ARG BUILD_DEPS
RUN apk add --no-cache ${BUILD_DEPS}
COPY --from=planner $CARGO_HOME/bin/cargo-chef $CARGO_HOME/bin/cargo-chef 
COPY --from=planner /app/recipe.json recipe.json
ARG RUSTFLAGS=-Ctarget-feature=-crt-static
RUN --mount=type=cache,target=$CARGO_HOME/registry cargo chef cook --recipe-path recipe.json -p twitch-discord-moderation
FROM rust:1.51-alpine3.13 as builder
WORKDIR /app
ARG BUILD_DEPS
RUN apk add --no-cache ${BUILD_DEPS}
COPY . .
COPY --from=cacher /app/target /app/target
COPY --from=cacher $CARGO_HOME/registry $CARGO_HOME/registry
ARG RUSTFLAGS=-Ctarget-feature=-crt-static
RUN cargo -V; cargo build --locked --bin twitch-discord-moderation && mv /app/target/debug/twitch-discord-moderation /app/twitch-discord-moderation
FROM alpine:3.13 as runtime
WORKDIR /app
RUN apk add --no-cache \
        openssl ca-certificates
COPY --from=builder /app/twitch-discord-moderation /app/twitch-discord-moderation
ENTRYPOINT "/app/twitch-discord-moderation"