# Base is to have cargo build with cache
FROM rust:1.97 as base
COPY .cargo.toml.* .cargo/config.toml
RUN cargo install sccache --version ^0.16
RUN cargo install cargo-chef --version ^0.1

ARG RUSTC_WRAPPER=sccache
ARG SCCACHE_DIR=/sccache


# First: Create a recipe which is cached
FROM base AS prepare
WORKDIR /app
COPY .cargo.toml.* .cargo/config.toml
COPY . .
RUN --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
  cargo chef prepare --recipe-path recipe.json


# Then: Use cargo chef to download all third-party crates
FROM base AS cook
WORKDIR /app
COPY .cargo.toml.* .cargo/config.toml
COPY --from=prepare /app/recipe.json recipe.json
RUN --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
  cargo chef cook --release --recipe-path recipe.json


# As last: Build the project with the cached crates
FROM cook AS build
WORKDIR /app
COPY .cargo.toml.* .cargo/config.toml
COPY . .
RUN --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
  cargo build --release
RUN rm -Rf /app/{build,incremental,deps,examples,*.d,lib*}


# Finally: Create an image from scratch with all the binaries
FROM scratch
WORKDIR /app
COPY --from=build /app/target/release/ .

