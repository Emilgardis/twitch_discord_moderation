variable "TAG" {
    default = "latest"
}

group "default" {
    targets = ["app"]
}

target "app" {
    tags = ["docker.io/emilgardis/twitch-discord-moderation:${TAG}"]
}