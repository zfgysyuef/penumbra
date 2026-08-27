FROM rust:latest

RUN apt-get update && apt-get install -y \
    musl-tools \
    gcc-mingw-w64-x86-64 \
    mingw-w64 \
    libudev-dev \
    pkg-config

RUN rustup target add x86_64-unknown-linux-musl \
    aarch64-unknown-linux-musl \
    x86_64-pc-windows-gnu

RUN rustup toolchain install nightly && \
    rustup target add --toolchain nightly \
    x86_64-unknown-linux-musl \
    aarch64-unknown-linux-musl \
    x86_64-pc-windows-gnu

RUN rustup default nightly

WORKDIR /usr/src/penumbra

CMD ["/bin/bash"]
