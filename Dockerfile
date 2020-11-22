FROM rust:1.41.0 as chef
WORKDIR /app
RUN cargo install cargo-chef --version ^0.1

FROM chef as prepare
COPY Cargo.toml Cargo.lock /app/
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as build-env
COPY --from=prepare /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY src /app/src
RUN cargo build --release

FROM gcr.io/distroless/cc
COPY --from=build-env /app/target/release/modem_status /
CMD ["./modem_status"]
