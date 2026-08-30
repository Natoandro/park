ARG RUST_VERSION=1.85
ARG BUILD_PROFILE=release
FROM rust:${RUST_VERSION}-bookworm AS builder
ARG BUILD_PROFILE

WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
COPY e2e-macros ./e2e-macros
COPY build.rs ./build.rs
COPY src ./src
RUN if [ "$BUILD_PROFILE" = "release" ]; then \
        cargo build --release --bin park --bin park-e2e; \
    elif [ "$BUILD_PROFILE" = "debug" ]; then \
        cargo build --bin park --bin park-e2e; \
    else \
        printf 'unsupported BUILD_PROFILE: %s\n' "$BUILD_PROFILE" >&2; \
        exit 2; \
    fi
RUN mkdir /out \
    && if [ "$BUILD_PROFILE" = "release" ]; then profile=release; else profile=debug; fi \
    && cp "target/$profile/park" /out/park \
    && cp "target/$profile/park-e2e" /out/park-e2e

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates procps \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 park \
    && useradd --uid 10001 --gid 10001 --create-home --shell /bin/sh park

COPY --from=builder /out/park /usr/local/bin/park
COPY --from=builder /out/park-e2e /usr/local/bin/park-e2e
COPY docker/e2e/entrypoint.sh /usr/local/bin/park-e2e-entrypoint

RUN chown park:park /usr/local/bin/park /usr/local/bin/park-e2e /usr/local/bin/park-e2e-entrypoint

ENV PARK_BIN=/usr/local/bin/park \
    PARK_E2E_RUNNER=/usr/local/bin/park-e2e \
    RUST_BACKTRACE=1

USER park
ENTRYPOINT ["/usr/local/bin/park-e2e-entrypoint"]
