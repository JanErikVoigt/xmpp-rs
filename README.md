xmpp-rs
=======

What's this?
------------

A set of rust XMPP libraries with the goals of being type-safe and well-tested.

- `xmpp` provides a high-level API for the protocol. You shouldn't need to
  know about the protocol to use it. Still very much WIP.
- `tokio-xmpp` is a lower-level library that handles the XMPP stream(s).
- `xmpp-parsers` parses XML into Rust and back.
- `minidom` is a DOM library quite specific for XMPP.
- `jid` is a Jid parsing library.
- `sasl` handles the SASL protocol.
- `xso` and `xso-proc` provide the procedural macros powering `xmpp-parsers`.

Contact
-------

There is an XMPP MUC for the discussion of these libraries, feel free to join! :)

[chat@xmpp.rs](xmpp:chat@xmpp.rs?join)

Community
---------

A Code of Conduct is available as
[CODE\_OF\_CONDUCT.md](https://gitlab.com/xmpp-rs/xmpp-rs/-/raw/main/CODE_OF_CONDUCT.md)
in the repository for the well-being of the community. Please refer to it in
case of a possible conflict in any of the xmpp-rs venues (channel, forge,
etc.).

License
-------

Mozilla Public License 2 (MPL2). See the LICENSE file.

Building
--------

Dependencies should be provided by crates if you use the default features. If
you use tokio-xmpp's `tls-native` feature you will need an ssl library
(openssl, libressl, etc.).

```
cargo build
```

The various features available should be explained in the crates themselves.

Contributing
------------

Thank you for your interest in the project!

Contributing rules are available as
[CONTRIBUTING.md](https://gitlab.com/xmpp-rs/xmpp-rs/-/raw/main/CONTRIBUTING.md) in the repository.
