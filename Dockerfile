ARG GIT_SSL_NO_VERIFY=0
ARG CARGO_NET_GIT_FETCH_WITH_CLI="false"
ARG CARGO_HTTP_CAINFO

FROM rust:1.97 as base

RUN cargo install sccache --version ^0.16
RUN cargo install cargo-chef --version ^0.1

ARG RUSTC_WRAPPER=sccache
ARG SCCACHE_DIR=/sccache


FROM base AS prepare
WORKDIR /app
COPY . .
RUN --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
  cargo chef prepare --recipe-path recipe.json


FROM base AS cook
WORKDIR /app
COPY --from=prepare /app/recipe.json recipe.json
RUN --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
  cargo chef cook --release --resipe-path recipe.json


FROM cook AS build
WORKDIR /app
COPY . .
RUN --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
  cargo build --release


FROM scratch
WORKDIR /app
COPY --from=build /app/target/release/ .
RUN rm -Rf /app/{build,incremental,deps,examples,*.d,lib*}

