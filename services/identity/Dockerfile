FROM --platform=$BUILDPLATFORM golang:1.24-alpine AS builder

ARG TARGETOS
ARG TARGETARCH

WORKDIR /app

COPY go.mod go.sum ./

RUN go mod download

COPY . .

RUN CGO_ENABLED=0 GOOS=$TARGETOS GOARCH=$TARGETARCH go build -a -installsuffix cgo -o main ./cmd/server

FROM gcr.io/distroless/static-debian11

WORKDIR /app

COPY --from=builder /app/main .

EXPOSE 8080

ENTRYPOINT ["./main"]
