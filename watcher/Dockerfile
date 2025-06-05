FROM debian:12-slim

RUN apt-get update && apt-get install -y ca-certificates \
  && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1001 -U -s /bin/sh -d /home/sxtuser sxtuser

USER sxtuser

WORKDIR /opt/sxt

COPY --chmod=755 --chown=sxtuser ./watcher /opt/sxt/watcher
COPY --chmod=755 --chown=sxtuser ./subkey/subkey /opt/sxt/subkey

ENTRYPOINT ["/opt/sxt/watcher"]