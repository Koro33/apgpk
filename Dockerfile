###########################################################################
FROM rustlang/rust:nightly-alpine as build
RUN apk add musl-dev
WORKDIR /build

COPY ./Cargo.toml ./Cargo.lock ./
RUN mkdir ./src && echo 'fn main() { println!("Dummy!"); }' > ./src/main.rs \
    && cargo build --release

RUN rm -rf ./src && rm -rf ./target/release
COPY ./src ./src
RUN touch -a -m ./src/main.rs \
    && cargo build --release

###########################################################################
FROM alpine:latest

WORKDIR /etc/apgpk
COPY --from=build /build/target/release/apgpk ./bin/
RUN mkdir -p key \ 
    && touch pattern

ENTRYPOINT [ "./bin/apgpk" ]
CMD [ "--pattern", "./pattern", "--output", "./key" ]



