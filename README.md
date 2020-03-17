# slimweb

Slim HTTP 1.1 client/server library.

I felt compelled (or inspired if you will) to write this library after reading this article:
https://medium.com/@shnatsel/smoke-testing-rust-http-clients-b8f2ee5db4e6

More on the controversial side of the Rust community, it seemed quite interesting how such eloquent libraries could be riddled down to such minor details that could cause major problems.
So, I'm throwing another into the mix that will probably hit that same point.

* Rust 2018
* No async functionality.
* Decisively using deadlines for DoS prevention (didn't want to deal with leaky thread racing).
* Using Rustls for SSL/TLS encryption.
* Using flate2 for compression/decompression (GZip only).

### Installation
```toml
[dependencies]
slimweb = "0.1"
```

OR for using Server:
```toml
[dependencies.slimweb]
version = "0.1"
default-features = false

features = [ "server" ]
```

### ITW
- [X] Server deadlines
- [X] [100-Continue](https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/100)
- [ ] Multipart
