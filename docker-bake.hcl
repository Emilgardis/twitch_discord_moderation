variable "DOCKER_TAG" {
    default = "latest"
}

variable "DOCKER_REPO" {
    default = "twitch-discord-moderation"
}

group "default" {
    targets = ["app"]
}

group "release" {
    targets = ["app", "app-aarch64"]
}

target "app" {
    tags = ["emilgardis/${DOCKER_REPO}:${DOCKER_TAG}"]
    platforms = ["linux/amd64"]
    args = {
        BUILD_DEPS="musl-dev pkgconfig perl build-base openssl openssl-dev"
    }
}

target "app-aarch64" {
    inherits = ["app"]
    platforms = ["linux/arm64"]
    args = {
        BUILD_DEPS="musl-dev pkgconfig perl build-base openssl openssl-dev"
    }
}

target "app-v7" {
    inherits = ["app"]
    // armv7 is broken, see https://github.com/docker/buildx/issues/395 
    // rust-alpine doesn't  support it either
    platforms = ["linux/arm/v7"]
}