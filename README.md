# Spawn Server

This Rust library permits to execute programs without replying on `fork`. This crate exports macros `srpc!` (synchronous) and `arpc!` (asynchronous) that is similar to macro `sh!`. Have a look in the examples directory on how to use this macro.

## Usage

Use macro `srpc!` to send requests to the spawn server. This is a synchronous call. For asynchronous calls, use macro `arpc!`.

Add the following to your `Cargo.toml` file:

```toml
[dependencies]
spawn_server = { version="*" }

```

- ..

## Deployment

You should run the spawn server in the same container as the program that uses the spawn server. The spawn server will - for now - use port 8099.

## Build

Just execute `cargo build --release` to build the spawn server. 

