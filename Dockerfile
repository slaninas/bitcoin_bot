FROM rust:1.62-bullseye

COPY Cargo.toml Cargo.lock /app/
COPY src /app/src

RUN cd app && cargo build --release

ENV RUST_LOG=debug
COPY secret /app

CMD cd /app && cargo run --release
