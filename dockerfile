# ---- Build Stage ----
FROM rustlang/rust:nightly AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release


# ---- Runtime Stage ----
FROM debian:bookworm-slim

RUN apt-get update \
  && apt-get install -y \
     ca-certificates \
     libreoffice \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/tokio-pdf /usr/local/bin/tokio-pdf

EXPOSE 3000
CMD ["tokio-pdf --port 3000"]
