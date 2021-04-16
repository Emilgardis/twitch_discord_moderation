# syntax = docker/dockerfile:experimental
FROM rustlang/rust:nightly as planner
WORKDIR /app
RUN --mount=type=cache,target=$CARGO_HOME/registry --mount=type=cache,target=$CARGO_HOME/bin cargo install cargo-chef
COPY . .
RUN ls -la
RUN cargo chef prepare  --recipe-path recipe.json

FROM rustlang/rust:nightly as cacher
WORKDIR /app
RUN --mount=type=cache,target=$CARGO_HOME/registry --mount=type=cache,target=$CARGO_HOME/bin cargo install cargo-chef
ARG cargo_args=""
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=$CARGO_HOME/registry cargo chef cook $cargo_args --recipe-path recipe.json -p twitch-discord-moderation

# TODO specify cross compilation instead
FROM rustlang/rust:nightly as builder
WORKDIR /app
COPY . .
# Copy over the cached dependencies
COPY --from=cacher /app/target target
COPY --from=cacher $CARGO_HOME $CARGO_HOME
RUN --mount=type=cache,target=$CARGO_HOME/registry cargo build $cargo_args --bin twitch-discord-moderation --out-dir ./bin/ -Z unstable-options

FROM rustlang/rust:nightly as runtime
WORKDIR /app
COPY --from=builder /app/bin/twitch-discord-moderation /app/
ENTRYPOINT "/app/twitch-discord-moderation"