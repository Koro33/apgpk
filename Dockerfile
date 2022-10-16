###########################################################################
FROM rustlang/rust:nightly-slim as build
WORKDIR /build

COPY ./Cargo.toml ./Cargo.lock ./
RUN mkdir ./src && echo 'fn main() { println!("Dummy!"); }' > ./src/main.rs \
    && cargo build --release

RUN rm -rf ./src && rm -rf ./target/release
COPY ./src ./src
RUN touch -a -m ./src/main.rs \
    && cargo build --release

###########################################################################
FROM debian:bullseye-slim
WORKDIR /etc/apgpk
COPY --from=build /build/target/release/apgpk ./bin/
RUN mkdir -p key \ 
    && touch pattern

ENTRYPOINT [ "./bin/apgpk" ]
CMD [ "--pattern", "./pattern", "--output", "./key" ]



