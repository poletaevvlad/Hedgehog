FROM rust:1.60.0-alpine3.15

RUN apk update \
 && apk add jq dbus-dev gstreamer-dev build-base openssl-dev glib-dev sqlite-dev

WORKDIR /build
COPY . .

RUN RUSTFLAGS="-C target-feature=-crt-static" cargo test --all --features mpris \
    && mkdir /tmp/hedgehog_build \
    && RUSTFLAGS="-C target-feature=-crt-static" \
        cargo build --release --features mpris --message-format=json-render-diagnostics \
       | jq -r 'select(.out_dir) | select(.package_id | startswith("hedgehog-tui ")) | .out_dir' \
       > /tmp/hedgehog_build/out_dir 
RUN cp -r "$(cat /tmp/hedgehog_build/out_dir)/config" "/tmp/hedgehog_build/config"


FROM alpine:3.15

RUN apk update \
 && apk add dbus gstreamer openssl sqlite-libs libgcc

COPY --from=0 /build/target/release/hedgehog /usr/bin/hedgehog
COPY --from=0 /tmp/hedgehog_build/config /usr/share/hedgehog

RUN addgroup -S hedgehog\
 && adduser -S hedgehog -G hedgehog
USER hedgehog

CMD /usr/bin/hedgehog
