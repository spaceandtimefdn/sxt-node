FROM parity/subkey AS subkey

FROM rust:1.80

RUN useradd -m -u 1001 -U -s /bin/sh -d /home/sxtuser sxtuser && \
    mkdir /key && chown -R sxtuser:sxtuser /key
USER sxtuser

WORKDIR /opt/sxt
COPY --from=subkey /usr/local/bin/subkey /usr/local/bin
COPY --chmod=755 --chown=sxtuser watcher .

VOLUME /key
ENTRYPOINT [ "/opt/sxt/watcher" ]
