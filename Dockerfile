FROM rustlang/rust:nightly-alpine AS builder

RUN apk add musl-dev

COPY . .
RUN cargo build --release

# Final image
FROM alfg/bento4:ffmpeg

RUN apk add python3 --no-cache --update
ENV PATH=/opt/ffmpeg/bin:$PATH

RUN mkdir /app
WORKDIR /app
# copy the binary into the final image
COPY --from=builder /target/release/streamin-conv .

# set the binary as entrypoint
ENTRYPOINT ["/app/streamin-conv"]