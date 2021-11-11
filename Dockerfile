FROM debian as builder

ARG TARGETPLATFORM

COPY ./aarch64-unknown-linux-gnu /target/aarch64-unknown-linux-gnu

RUN ls -lR /target

RUN if [ $TARGETPLATFORM = "linux/arm64" ]; then \
    mv /target/aarch64-unknown-linux-gnu/release/races /races; \
  elif [ $TARGETPLATFORM = "linux/amd64" ]; then \
    mv x86_64-unknown-linux-gnu/release/races /races; \
  fi; \
  chmod +x /races


FROM debian

COPY --from=builder /races /

CMD ["/races"]
