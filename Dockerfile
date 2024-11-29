FROM rust:1 AS build

WORKDIR /build
COPY src/       /build/src
COPY Cargo.toml /build/Cargo.toml
COPY Cargo.lock /build/Cargo.lock
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12 AS runtime
COPY --from=build /build/target/release/qqr /app/qqr

LABEL org.opencontainers.image.title="qqr"
LABEL org.opencontainers.image.url="https://github.com/ngc7293/qqr"
LABEL org.opencontainers.image.authors="contact@davidbourgault.ca"

EXPOSE 8000
ENTRYPOINT [ "/app/qqr" ]