variable "TAG" {
    default = "latest"
}

group "default" {
    targets = ["app"]
}

target "app" {
    tags = ["docker.io/emilgardis/twitch-discord-moderation:${TAG}"]
    platforms = ["linux/amd64", "linux/arm64", "linux/arm/v7"]
}