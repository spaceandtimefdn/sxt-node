# Build SxT Node Image
FROM nixos/nix as builder

WORKDIR /sxt-node
COPY . /sxt-node

RUN nix develop --experimental-features "nix-command flakes" --command cargo build --locked --release

FROM docker.io/parity/base-bin:latest

# Switch to root user to make system-wide changes
USER root

ARG DEBIAN_FRONTEND=noninteractive
RUN useradd -m -u 1001 -U -s /bin/sh -d /sxtuser sxtuser && \
    mkdir -p  /data /key /sxtuser/.local/share && \
    chown -R sxtuser:sxtuser /data /key && \
    ln -s /data /sxtuser/.local/share/sxtuser && \
    apt-get update --allow-insecure-repositories && \
    apt-get install -y \
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
COPY --from=builder --chmod=755 /sxt-node/target/release/sxt-node /usr/local/bin
COPY --from=builder /nix/store /nix/store

# Chainspecs
RUN mkdir -p /opt/chainspecs
COPY --from=builder --chmod=644 sxt-node/chainspecs/raw/*-spec.json /opt/chainspecs/

# Switch to sxtuser
USER sxtuser

EXPOSE 30333 9933 9944 9615

VOLUME ["/data", "/key"]


# Set Defautl logging in Env
ENV RUST_BACKTRACE=full
ENV RUST_LOG=debug


# Entry point to start the application
ENTRYPOINT ["/usr/local/bin/sxt-node"]

