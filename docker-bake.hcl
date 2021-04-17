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
    tags = ["docker.io/emilgardis/${DOCKER_REPO}:${DOCKER_TAG}"]
    platforms = ["linux/amd64", "linux/arm64", "linux/arm/v7"]
}