# Do most of our build in one container, we don't need the intermediate
# artifacts or build requirements in our release container. We'll copy in our
# produced binary to the final production container later.
FROM docker.io/library/rust:1.70.0 AS build

RUN mkdir -p /usr/src/build
WORKDIR /usr/src/build

# Perform a release build only using the dependencies and an otherwise empty
# binary project, to allow the dependencies to build and be cached. This
# prevents rebuilding them in the future if only the service's source has
# changed.
RUN cargo init --name web-app-template
COPY Cargo.toml Cargo.lock /usr/src/build
RUN cargo build --release

# Copy in the actual service source code, and perform the release build
# (install is release mode by default).
COPY build.rs /usr/src/build/build.rs
COPY src /usr/src/build/src
RUN cargo install --bins --path ./

# Use an absolutely minimal container with the barest permissions to limit
# sources of security vulnerabilities, and ensure that any security issues are
# extremely scoped in how they can be exploited.
FROM gcr.io/distroless/cc-debian11:nonroot

# Bring in just our final compiled artifact
COPY --from=build /usr/local/cargo/bin/web-app-template /usr/bin/web-app-template

EXPOSE 3000
VOLUME /data

CMD ["/usr/bin/web-app-template"]
