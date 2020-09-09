# quickload

A rust crate for quickly downloading files over the HTTP protocol.

## Features

- loads multiple chunks in parallel
- preallocates disk space in advance for efficient positional writes

## Examples

1. Install [rust](https://www.rust-lang.org/).

2. Clone the git repo, `cd` into it.

3. Run the `quickload` example.
   
    ```shell
    cargo run --example quickload
    ```

    Add extra arguments as requested.
