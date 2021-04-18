# syntax = docker/dockerfile:experimental
# TODO specify cross compilation instead
FROM rust:1.51 as builder
WORKDIR /app
COPY . .
RUN --mount=type=cache,target=$CARGO_HOME/registry --mount=type=cache,target=/app/target cargo build --release --bin twitch-discord-moderation; mv /app/target/release/twitch-discord-moderation /app/twitch-discord-moderation
FROM debian:buster as runtime
WORKDIR /app
COPY --from=builder /app/twitch-discord-moderation /app/twitch-discord-moderation
ENTRYPOINT "/app/twitch-discord-moderation"