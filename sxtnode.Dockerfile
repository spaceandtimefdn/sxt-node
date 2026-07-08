# Build SxT Node Image
# Ubuntu 24.10 provides glibc 2.40, matching the Nix toolchain that
# builds sxt-node in CI. See PR #198 for the interpreter/glibc story.
FROM docker.io/library/ubuntu:24.10

# Switch to root user to make system-wide changes
USER root

ARG DEBIAN_FRONTEND=noninteractive
RUN useradd -m -u 1001 -U -s /bin/sh -d /sxtuser sxtuser && \
    mkdir -p  /data /key /sxtuser/.local/share && \
    chown -R sxtuser:sxtuser /data /key && \
    ln -s /data /sxtuser/.local/share/sxtuser && \
    apt-get update && \
    apt-get install -y \
    ca-certificates \
    curl \
    apache2-utils \
    && apt-get clean && \
    rm -rf /var/lib/apt/lists/*


ENV AZURE_ENDPOINT="https://opspublicblockssandboxst.blob.core.windows.net"
ENV AZURE_ACCOUNT_NAME="opspublicblockssandboxst"
ENV AZURE_CONTAINER_NAME="ops-publicblocks-sandbox-stdl-wus2"
ENV AZURE_BASE_PATH="/v0/ETHEREUM"
ENV RUST_LOG="info"


# Copy the built application from workspace
COPY --chmod=755 target/release/sxt-node /usr/local/bin

# Chainspecs
RUN mkdir -p /opt/chainspecs
COPY --chmod=644 chainspecs/raw/*-spec.json /opt/chainspecs/

# Switch to sxtuser
USER sxtuser

EXPOSE 30333 9933 9944 9615

VOLUME ["/data", "/key"]


# Set Defautl logging in Env
ENV RUST_BACKTRACE=full
ENV RUST_LOG=debug


# Entry point to start the application
ENTRYPOINT ["/usr/local/bin/sxt-node"]

