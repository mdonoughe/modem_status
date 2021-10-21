FROM gcr.io/distroless/cc
COPY target/release/modem_status /
CMD ["./modem_status"]
