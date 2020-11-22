FROM rustlang/rust:nightly as rust

# Cache deps
WORKDIR /app
#RUN sudo chown -R rust:rust .
RUN USER=root cargo new twitch-discord-moderation

# Install cache-deps
RUN cargo install --git https://github.com/romac/cargo-build-deps.git

WORKDIR /app/twitch-discord-moderation
RUN mkdir -p xtask/src/
# Copy the Cargo tomls
COPY ./Cargo.toml ./Cargo.lock ./
RUN ls
COPY ./xtask/Cargo.toml ./xtask
RUN cat ./xtask/Cargo.toml
# Cache the deps
RUN cargo build-deps --release 

# Copy the src folders
COPY ./src ./src/
COPY ./xtask/src ./xtask/src/

# run
CMD cargo run --release --locked