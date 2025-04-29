# Build SxT Node Image with PostgreSQL
FROM docker.io/parity/base-bin:latest

# Switch to root user to make system-wide changes
USER root

# Install PostgreSQL
ARG DEBIAN_FRONTEND=noninteractive
RUN useradd -m -u 1001 -U -s /bin/sh -d /sxtuser sxtuser && \
    mkdir -p  /data /key /pg_data /logs/postgres /sxtuser/.local/share && \
    chown -R sxtuser:sxtuser /data /key /pg_data /logs/postgres  && \
    ln -s /data /sxtuser/.local/share/sxtuser && \
    apt-get update --allow-insecure-repositories && \
    apt-get install -y \
    postgresql \
    postgresql-contrib \
    curl \
    apache2-utils \
    && apt-get clean && \
    chown -R sxtuser:sxtuser /var/run/postgresql && \
    rm -rf /var/lib/apt/lists/*


# Default environment variables for Flightsql-pg
# These can be overridden in helm or docker startup.
# Flightsql relies on Postgres as DB.
ENV POSTGRES_DB=postgres
ENV POSTGRES_USER=postgres
ENV POSTGRES_PASSWORD=postgres
ENV DATABASE_URL="postgresql://localhost:5432/postgres?user=postgres&password=postgres"
ENV HOST="127.0.0.1"
ENV PORT="50555"
ENV FLIGHTSQL_USER="admin"
ENV FLIGHTSQL_PASSWORD="admin"
ENV AZURE_ENDPOINT="https://opspublicblockssandboxst.blob.core.windows.net"
ENV AZURE_ACCOUNT_NAME="opspublicblockssandboxst"
ENV AZURE_CONTAINER_NAME="ops-publicblocks-sandbox-stdl-wus2"
ENV AZURE_BASE_PATH="/v0/ETHEREUM"
ENV RUST_LOG="info"


# Install Flight-SQL
ARG FLIGHTSQL_PG_SERVICE="https://spaceandtime.jfrog.io/artifactory/dw-generic-local/flightsql-pg/0.1-16d2f5f/x86/flightsql-pg"
RUN --mount=type=secret,id=ARTIFACTORY_LOGIN  \
    curl --user $(cat /run/secrets/ARTIFACTORY_LOGIN) $FLIGHTSQL_PG_SERVICE --output /usr/local/bin/flightsql-pg &&  \
    chmod +x /usr/local/bin/flightsql-pg

# Copy the built application from workspace
COPY --chmod=755 target/release/sxt-node /usr/local/bin

# Copy SxT Initializetion scripts
COPY --chmod=755 scripts/* /opt

# Chainspecs
RUN mkdir -p /opt/chainspecs
COPY --chmod=644 chainspecs/raw/*-spec.json /opt/chainspecs/

# Switch to sxtuser
USER sxtuser

# Expose ports
# NOTE: Not exposing ports for Postgres and Flightsql-pg.
EXPOSE 30333 9933 9944 9615

# Set volume.
# TO DO - Add Volume mounts for Postgres Data, Logs etc.
VOLUME ["/data", "/key"]


# Set Defautl logging in Env
ENV RUST_BACKTRACE=full
ENV RUST_LOG=debug


# Entry point to start the application
ENTRYPOINT ["/opt/sxtnode.sh"]

