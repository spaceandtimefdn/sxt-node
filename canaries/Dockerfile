FROM rust:1.85

RUN useradd -m -u 1001 -U -s /bin/sh -d /home/sxtuser sxtuser
USER sxtuser

WORKDIR /opt/sxt
COPY --chmod=755 --chown=sxtuser canaries .

ENTRYPOINT [ "/opt/sxt/canaries" ]
