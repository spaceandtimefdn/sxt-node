FROM rust:1.80

RUN useradd -m -u 1001 -U -s /bin/sh -d /home/sxtuser sxtuser

RUN apt-get update && apt-get install -y \
    clang \
    cmake \
    libssl-dev \
    pkg-config \
    git \
    && rm -rf /var/lib/apt/lists/*# Set environment for cargo install (use user-local bin directory)

ENV CARGO_HOME=/home/sxtuser/.cargo
ENV PATH=$CARGO_HOME/bin:$PATH 

RUN cargo install --force subkey --git https://github.com/paritytech/substrate --version 23.0.0 --locked

USER sxtuser

WORKDIR /opt/sxt

COPY --chmod=755 --chown=sxtuser watcher .

ENTRYPOINT [ "/opt/sxt/watcher" ]