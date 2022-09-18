variable "build_arm" {
    default = false
}

group "default" {
    targets = ["app"]
}

target "docker-metadata-action" {}

target "app" {
    inherits = ["docker-metadata-action"]
    platforms = ["linux/amd64", build_arm ? "linux/arm64" : ""]
    args = {
        BUILD_DEPS="musl-dev pkgconfig perl build-base openssl openssl-dev git"
        RUN_DEPS="ca-certificates openssl libgcc"
    }
}