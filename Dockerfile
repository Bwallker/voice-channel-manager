ARG release_mode=false
ARG use_precompiled
FROM rust as base

RUN mkdir app
WORKDIR /app


EXPOSE 5432
LABEL name="App Image"

FROM base AS precompiled-false

COPY ./src ./src
COPY ./Cargo.toml ./Cargo.toml
COPY ./migrations ./migrations
COPY ./build.rs ./build.rust

FROM base as debug-mode

ENTRYPOINT ["cargo", "run"]

FROM base as release-mode

ENTRYPOINT ["cargo", "run", "--release"]

FROM debug-mode as release-false
FROM release-mode as release-true

FROM ubuntu AS precompiled-true

COPY ./bin ./bin

ENTRYPOINT ["./bin/x86_64-unknown-linux-gnu/voice-channel-manager"]


RUN echo "release mode: ${release_mode}"
RUN echo "use precompiled: ${use_precompiled}"
FROM release-${release_mode} as final2
FROM precompiled-${use_precompiled} as final