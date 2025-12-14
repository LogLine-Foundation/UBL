FROM rust:1-bookworm as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/ubl_core /usr/local/bin/ubl_core
EXPOSE 8000
ENV RUST_LOG=info
CMD ["ubl_core"]
