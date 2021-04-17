# syntax = docker/dockerfile:experimental
FROM --platform=$BUILDPLATFORM rust:latest as planner
WORKDIR /app
RUN --mount=type=cache,target=$CARGO_HOME/registry --mount=type=cache,target=$CARGO_HOME/bin cargo install cargo-chef
COPY . .
RUN ls -la
RUN cargo chef prepare  --recipe-path recipe.json

FROM rust:latest as cacher
WORKDIR /app
RUN --mount=type=cache,target=$CARGO_HOME/registry --mount=type=cache,target=$CARGO_HOME/bin cargo install cargo-chef
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=$CARGO_HOME/registry cargo chef cook --release --recipe-path recipe.json -p twitch-discord-moderation

# TODO specify cross compilation instead
FROM rust:latest as builder
WORKDIR /app
COPY . .
# Copy over the cached dependencies
COPY --from=cacher /app/target target
COPY --from=cacher $CARGO_HOME $CARGO_HOME
RUN --mount=type=cache,target=$CARGO_HOME/registry cargo build --release --bin twitch-discord-moderation
FROM rust:latest as runtime
WORKDIR /app
COPY --from=builder /app/target/release/twitch-discord-moderation /app/twitch-discord-moderation
ENTRYPOINT "/app/twitch-discord-moderation"