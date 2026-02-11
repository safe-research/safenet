FROM ghcr.io/foundry-rs/foundry:stable

COPY --chown=foundry contracts /contracts
WORKDIR /contracts

RUN forge build
