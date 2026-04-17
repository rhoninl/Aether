FROM --platform=$BUILDPLATFORM rust:1.84-alpine AS builder

ARG TARGETOS
ARG TARGETARCH

WORKDIR /app

# Install the minimal cross-compile toolchain Alpine needs.
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static

# Copy the entire workspace so path dependencies resolve.
COPY . .

RUN cargo build --release -p agent-cp \
    && cp target/release/agent-cp /agent-cp

FROM gcr.io/distroless/static-debian12

WORKDIR /app

COPY --from=builder /agent-cp ./agent-cp

EXPOSE 7830 7831

ENTRYPOINT ["./agent-cp"]
