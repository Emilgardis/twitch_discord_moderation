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
    platforms = ["linux/amd64"]
}
target "app-release" {
    inherits = ["app"]
    platforms = ["linux/amd64", "linux/arm64"]
}