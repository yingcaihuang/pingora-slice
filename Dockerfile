# Multi-stage build for Pingora Slice

# Stage 1: Build
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY examples ./examples

# Build release binary
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r pingora-slice && \
    useradd -r -g pingora-slice -d /var/cache/pingora-slice -s /sbin/nologin pingora-slice

# Create necessary directories
RUN mkdir -p /etc/pingora-slice \
    /var/cache/pingora-slice \
    /var/log/pingora-slice && \
    chown -R pingora-slice:pingora-slice /var/cache/pingora-slice /var/log/pingora-slice

# Copy binary from builder
COPY --from=builder /app/target/release/pingora-slice /usr/local/bin/

# Copy default configuration
COPY examples/pingora_slice.yaml /etc/pingora-slice/

# Set permissions
RUN chmod +x /usr/local/bin/pingora-slice

# Switch to non-root user
USER pingora-slice

# Expose ports
EXPOSE 8080 9091

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/pingora-slice", "--version"] || exit 1

# Set working directory
WORKDIR /var/cache/pingora-slice

# Run the application
CMD ["/usr/local/bin/pingora-slice", "/etc/pingora-slice/pingora_slice.yaml"]
