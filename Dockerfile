FROM rust:1.62-bullseye
ARG LAST_BLOCK=

COPY Cargo.toml Cargo.lock /app/
COPY src /app/src

RUN cd app && cargo build --release

ENV RUST_LOG=debug
ENV LAST_BLOCK=$LAST_BLOCK
COPY secret /app

CMD cd /app && cargo run --release -- $LAST_BLOCK
