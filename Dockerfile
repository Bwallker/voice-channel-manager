ARG release_mode=false
FROM rust as base

RUN mkdir app
WORKDIR /app

EXPOSE 5432
LABEL name="App Image"

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

RUN echo "release mode: ${release_mode}"
FROM release-${release_mode} as final
