# The Topos specification and development internals documentation

The specification doc is compiled from several source files with [`mdBook`](https://github.com/rust-lang/mdBook).
To view it live, locally, from the repo root:

Ensure graphviz and plantuml are installed:
```sh
brew install graphviz plantuml # for macOS
sudo apt-get install graphviz plantuml # for Ubuntu/Debian
```

Then install and build the book:

```sh
cargo install mdbook mdbook-plantuml mdbook-linkcheck mdbook-graphviz
mdbook serve doc
open http://localhost:3000
```
