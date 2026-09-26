# Lightweight container for the learn-rust course (and for building/running
# the databroker). Same base image as ../Dockerfile so layers are shared,
# without the release tooling (cross, pyenv, docker-in-docker).
FROM mcr.microsoft.com/devcontainers/rust:2-trixie
ARG DEBIAN_FRONTEND=noninteractive

# build-essential + cmake: databroker-proto builds a vendored `protoc`
# (protobuf-src) from C++ source; ring (TLS) needs a C compiler.
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends build-essential cmake \
    && rm -rf /var/lib/apt/lists/*
