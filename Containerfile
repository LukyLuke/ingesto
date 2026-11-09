ARG INGESTO_VERSION=0.1.0

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
RUN --mount=type=cache,target=$SCCACHE_DIR,sharing=locked,uid=1000 \
  cargo chef prepare --recipe-path recipe.json


# Then: Use cargo chef to download all third-party crates
FROM base AS cook
WORKDIR /app
COPY .cargo.toml.* .cargo/config.toml
COPY --from=prepare /app/recipe.json recipe.json
RUN --mount=type=cache,target=$SCCACHE_DIR,sharing=locked,uid=1000 \
  cargo chef cook --release --recipe-path recipe.json


# As last: Build the project with the cached crates
FROM cook AS build
WORKDIR /app
COPY .cargo.toml.* .cargo/config.toml
COPY . .
RUN --mount=type=cache,target=$SCCACHE_DIR,sharing=locked,uid=1000 \
  cargo build --release


# Finally: Create an image from scratch with all the binaries
FROM scratch
ARG INGESTO_VERSION

LABEL org.opencontainers.image.source=https://github.com/LukyLuke/ingesto
LABEL org.opencontainers.image.title="inesto"
LABEL org.opencontainers.image.description="ingesto is a Log-Processing-Pipeline"
LABEL org.opencontainers.image.licenses="MIT AND Apache-2.0"
LABEL org.opencontainers.image.authors="Lukas LukyLuke_ch Zurschmiede"
LABEL org.opencontainers.image.version=${INGESTO_VERSION}

WORKDIR /app

VOLUME /app/config
VOLUME /app/secrets

COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

COPY --from=build /lib/x86_64-linux-gnu/ld-linux-x86-64.so* /lib/x86_64-linux-gnu/
COPY --from=build /lib64/ld-linux-x86-64.so.2 /lib64/

COPY --from=build /lib/x86_64-linux-gnu/libc.so* /lib/x86_64-linux-gnu/
COPY --from=build /lib/x86_64-linux-gnu/libgcc_s.so* /lib/x86_64-linux-gnu/
COPY --from=build /lib/x86_64-linux-gnu/libm.so* /lib/x86_64-linux-gnu/
COPY --from=build /lib/x86_64-linux-gnu/libselinux.so* /lib/x86_64-linux-gnu/
COPY --from=build /lib/x86_64-linux-gnu/libcap.so* /lib/x86_64-linux-gnu/
COPY --from=build /lib/x86_64-linux-gnu/libpcre* /lib/x86_64-linux-gnu/
COPY --from=build /usr/bin/sh /bin/
COPY --from=build /usr/bin/ls /bin/
COPY --from=build /usr/bin/cat /bin/
COPY --from=build /usr/bin/mv /bin/
COPY --from=build /usr/bin/rm /bin/

COPY --from=build /app/target/release/ingesto-* .
RUN /bin/rm *.d

USER 1000
ENTRYPOINT [""]
CMD [""]
