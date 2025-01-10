FROM rust:1.80

RUN useradd -m -u 1001 -U -s /bin/sh -d /home/sxtuser sxtuser
USER sxtuser

WORKDIR /opt/sxt
COPY --chmod=755 --chown=sxtuser watcher .

ENTRYPOINT [ "/opt/sxt/watcher" ]
