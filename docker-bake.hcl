variable "DOCKER_TAG" {
    default = "latest"
}

variable "DOCKER_REPO" {
    default = "twitch-discord-moderation"
}

group "default" {
    targets = ["app"]
}

target "app" {
    tags = ["emilgardis/${DOCKER_REPO}:${DOCKER_TAG}"]
    platforms = ["linux/amd64", "linux/arm64"]
    args = {
        BUILD_DEPS="musl-dev pkgconfig perl build-base openssl openssl-dev git"
        RUN_DEPS="ca-certificates openssl libgcc"
    }
}

target "app-v7" {
    inherits = ["app"]
    // armv7 is broken, see https://github.com/docker/buildx/issues/395 
    // rust-alpine doesn't  support it either
    platforms = ["linux/arm/v7"]
}