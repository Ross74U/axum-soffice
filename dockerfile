# ---- Build Stage ----
FROM rustlang/rust:nightly AS builder

RUN apt-get update && apt-get install -y \
    cmake \
    make \
    gcc \
    g++ \
    nasm \                 
    pkg-config \
    libjpeg-dev \
    git \
 && rm -rf /var/lib/apt/lists/*

# build docx2pdf_rs
RUN git clone https://github.com/Ross74U/docx2pdf-rs.git
WORKDIR /docx2pdf-rs
RUN cargo build --release

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release


# ---- Runtime Stage ----
FROM debian:bookworm-slim

RUN apt-get update \
  && apt-get install -y \
     ca-certificates \
     libreoffice-core \
     libreoffice-writer \
     libreoffice-common \
     ure \
     fonts-dejavu \
     curl \    
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/tokio-pdf /usr/local/bin/tokio-pdf
COPY --from=builder /docx2pdf-rs/target/release/docx2pdf_rs /usr/local/bin/docx2pdf-rs

ENV DOCX2PDF_RS_COMMAND="docx2pdf-rs"

EXPOSE 4000
CMD ["tokio-pdf", "--port", "4000"]
