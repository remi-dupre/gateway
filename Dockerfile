FROM rust:1.58.1 as builder
WORKDIR /usr/src

RUN apt-get update && \
    apt-get dist-upgrade -y

RUN USER=root cargo new gateway
WORKDIR /usr/src/gateway
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo install --path .


FROM debian:buster-20220125-slim
RUN apt-get -y update && \
    apt-get -y install libssl-dev && \
    apt-get clean autoclean && \
    apt-get autoremove --yes && \
    rm -rf /var/lib/{apt,dpkg,cache,log}/
COPY --from=builder /usr/local/cargo/bin/gateway /home/app/gateway
WORKDIR /home/app
USER 1000
CMD ["/home/app/gateway", "/config/runtime_config.yaml"]

