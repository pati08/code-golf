# Build args:
# OPTIMIZE=false (default): Fast debug builds
# OPTIMIZE=true: Production release builds
ARG OPTIMIZE=false

FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /code-golf

# Install build dependencies
RUN apt-get update && apt-get install -y \
  clang \
  mold \
  && rm -rf /var/lib/apt/lists/*

# ============ TOOLCHAIN & FETCH STAGE ============
# Install nightly toolchain and pre-fetch all dependencies
# This layer caches the rust compiler download and all crates
FROM chef AS fetch
COPY rust-toolchain.toml .
# Install the nightly toolchain specified in rust-toolchain.toml
RUN rustup toolchain install nightly && rustup default nightly

# ============ PLANNER STAGE ============
FROM fetch AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ============ BUILDER STAGE ============
FROM fetch AS builder
ARG OPTIMIZE=false

COPY --from=planner /code-golf/recipe.json recipe.json

# Build dependencies - all crates are already fetched, this just compiles them
RUN if [ "$OPTIMIZE" = "true" ]; then \
  cargo chef cook --release --recipe-path recipe.json; \
  else \
  cargo chef cook --recipe-path recipe.json; \
  fi

COPY . .

RUN touch src/main.rs

# Build the application
RUN if [ "$OPTIMIZE" = "true" ]; then \
  cargo build --release --bin code-golf && \
  cp target/release/code-golf /tmp/code-golf; \
  else \
  cargo build --bin code-golf && \
  cp target/debug/code-golf /tmp/code-golf; \
  fi

# ============ RUNTIME STAGE ============
FROM debian:trixie-slim AS runtime
WORKDIR /code-golf

RUN apt-get update && apt-get install -y \
  python3 \
  black \
  bash \
  shfmt \
  ruby \
  perl \
  perltidy \
  nodejs \
  npm \
  lua5.4 \
  && npm install -g prettier \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /tmp/code-golf /usr/local/bin/code-golf
COPY ./templates /code-golf/templates
COPY ./static /code-golf/static
ENTRYPOINT ["/usr/local/bin/code-golf"]
