# Third-Party Notices

This product (**Alba Agent**, including `nexus-albaai`, `alba-agent-desktop`, and the React UI workspace under `ui/`) includes third-party open-source software components governed by permissive licenses.

All resolved dependencies in the Rust workspace (`cargo deny check licenses`: `licenses ok`) and locked npm packages (`ui/package-lock.json`) satisfy the commercial-safe permissive license policy.

## 1. Rust Workspace Dependencies (491 Resolved Packages)

| Package | Version | License | Attribution | Repository |
|---|---|---|---|---|
| `adler2` | `2.0.1` | `0BSD OR MIT OR Apache-2.0` | Jonas Schievink <jonasschievink@gmail.com>, oyvindln <oyvindln@users.noreply.github.com> | https://github.com/oyvindln/adler2 |
| `aho-corasick` | `1.1.5` | `Unlicense OR MIT` | Andrew Gallant <jamslam@gmail.com> | https://github.com/BurntSushi/aho-corasick |
| `alloc-no-stdlib` | `3.0.0` | `BSD-3-Clause` | Daniel Reiter Horn <danielrh@dropbox.com> | https://github.com/dropbox/rust-alloc-no-stdlib |
| `alloc-stdlib` | `0.3.0` | `BSD-3-Clause` | Daniel Reiter Horn <danielrh@dropbox.com> | https://github.com/dropbox/rust-alloc-no-stdlib |
| `android_system_properties` | `0.1.6` | `MIT OR Apache-2.0` | Nicolas Silva <nical@fastmail.com> | https://github.com/nical/android_system_properties |
| `anstream` | `1.0.0` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-cli/anstyle.git |
| `anstyle` | `1.0.14` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-cli/anstyle.git |
| `anstyle-parse` | `1.0.0` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-cli/anstyle.git |
| `anstyle-query` | `1.1.5` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-cli/anstyle.git |
| `anstyle-wincon` | `3.0.11` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-cli/anstyle.git |
| `anyhow` | `1.0.104` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/anyhow |
| `atk` | `0.18.2` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk3-rs |
| `atk-sys` | `0.18.2` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk3-rs |
| `atomic-waker` | `1.1.2` | `Apache-2.0 OR MIT` | Stjepan Glavina <stjepang@gmail.com>, Contributors to futures-rs | https://github.com/smol-rs/atomic-waker |
| `autocfg` | `1.5.1` | `Apache-2.0 OR MIT` | Josh Stone <cuviper@gmail.com> | https://github.com/cuviper/autocfg |
| `base64` | `0.21.7` | `MIT OR Apache-2.0` | Alice Maz <alice@alicemaz.com>, Marshall Pierce <marshall@mpierce.org> | https://github.com/marshallpierce/rust-base64 |
| `base64` | `0.22.1` | `MIT OR Apache-2.0` | Marshall Pierce <marshall@mpierce.org> | https://github.com/marshallpierce/rust-base64 |
| `base64` | `0.23.1` | `MIT OR Apache-2.0` | Marshall Pierce <marshall@mpierce.org> | https://github.com/marshallpierce/rust-base64 |
| `bit-set` | `0.11.1` | `Apache-2.0 OR MIT` | Alexis Beingessner <a.beingessner@gmail.com> | https://github.com/contain-rs/bit-vec |
| `bit-set` | `0.8.0` | `Apache-2.0 OR MIT` | Alexis Beingessner <a.beingessner@gmail.com> | https://github.com/contain-rs/bit-set |
| `bit-vec` | `0.10.2` | `Apache-2.0 OR MIT` | Alexis Beingessner <a.beingessner@gmail.com> | https://github.com/contain-rs/bit-vec |
| `bit-vec` | `0.8.0` | `Apache-2.0 OR MIT` | Alexis Beingessner <a.beingessner@gmail.com> | https://github.com/contain-rs/bit-vec |
| `bitflags` | `1.3.2` | `MIT/Apache-2.0` | The Rust Project Developers | https://github.com/bitflags/bitflags |
| `bitflags` | `2.13.2` | `MIT OR Apache-2.0` | The Rust Project Developers | https://github.com/bitflags/bitflags |
| `block-buffer` | `0.10.4` | `MIT OR Apache-2.0` | RustCrypto Developers | https://github.com/RustCrypto/utils |
| `block2` | `0.6.2` | `MIT` | Mads Marquart <mads@marquart.dk> | https://github.com/madsmtm/objc2 |
| `brotli` | `9.1.1` | `BSD-3-Clause AND MIT` | Daniel Reiter Horn <danielrh@dropbox.com>, The Brotli Authors | https://github.com/dropbox/rust-brotli |
| `brotli-decompressor` | `6.3.1` | `BSD-3-Clause/MIT` | Daniel Reiter Horn <danielrh@dropbox.com>, The Brotli Authors | https://github.com/dropbox/rust-brotli-decompressor |
| `bs58` | `0.5.1` | `MIT/Apache-2.0` | See upstream crate manifest | https://github.com/Nullus157/bs58-rs |
| `bstr` | `1.13.1` | `MIT OR Apache-2.0` | Andrew Gallant <jamslam@gmail.com> | https://github.com/BurntSushi/bstr |
| `bumpalo` | `3.20.3` | `MIT OR Apache-2.0` | Nick Fitzgerald <fitzgen@gmail.com> | https://github.com/fitzgen/bumpalo |
| `bytemuck` | `1.25.2` | `Zlib OR Apache-2.0 OR MIT` | Lokathor <zefria@gmail.com> | https://github.com/Lokathor/bytemuck |
| `byteorder` | `1.5.0` | `Unlicense OR MIT` | Andrew Gallant <jamslam@gmail.com> | https://github.com/BurntSushi/byteorder |
| `bytes` | `1.12.1` | `MIT` | Carl Lerche <me@carllerche.com>, Sean McArthur <sean@seanmonstar.com> | https://github.com/tokio-rs/bytes |
| `cairo-rs` | `0.18.5` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk-rs-core |
| `cairo-sys-rs` | `0.18.2` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk-rs-core |
| `camino` | `1.2.6` | `MIT OR Apache-2.0` | Without Boats <saoirse@without.boats>, Ashley Williams <ashley666ashley@gmail.com>, Steve Klabnik <steve@steveklabnik.com>, Rain <rain@sunshowers.io> | https://github.com/camino-rs/camino |
| `cargo-platform` | `0.1.9` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-lang/cargo |
| `cargo_metadata` | `0.19.2` | `MIT` | Oliver Schneider <git-spam-no-reply9815368754983@oli-obk.de> | https://github.com/oli-obk/cargo_metadata |
| `cargo_toml` | `1.0.1` | `Apache-2.0 OR MIT` | Kornel <kornel@geekhood.net> | https://gitlab.com/lib.rs/cargo_toml |
| `cc` | `1.11.1` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-lang/cc-rs |
| `cesu8` | `1.1.0` | `Apache-2.0/MIT` | Eric Kidd <git@randomhacks.net> | https://github.com/emk/cesu8-rs |
| `cfb` | `0.14.0` | `MIT` | Matthew D. Steele <mdsteele@alum.mit.edu> | https://github.com/mdsteele/rust-cfb |
| `cfg-expr` | `0.15.8` | `MIT OR Apache-2.0` | Embark <opensource@embark-studios.com>, Jake Shadle <jake.shadle@embark-studios.com> | https://github.com/EmbarkStudios/cfg-expr |
| `cfg-if` | `1.0.5` | `MIT OR Apache-2.0` | Alex Crichton <alex@alexcrichton.com> | https://github.com/rust-lang/cfg-if |
| `cfg_aliases` | `0.2.2` | `MIT` | Zicklag <zicklag@katharostech.com> | https://github.com/katharostech/cfg_aliases |
| `chacha20` | `0.10.2` | `MIT OR Apache-2.0` | RustCrypto Developers | https://github.com/RustCrypto/stream-ciphers |
| `chrono` | `0.4.45` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/chronotope/chrono |
| `clap` | `4.6.7` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/clap-rs/clap |
| `clap_builder` | `4.6.7` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/clap-rs/clap |
| `clap_derive` | `4.6.7` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/clap-rs/clap |
| `clap_lex` | `1.1.1` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/clap-rs/clap |
| `colorchoice` | `1.0.5` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-cli/anstyle.git |
| `combine` | `4.6.9` | `MIT` | Markus Westerlind <marwes91@gmail.com> | https://github.com/Marwes/combine |
| `cookie` | `0.18.2` | `MIT OR Apache-2.0` | Sergio Benitez <sb@sergio.bz>, Alex Crichton <alex@alexcrichton.com> | https://github.com/SergioBenitez/cookie-rs |
| `core-foundation` | `0.10.1` | `MIT OR Apache-2.0` | The Servo Project Developers | https://github.com/servo/core-foundation-rs |
| `core-foundation-sys` | `0.8.7` | `MIT OR Apache-2.0` | The Servo Project Developers | https://github.com/servo/core-foundation-rs |
| `core-graphics` | `0.25.0` | `MIT OR Apache-2.0` | The Servo Project Developers | https://github.com/servo/core-foundation-rs |
| `core-graphics-types` | `0.2.0` | `MIT OR Apache-2.0` | The Servo Project Developers | https://github.com/servo/core-foundation-rs |
| `cpufeatures` | `0.2.17` | `MIT OR Apache-2.0` | RustCrypto Developers | https://github.com/RustCrypto/utils |
| `cpufeatures` | `0.3.1` | `MIT OR Apache-2.0` | RustCrypto Developers | https://github.com/RustCrypto/utils |
| `crc32fast` | `1.5.5` | `MIT OR Apache-2.0` | Sam Rijs <srijs@airpost.net>, Alex Crichton <alex@alexcrichton.com> | https://github.com/srijs/rust-crc32fast |
| `crossbeam-channel` | `0.5.17` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/crossbeam-rs/crossbeam |
| `crossbeam-deque` | `0.8.8` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/crossbeam-rs/crossbeam |
| `crossbeam-epoch` | `0.9.21` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/crossbeam-rs/crossbeam |
| `crossbeam-utils` | `0.8.23` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/crossbeam-rs/crossbeam |
| `crypto-common` | `0.1.7` | `MIT OR Apache-2.0` | RustCrypto Developers | https://github.com/RustCrypto/traits |
| `cssparser` | `0.37.0` | `MPL-2.0` | Simon Sapin <simon.sapin@exyr.org> | https://github.com/servo/rust-cssparser |
| `cssparser` | `0.38.0` | `MPL-2.0` | Simon Sapin <simon.sapin@exyr.org> | https://github.com/servo/rust-cssparser |
| `cssparser-macros` | `0.7.1` | `MPL-2.0` | Simon Sapin <simon.sapin@exyr.org> | https://github.com/servo/rust-cssparser |
| `ctor` | `1.0.13` | `Apache-2.0 OR MIT` | Matt Mastracci <matthew@mastracci.com> | https://github.com/mmastrac/linktime |
| `darling` | `0.24.1` | `MIT` | Ted Driggs <ted.driggs@outlook.com> | https://github.com/TedDriggs/darling |
| `darling_core` | `0.24.1` | `MIT` | Ted Driggs <ted.driggs@outlook.com> | https://github.com/TedDriggs/darling |
| `darling_macro` | `0.24.1` | `MIT` | Ted Driggs <ted.driggs@outlook.com> | https://github.com/TedDriggs/darling |
| `dbus` | `0.9.12` | `Apache-2.0/MIT` | David Henningsson <diwic@ubuntu.com> | https://github.com/diwic/dbus-rs |
| `deranged` | `0.5.9` | `MIT OR Apache-2.0` | Jacob Pratt <jacob@jhpratt.dev> | https://github.com/jhpratt/deranged |
| `derive_more` | `2.1.1` | `MIT` | Jelte Fennema <github-tech@jeltef.nl> | https://github.com/JelteF/derive_more |
| `derive_more-impl` | `2.1.1` | `MIT` | Jelte Fennema <github-tech@jeltef.nl> | https://github.com/JelteF/derive_more |
| `digest` | `0.10.7` | `MIT OR Apache-2.0` | RustCrypto Developers | https://github.com/RustCrypto/traits |
| `dirs` | `7.0.0` | `MIT OR Apache-2.0` | Simon Ochsenreither <simon@ochsenreither.de> | https://codeberg.org/dirs/dirs-rs |
| `dirs-sys` | `0.5.0` | `MIT OR Apache-2.0` | Simon Ochsenreither <simon@ochsenreither.de> | https://github.com/dirs-dev/dirs-sys-rs |
| `dispatch2` | `0.3.1` | `Zlib OR Apache-2.0 OR MIT` | Mads Marquart <mads@marquart.dk>, Mary <mary@mary.zone> | https://github.com/madsmtm/objc2 |
| `displaydoc` | `0.2.7` | `MIT OR Apache-2.0` | Jane Lusby <jlusby@yaah.dev> | https://github.com/yaahc/displaydoc |
| `dlopen2` | `0.8.2` | `MIT` | Szymon Wieloch <szymon.wieloch@gmail.com>, Ahmed Masud <ahmed.masud@saf.ai>, OpenByte <development.openbyte@gmail.com> | https://github.com/OpenByteDev/dlopen2 |
| `dlopen2_derive` | `0.4.3` | `MIT` | Szymon Wieloch <szymon.wieloch@gmail.com>, OpenByte <development.openbyte@gmail.com> | https://github.com/OpenByteDev/dlopen2 |
| `dom_query` | `0.28.0` | `MIT` | niklak <morgenpurple@gmail.com>, importcjj <importcjj@gmail.com> | https://github.com/niklak/dom_query |
| `dom_query` | `0.29.0` | `MIT` | niklak <morgenpurple@gmail.com>, importcjj <importcjj@gmail.com> | https://github.com/niklak/dom_query |
| `dpi` | `0.1.2` | `Apache-2.0 AND MIT` | See upstream crate manifest | https://github.com/rust-windowing/winit |
| `dtoa` | `1.0.11` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/dtoa |
| `dtoa-short` | `0.3.5` | `MPL-2.0` | Xidorn Quan <me@upsuper.org> | https://github.com/upsuper/dtoa-short |
| `dunce` | `1.0.5` | `CC0-1.0 OR MIT-0 OR Apache-2.0` | Kornel <kornel@geekhood.net> | https://gitlab.com/kornelski/dunce |
| `dyn-clone` | `1.0.20` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/dyn-clone |
| `embed-resource` | `3.0.12` | `MIT` | наб <nabijaczleweli@nabijaczleweli.xyz>, Cat Plus Plus <piotrlegnica@piotrl.pl>, Liigo <liigo@qq.com>, azyobuzin <azyobuzin@users.sourceforge.jp>, Peter Atashian <retep998@gmail.com>, pravic <ehysta@gmail.com>, Gabriel Majeri <gabriel.majeri6@gmail.com>, SonnyX, Johan Andersson <repi@repi.se>, Jordan Poles <jpdev.noreply@gmail.com>, MSxDOS <melcodos@gmail.com>, Jim McGrath <jimmc2@gmail.com>, roblabla <unfiltered@roblab.la>, Jasper Bekkers <jasper@traverseresearch.nl>, Richard Markiewicz <rmarkiewicz@devolutions.net>, Emerson de Freitas Barcelos <emersonfxbx@gmail.com>, Li Keqing <me@kaze.ai>, Alexis Bourget <alexis.bourget@gmail.com>, Michael Farrell <micolous+git@gmail.com>, Jacob Okamoto <oko@oko.io>, Marijn Suijten <marijn@traverseresearch.nl>, Lucas Nogueira <lucas@tauri.app>, CharlesChen0823 <yongchen0823@gmail.com>, Daniel Schaefer <dhs@frame.work>, Rene Leonhardt, ssrlive, Kan-Ru Chen <kanru@kanru.info>, Tony <legendmastertony@gmail.com>, Berrysoft <Strawberry_Str@hotmail.com>, Marcus Ahlberg <marcus.ahlberg@kvaser.com>, Thomas Eizinger <thomas@eizinger.io> | https://github.com/nabijaczleweli/rust-embed-resource |
| `embed_plist` | `1.2.2` | `MIT OR Apache-2.0` | Nikolai Vazquez <hello@nikolaivazquez.com> | https://github.com/nvzqz/embed-plist-rs |
| `equivalent` | `1.0.3` | `Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/indexmap-rs/equivalent |
| `erased-serde` | `0.4.11` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/erased-serde |
| `errno` | `0.3.14` | `MIT OR Apache-2.0` | Chris Wong <lambda.fairy@gmail.com>, Dan Gohman <dev@sunfishcode.online> | https://github.com/lambda-fairy/rust-errno |
| `fallible-iterator` | `0.3.0` | `MIT/Apache-2.0` | Steven Fackler <sfackler@gmail.com> | https://github.com/sfackler/rust-fallible-iterator |
| `fallible-streaming-iterator` | `0.1.9` | `MIT/Apache-2.0` | Steven Fackler <sfackler@gmail.com> | https://github.com/sfackler/fallible-streaming-iterator |
| `fastrand` | `2.5.0` | `Apache-2.0 OR MIT` | Stjepan Glavina <stjepang@gmail.com> | https://github.com/smol-rs/fastrand |
| `fdeflate` | `0.3.7` | `MIT OR Apache-2.0` | The image-rs Developers | https://github.com/image-rs/fdeflate |
| `field-offset` | `0.3.6` | `MIT OR Apache-2.0` | Diggory Blake <diggsey@googlemail.com> | https://github.com/Diggsey/rust-field-offset |
| `find-msvc-tools` | `0.1.15` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-lang/cc-rs |
| `flate2` | `1.1.11` | `MIT OR Apache-2.0` | Alex Crichton <alex@alexcrichton.com>, Josh Triplett <josh@joshtriplett.org> | https://github.com/rust-lang/flate2-rs |
| `fnv` | `1.0.7` | `Apache-2.0 / MIT` | Alex Crichton <alex@alexcrichton.com> | https://github.com/servo/rust-fnv |
| `foldhash` | `0.1.5` | `Zlib` | Orson Peters <orsonpeters@gmail.com> | https://github.com/orlp/foldhash |
| `foldhash` | `0.2.0` | `Zlib` | Orson Peters <orsonpeters@gmail.com> | https://github.com/orlp/foldhash |
| `foreign-types` | `0.5.0` | `MIT/Apache-2.0` | Steven Fackler <sfackler@gmail.com> | https://github.com/sfackler/foreign-types |
| `foreign-types-macros` | `0.2.4` | `MIT/Apache-2.0` | Steven Fackler <sfackler@gmail.com> | https://github.com/sfackler/foreign-types |
| `foreign-types-shared` | `0.3.1` | `MIT/Apache-2.0` | Steven Fackler <sfackler@gmail.com> | https://github.com/sfackler/foreign-types |
| `form_urlencoded` | `1.2.2` | `MIT OR Apache-2.0` | The rust-url developers | https://github.com/servo/rust-url |
| `futures-channel` | `0.3.34` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-lang/futures-rs |
| `futures-core` | `0.3.34` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-lang/futures-rs |
| `futures-executor` | `0.3.34` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-lang/futures-rs |
| `futures-io` | `0.3.34` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-lang/futures-rs |
| `futures-macro` | `0.3.34` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-lang/futures-rs |
| `futures-sink` | `0.3.34` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-lang/futures-rs |
| `futures-task` | `0.3.34` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-lang/futures-rs |
| `futures-util` | `0.3.34` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-lang/futures-rs |
| `gdk` | `0.18.2` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk3-rs |
| `gdk-pixbuf` | `0.18.5` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk-rs-core |
| `gdk-pixbuf-sys` | `0.18.0` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk-rs-core |
| `gdk-sys` | `0.18.2` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk3-rs |
| `gdkwayland-sys` | `0.18.2` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk3-rs |
| `gdkx11` | `0.18.2` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk3-rs |
| `gdkx11-sys` | `0.18.2` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk3-rs |
| `generic-array` | `0.14.7` | `MIT` | Bartłomiej Kamiński <fizyk20@gmail.com>, Aaron Trent <novacrazy@gmail.com> | https://github.com/fizyk20/generic-array.git |
| `getrandom` | `0.2.17` | `MIT OR Apache-2.0` | The Rand Project Developers | https://github.com/rust-random/getrandom |
| `getrandom` | `0.3.4` | `MIT OR Apache-2.0` | The Rand Project Developers | https://github.com/rust-random/getrandom |
| `getrandom` | `0.4.3` | `MIT OR Apache-2.0` | The Rand Project Developers | https://github.com/rust-random/getrandom |
| `gio` | `0.18.4` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk-rs-core |
| `gio-sys` | `0.18.1` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk-rs-core |
| `glib` | `0.18.5` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk-rs-core |
| `glib-macros` | `0.18.5` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk-rs-core |
| `glib-sys` | `0.18.1` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk-rs-core |
| `glob` | `0.3.4` | `MIT OR Apache-2.0` | The Rust Project Developers | https://github.com/rust-lang/glob |
| `globset` | `0.4.21` | `Unlicense OR MIT` | Andrew Gallant <jamslam@gmail.com> | https://github.com/BurntSushi/ripgrep/tree/master/crates/globset |
| `gobject-sys` | `0.18.0` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk-rs-core |
| `gtk` | `0.18.2` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk3-rs |
| `gtk-sys` | `0.18.2` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk3-rs |
| `gtk3-macros` | `0.18.2` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk3-rs |
| `hashbrown` | `0.12.3` | `MIT OR Apache-2.0` | Amanieu d'Antras <amanieu@gmail.com> | https://github.com/rust-lang/hashbrown |
| `hashbrown` | `0.15.5` | `MIT OR Apache-2.0` | Amanieu d'Antras <amanieu@gmail.com> | https://github.com/rust-lang/hashbrown |
| `hashbrown` | `0.17.1` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-lang/hashbrown |
| `hashlink` | `0.10.0` | `MIT OR Apache-2.0` | kyren <kerriganw@gmail.com> | https://github.com/kyren/hashlink |
| `heck` | `0.4.1` | `MIT OR Apache-2.0` | Without Boats <woboats@gmail.com> | https://github.com/withoutboats/heck |
| `heck` | `0.5.0` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/withoutboats/heck |
| `hex` | `0.4.3` | `MIT OR Apache-2.0` | KokaKiwi <kokakiwi@kokakiwi.net> | https://github.com/KokaKiwi/rust-hex |
| `html5ever` | `0.39.0` | `MIT OR Apache-2.0` | The html5ever Project Developers | https://github.com/servo/html5ever |
| `html5ever` | `0.40.1` | `MIT OR Apache-2.0` | The html5ever Project Developers | https://github.com/servo/html5ever |
| `http` | `1.5.0` | `MIT OR Apache-2.0` | Alex Crichton <alex@alexcrichton.com>, Carl Lerche <me@carllerche.com>, Sean McArthur <sean@seanmonstar.com> | https://github.com/hyperium/http |
| `http-body` | `1.1.0` | `MIT` | Carl Lerche <me@carllerche.com>, Lucio Franco <luciofranco14@gmail.com>, Sean McArthur <sean@seanmonstar.com> | https://github.com/hyperium/http-body |
| `http-body-util` | `0.1.5` | `MIT` | Carl Lerche <me@carllerche.com>, Lucio Franco <luciofranco14@gmail.com>, Sean McArthur <sean@seanmonstar.com> | https://github.com/hyperium/http-body |
| `httparse` | `1.10.1` | `MIT OR Apache-2.0` | Sean McArthur <sean@seanmonstar.com> | https://github.com/seanmonstar/httparse |
| `hyper` | `1.12.0` | `MIT` | Sean McArthur <sean@seanmonstar.com> | https://github.com/hyperium/hyper |
| `hyper-rustls` | `0.27.10` | `Apache-2.0 OR ISC OR MIT` | See upstream crate manifest | https://github.com/rustls/hyper-rustls |
| `hyper-util` | `0.1.21` | `MIT` | Sean McArthur <sean@seanmonstar.com> | https://github.com/hyperium/hyper-util |
| `iana-time-zone` | `0.1.65` | `MIT OR Apache-2.0` | Andrew Straw <strawman@astraw.com>, René Kijewski <rene.kijewski@fu-berlin.de>, Ryan Lopopolo <rjl@hyperbo.la> | https://github.com/strawlab/iana-time-zone |
| `iana-time-zone-haiku` | `0.1.2` | `MIT OR Apache-2.0` | René Kijewski <crates.io@k6i.de> | https://github.com/strawlab/iana-time-zone |
| `ico` | `0.6.0` | `MIT` | Matthew D. Steele <mdsteele@alum.mit.edu> | https://github.com/mdsteele/rust-ico |
| `icu_collections` | `2.4.0` | `Unicode-3.0` | The ICU4X Project Developers | https://github.com/unicode-org/icu4x |
| `icu_locale_core` | `2.4.0` | `Unicode-3.0` | The ICU4X Project Developers | https://github.com/unicode-org/icu4x |
| `icu_normalizer` | `2.4.0` | `Unicode-3.0` | The ICU4X Project Developers | https://github.com/unicode-org/icu4x |
| `icu_normalizer_data` | `2.4.0` | `Unicode-3.0` | The ICU4X Project Developers | https://github.com/unicode-org/icu4x |
| `icu_properties` | `2.4.0` | `Unicode-3.0` | The ICU4X Project Developers | https://github.com/unicode-org/icu4x |
| `icu_properties_data` | `2.4.0` | `Unicode-3.0` | The ICU4X Project Developers | https://github.com/unicode-org/icu4x |
| `icu_provider` | `2.4.0` | `Unicode-3.0` | The ICU4X Project Developers | https://github.com/unicode-org/icu4x |
| `ident_case` | `1.0.1` | `MIT/Apache-2.0` | Ted Driggs <ted.driggs@outlook.com> | https://github.com/TedDriggs/ident_case |
| `idna` | `1.1.0` | `MIT OR Apache-2.0` | The rust-url developers | https://github.com/servo/rust-url/ |
| `idna_adapter` | `1.2.2` | `Apache-2.0 OR MIT` | The rust-url developers | https://github.com/hsivonen/idna_adapter |
| `ignore` | `0.4.34` | `Unlicense OR MIT` | Andrew Gallant <jamslam@gmail.com> | https://github.com/BurntSushi/ripgrep/tree/master/crates/ignore |
| `indexmap` | `1.9.3` | `Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/bluss/indexmap |
| `indexmap` | `2.14.2` | `Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/indexmap-rs/indexmap |
| `infer` | `0.22.0` | `MIT` | Bojan <dbojan@gmail.com> | https://github.com/bojand/infer |
| `ipnet` | `2.12.2` | `MIT OR Apache-2.0` | Kris Price <kris@krisprice.nz> | https://github.com/krisprice/ipnet |
| `is_terminal_polyfill` | `1.70.2` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/polyfill-rs/is_terminal_polyfill |
| `itoa` | `1.0.19` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/itoa |
| `javascriptcore-rs` | `1.1.2` | `MIT` | See upstream crate manifest | https://github.com/tauri-apps/javascriptcore-rs |
| `javascriptcore-rs-sys` | `1.1.1` | `MIT` | The Gtk-rs Project Developers | https://github.com/tauri-apps/javascriptcore-rs |
| `jiff` | `0.2.43` | `Unlicense OR MIT` | Andrew Gallant <jamslam@gmail.com> | https://github.com/BurntSushi/jiff |
| `jiff-core` | `0.1.5` | `Unlicense OR MIT` | Andrew Gallant <jamslam@gmail.com> | https://github.com/BurntSushi/jiff |
| `jiff-static` | `0.2.43` | `Unlicense OR MIT` | Andrew Gallant <jamslam@gmail.com> | https://github.com/BurntSushi/jiff |
| `jiff-tzdb` | `0.1.9` | `Unlicense OR MIT` | Andrew Gallant <jamslam@gmail.com> | https://github.com/BurntSushi/jiff |
| `jiff-tzdb-platform` | `0.1.3` | `Unlicense OR MIT` | Andrew Gallant <jamslam@gmail.com> | https://github.com/BurntSushi/jiff |
| `jni` | `0.21.1` | `MIT/Apache-2.0` | Josh Chase <josh@prevoty.com> | https://github.com/jni-rs/jni-rs |
| `jni-sys` | `0.3.1` | `MIT OR Apache-2.0` | Steven Fackler <sfackler@gmail.com> | https://github.com/jni-rs/jni-sys |
| `jni-sys` | `0.4.1` | `MIT OR Apache-2.0` | Steven Fackler <sfackler@gmail.com>, Robert Bragg <robert@sixbynine.org> | https://github.com/jni-rs/jni-sys |
| `jni-sys-macros` | `0.4.1` | `MIT OR Apache-2.0` | Robert Bragg <robert@sixbynine.org> | https://github.com/jni-rs/jni-sys |
| `js-sys` | `0.3.107` | `MIT OR Apache-2.0` | The wasm-bindgen Developers | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/js-sys |
| `json-patch` | `4.2.0` | `MIT/Apache-2.0` | Ivan Dubrov <dubrov.ivan@gmail.com> | https://github.com/idubrov/json-patch |
| `jsonptr` | `0.7.1` | `MIT OR Apache-2.0` | chance dinkins, André Sá de Mello <codasm@pm.me>, Oliver Wangler <oliver@wngr.de> | https://github.com/chanced/jsonptr |
| `keyboard-types` | `0.8.3` | `MIT OR Apache-2.0` | Pyfisch <pyfisch@posteo.org> | https://github.com/rust-windowing/keyboard-types |
| `libappindicator` | `0.9.0` | `Apache-2.0 OR MIT` | See upstream crate manifest | N/A |
| `libappindicator-sys` | `0.9.0` | `Apache-2.0 OR MIT` | See upstream crate manifest | N/A |
| `libc` | `0.2.191` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-lang/libc |
| `libdbus-sys` | `0.2.7` | `Apache-2.0/MIT` | David Henningsson <diwic@ubuntu.com> | https://github.com/diwic/dbus-rs |
| `libloading` | `0.7.4` | `ISC` | Simonas Kazlauskas <libloading@kazlauskas.me> | https://github.com/nagisa/rust_libloading/ |
| `libredox` | `0.1.25` | `MIT` | 4lDO2 <4lDO2@protonmail.com> | https://gitlab.redox-os.org/redox-os/libredox.git |
| `libsqlite3-sys` | `0.31.0` | `MIT` | The rusqlite developers | https://github.com/rusqlite/rusqlite |
| `linux-raw-sys` | `0.12.1` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | Dan Gohman <dev@sunfishcode.online> | https://github.com/sunfishcode/linux-raw-sys |
| `litemap` | `0.8.3` | `Unicode-3.0` | The ICU4X Project Developers | https://github.com/unicode-org/icu4x |
| `lock_api` | `0.4.14` | `MIT OR Apache-2.0` | Amanieu d'Antras <amanieu@gmail.com> | https://github.com/Amanieu/parking_lot |
| `log` | `0.4.35` | `MIT OR Apache-2.0` | The Rust Project Developers | https://github.com/rust-lang/log |
| `lru-slab` | `0.1.3` | `MIT OR Apache-2.0 OR Zlib` | Benjamin Saunders <ben.e.saunders@gmail.com> | https://github.com/Ralith/lru-slab |
| `markup5ever` | `0.39.0` | `MIT OR Apache-2.0` | The html5ever Project Developers | https://github.com/servo/html5ever |
| `markup5ever` | `0.40.0` | `MIT OR Apache-2.0` | The html5ever Project Developers | https://github.com/servo/html5ever |
| `memchr` | `2.8.3` | `Unlicense OR MIT` | Andrew Gallant <jamslam@gmail.com>, bluss | https://github.com/BurntSushi/memchr |
| `memoffset` | `0.9.1` | `MIT` | Gilad Naaman <gilad.naaman@gmail.com> | https://github.com/Gilnaa/memoffset |
| `mime` | `0.3.18` | `MIT OR Apache-2.0` | Sean McArthur <sean@seanmonstar.com> | https://github.com/hyperium/mime |
| `miniz_oxide` | `0.8.9` | `MIT OR Zlib OR Apache-2.0` | Frommi <daniil.liferenko@gmail.com>, oyvindln <oyvindln@users.noreply.github.com>, Rich Geldreich richgel99@gmail.com | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide |
| `miniz_oxide` | `0.9.1` | `MIT OR Zlib OR Apache-2.0` | Frommi <daniil.liferenko@gmail.com>, oyvindln <oyvindln@users.noreply.github.com>, Rich Geldreich richgel99@gmail.com | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide |
| `mio` | `1.2.4` | `MIT` | Carl Lerche <me@carllerche.com>, Thomas de Zeeuw <thomasdezeeuw@gmail.com>, Tokio Contributors <team@tokio.rs> | https://github.com/tokio-rs/mio |
| `muda` | `0.20.0` | `Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/tauri-apps/muda |
| `ndk` | `0.9.0` | `MIT OR Apache-2.0` | The Rust Mobile contributors | https://github.com/rust-mobile/ndk |
| `ndk-context` | `0.1.1` | `MIT OR Apache-2.0` | The Rust Windowing contributors | https://github.com/rust-windowing/android-ndk-rs |
| `ndk-sys` | `0.6.0+11769913` | `MIT OR Apache-2.0` | The Rust Windowing contributors | https://github.com/rust-mobile/ndk |
| `new_debug_unreachable` | `1.0.6` | `MIT` | Matt Brubeck <mbrubeck@limpet.net>, Jonathan Reem <jonathan.reem@gmail.com> | https://github.com/mbrubeck/rust-debug-unreachable |
| `num-conv` | `0.2.2` | `MIT OR Apache-2.0` | Jacob Pratt <jacob@jhpratt.dev> | https://github.com/jhpratt/num-conv |
| `num-traits` | `0.2.19` | `MIT OR Apache-2.0` | The Rust Project Developers | https://github.com/rust-num/num-traits |
| `num_enum` | `0.7.6` | `BSD-3-Clause OR MIT OR Apache-2.0` | Daniel Wagner-Hall <dawagner@gmail.com>, Daniel Henry-Mantilla <daniel.henry.mantilla@gmail.com>, Vincent Esche <regexident@gmail.com> | https://github.com/illicitonion/num_enum |
| `num_enum_derive` | `0.7.6` | `BSD-3-Clause OR MIT OR Apache-2.0` | Daniel Wagner-Hall <dawagner@gmail.com>, Daniel Henry-Mantilla <daniel.henry.mantilla@gmail.com>, Vincent Esche <regexident@gmail.com> | https://github.com/illicitonion/num_enum |
| `objc2` | `0.6.5` | `MIT` | Mads Marquart <mads@marquart.dk> | https://github.com/madsmtm/objc2 |
| `objc2-app-kit` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/madsmtm/objc2 |
| `objc2-cloud-kit` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/madsmtm/objc2 |
| `objc2-core-data` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/madsmtm/objc2 |
| `objc2-core-foundation` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/madsmtm/objc2 |
| `objc2-core-graphics` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/madsmtm/objc2 |
| `objc2-core-image` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/madsmtm/objc2 |
| `objc2-core-location` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/madsmtm/objc2 |
| `objc2-core-text` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/madsmtm/objc2 |
| `objc2-encode` | `4.1.0` | `MIT` | Mads Marquart <mads@marquart.dk> | https://github.com/madsmtm/objc2 |
| `objc2-exception-helper` | `0.1.1` | `Zlib OR Apache-2.0 OR MIT` | Mads Marquart <mads@marquart.dk> | https://github.com/madsmtm/objc2 |
| `objc2-foundation` | `0.3.2` | `MIT` | See upstream crate manifest | https://github.com/madsmtm/objc2 |
| `objc2-io-surface` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/madsmtm/objc2 |
| `objc2-quartz-core` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/madsmtm/objc2 |
| `objc2-ui-kit` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/madsmtm/objc2 |
| `objc2-user-notifications` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/madsmtm/objc2 |
| `objc2-web-kit` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/madsmtm/objc2 |
| `once_cell` | `1.21.4` | `MIT OR Apache-2.0` | Aleksey Kladov <aleksey.kladov@gmail.com> | https://github.com/matklad/once_cell |
| `once_cell_polyfill` | `1.70.2` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/polyfill-rs/once_cell_polyfill |
| `openssl-probe` | `0.2.2` | `MIT OR Apache-2.0` | Alex Crichton <alex@alexcrichton.com> | https://github.com/rustls/openssl-probe |
| `option-ext` | `0.2.0` | `MPL-2.0` | Simon Ochsenreither <simon@ochsenreither.de> | https://github.com/soc/option-ext.git |
| `pango` | `0.18.3` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk-rs-core |
| `pango-sys` | `0.18.0` | `MIT` | The gtk-rs Project Developers | https://github.com/gtk-rs/gtk-rs-core |
| `parking_lot` | `0.12.5` | `MIT OR Apache-2.0` | Amanieu d'Antras <amanieu@gmail.com> | https://github.com/Amanieu/parking_lot |
| `parking_lot_core` | `0.9.12` | `MIT OR Apache-2.0` | Amanieu d'Antras <amanieu@gmail.com> | https://github.com/Amanieu/parking_lot |
| `percent-encoding` | `2.3.2` | `MIT OR Apache-2.0` | The rust-url developers | https://github.com/servo/rust-url/ |
| `phf` | `0.13.1` | `MIT` | Steven Fackler <sfackler@gmail.com> | https://github.com/rust-phf/rust-phf |
| `phf` | `0.14.0` | `MIT` | Steven Fackler <sfackler@gmail.com> | https://github.com/rust-phf/rust-phf |
| `phf_codegen` | `0.13.1` | `MIT` | Steven Fackler <sfackler@gmail.com> | https://github.com/rust-phf/rust-phf |
| `phf_codegen` | `0.14.0` | `MIT` | Steven Fackler <sfackler@gmail.com> | https://github.com/rust-phf/rust-phf |
| `phf_generator` | `0.13.1` | `MIT` | Steven Fackler <sfackler@gmail.com> | https://github.com/rust-phf/rust-phf |
| `phf_generator` | `0.14.0` | `MIT` | Steven Fackler <sfackler@gmail.com> | https://github.com/rust-phf/rust-phf |
| `phf_macros` | `0.13.1` | `MIT` | Steven Fackler <sfackler@gmail.com> | https://github.com/rust-phf/rust-phf |
| `phf_macros` | `0.14.0` | `MIT` | Steven Fackler <sfackler@gmail.com> | https://github.com/rust-phf/rust-phf |
| `phf_shared` | `0.13.1` | `MIT` | Steven Fackler <sfackler@gmail.com> | https://github.com/rust-phf/rust-phf |
| `phf_shared` | `0.14.0` | `MIT` | Steven Fackler <sfackler@gmail.com> | https://github.com/rust-phf/rust-phf |
| `pin-project-lite` | `0.2.17` | `Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/taiki-e/pin-project-lite |
| `pkg-config` | `0.3.34` | `MIT OR Apache-2.0` | Alex Crichton <alex@alexcrichton.com> | https://github.com/rust-lang/pkg-config-rs |
| `plist` | `1.10.3` | `MIT` | Ed Barnard <eabarnard@gmail.com> | https://github.com/ebarnard/rust-plist/ |
| `png` | `0.18.1` | `MIT OR Apache-2.0` | The image-rs Developers | https://github.com/image-rs/image-png |
| `portable-atomic` | `1.15.0` | `Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/taiki-e/portable-atomic |
| `portable-atomic-util` | `0.2.8` | `Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/taiki-e/portable-atomic-util |
| `potential_utf` | `0.1.6` | `Unicode-3.0` | The ICU4X Project Developers | https://github.com/unicode-org/icu4x |
| `powerfmt` | `0.2.1` | `MIT OR Apache-2.0` | Jacob Pratt <jacob@jhpratt.dev> | https://github.com/jhpratt/powerfmt |
| `precomputed-hash` | `0.1.1` | `MIT` | Emilio Cobos Álvarez <emilio@crisal.io> | https://github.com/emilio/precomputed-hash |
| `proc-macro-crate` | `1.3.1` | `MIT OR Apache-2.0` | Bastian Köcher <git@kchr.de> | https://github.com/bkchr/proc-macro-crate |
| `proc-macro-crate` | `2.0.2` | `MIT OR Apache-2.0` | Bastian Köcher <git@kchr.de> | https://github.com/bkchr/proc-macro-crate |
| `proc-macro-crate` | `3.5.0` | `MIT OR Apache-2.0` | Bastian Köcher <git@kchr.de> | https://github.com/bkchr/proc-macro-crate |
| `proc-macro-error` | `1.0.4` | `MIT OR Apache-2.0` | CreepySkeleton <creepy-skeleton@yandex.ru> | https://gitlab.com/CreepySkeleton/proc-macro-error |
| `proc-macro-error-attr` | `1.0.4` | `MIT OR Apache-2.0` | CreepySkeleton <creepy-skeleton@yandex.ru> | https://gitlab.com/CreepySkeleton/proc-macro-error |
| `proc-macro2` | `1.0.107` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com>, Alex Crichton <alex@alexcrichton.com> | https://github.com/dtolnay/proc-macro2 |
| `quick-xml` | `0.42.0` | `MIT` | See upstream crate manifest | https://github.com/tafia/quick-xml |
| `quinn` | `0.11.12` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/quinn-rs/quinn |
| `quinn-proto` | `0.11.19` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/quinn-rs/quinn |
| `quinn-udp` | `0.5.16` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/quinn-rs/quinn |
| `quote` | `1.0.47` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/quote |
| `r-efi` | `5.3.0` | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | See upstream crate manifest | https://github.com/r-efi/r-efi |
| `r-efi` | `6.0.0` | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | See upstream crate manifest | https://github.com/r-efi/r-efi |
| `rand` | `0.10.3` | `MIT OR Apache-2.0` | The Rand Project Developers, The Rust Project Developers | https://github.com/rust-random/rand |
| `rand_core` | `0.10.1` | `MIT OR Apache-2.0` | The Rand Project Developers | https://github.com/rust-random/rand_core |
| `rand_pcg` | `0.10.2` | `MIT OR Apache-2.0` | The Rand Project Developers | https://github.com/rust-random/rngs |
| `raw-window-handle` | `0.6.2` | `MIT OR Apache-2.0 OR Zlib` | Osspial <osspial@gmail.com> | https://github.com/rust-windowing/raw-window-handle |
| `redox_syscall` | `0.5.18` | `MIT` | Jeremy Soller <jackpot51@gmail.com> | https://gitlab.redox-os.org/redox-os/syscall |
| `redox_users` | `0.5.3` | `MIT` | Jose Narvaez <goyox86@gmail.com>, Wesley Hershberger <mggmugginsmc@gmail.com> | https://gitlab.redox-os.org/redox-os/users |
| `ref-cast` | `1.0.27` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/ref-cast |
| `ref-cast-impl` | `1.0.27` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/ref-cast |
| `regex` | `1.13.1` | `MIT OR Apache-2.0` | The Rust Project Developers, Andrew Gallant <jamslam@gmail.com> | https://github.com/rust-lang/regex |
| `regex-automata` | `0.4.18` | `MIT OR Apache-2.0` | The Rust Project Developers, Andrew Gallant <jamslam@gmail.com> | https://github.com/rust-lang/regex |
| `regex-syntax` | `0.8.11` | `MIT OR Apache-2.0` | The Rust Project Developers, Andrew Gallant <jamslam@gmail.com> | https://github.com/rust-lang/regex |
| `reqwest` | `0.12.28` | `MIT OR Apache-2.0` | Sean McArthur <sean@seanmonstar.com> | https://github.com/seanmonstar/reqwest |
| `reqwest` | `0.13.5` | `MIT OR Apache-2.0` | Sean McArthur <sean@seanmonstar.com> | https://github.com/seanmonstar/reqwest |
| `ring` | `0.17.14` | `Apache-2.0 AND ISC` | See upstream crate manifest | https://github.com/briansmith/ring |
| `rusqlite` | `0.33.0` | `MIT` | The rusqlite developers | https://github.com/rusqlite/rusqlite |
| `rustc-hash` | `2.1.3` | `Apache-2.0 OR MIT` | The Rust Project Developers | https://github.com/rust-lang/rustc-hash |
| `rustc_version` | `0.4.1` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/djc/rustc-version-rs |
| `rustix` | `1.1.6` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | Dan Gohman <dev@sunfishcode.online>, Jakub Konka <kubkon@jakubkonka.com> | https://github.com/bytecodealliance/rustix |
| `rustls` | `0.23.46` | `Apache-2.0 OR ISC OR MIT` | See upstream crate manifest | https://github.com/rustls/rustls |
| `rustls-native-certs` | `0.8.5` | `Apache-2.0 OR ISC OR MIT` | See upstream crate manifest | https://github.com/rustls/rustls-native-certs |
| `rustls-pki-types` | `1.16.0` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rustls/pki-types |
| `rustls-webpki` | `0.103.16` | `ISC` | See upstream crate manifest | https://github.com/rustls/webpki |
| `rustversion` | `1.0.23` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/rustversion |
| `ryu` | `1.0.23` | `Apache-2.0 OR BSL-1.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/ryu |
| `same-file` | `1.0.6` | `Unlicense/MIT` | Andrew Gallant <jamslam@gmail.com> | https://github.com/BurntSushi/same-file |
| `schannel` | `0.1.30` | `MIT` | Steven Fackler <sfackler@gmail.com>, Steffen Butzer <steffen.butzer@outlook.com> | https://github.com/steffengy/schannel-rs |
| `schemars` | `0.8.22` | `MIT` | Graham Esau <gesau@hotmail.co.uk> | https://github.com/GREsau/schemars |
| `schemars` | `0.9.0` | `MIT` | Graham Esau <gesau@hotmail.co.uk> | https://github.com/GREsau/schemars |
| `schemars` | `1.2.3` | `MIT` | Graham Esau <gesau@hotmail.co.uk> | https://github.com/GREsau/schemars |
| `schemars_derive` | `0.8.22` | `MIT` | Graham Esau <gesau@hotmail.co.uk> | https://github.com/GREsau/schemars |
| `scopeguard` | `1.2.0` | `MIT OR Apache-2.0` | bluss | https://github.com/bluss/scopeguard |
| `security-framework` | `3.7.0` | `MIT OR Apache-2.0` | Steven Fackler <sfackler@gmail.com>, Kornel <kornel@geekhood.net> | https://github.com/kornelski/rust-security-framework |
| `security-framework-sys` | `2.17.0` | `MIT OR Apache-2.0` | Steven Fackler <sfackler@gmail.com>, Kornel <kornel@geekhood.net> | https://github.com/kornelski/rust-security-framework |
| `selectors` | `0.38.0` | `MPL-2.0` | The Servo Project Developers | https://github.com/servo/stylo |
| `selectors` | `0.41.0` | `MPL-2.0` | The Servo Project Developers | https://github.com/servo/stylo |
| `semver` | `1.0.28` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/semver |
| `serde` | `1.0.229` | `MIT OR Apache-2.0` | Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com> | https://github.com/serde-rs/serde |
| `serde-untagged` | `0.1.9` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/serde-untagged |
| `serde_core` | `1.0.229` | `MIT OR Apache-2.0` | Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com> | https://github.com/serde-rs/serde |
| `serde_derive` | `1.0.229` | `MIT OR Apache-2.0` | Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com> | https://github.com/serde-rs/serde |
| `serde_derive_internals` | `0.29.1` | `MIT OR Apache-2.0` | Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com> | https://github.com/serde-rs/serde |
| `serde_json` | `1.0.154` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/serde-rs/json |
| `serde_repr` | `0.1.21` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/serde-repr |
| `serde_spanned` | `0.6.9` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/toml-rs/toml |
| `serde_spanned` | `1.1.3` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/toml-rs/toml |
| `serde_urlencoded` | `0.7.1` | `MIT/Apache-2.0` | Anthony Ramine <n.oxyde@gmail.com> | https://github.com/nox/serde_urlencoded |
| `serde_with` | `3.24.0` | `MIT OR Apache-2.0` | Jonas Bushart, Marcin Kaźmierczak | https://github.com/jonasbb/serde_with/ |
| `serde_with_macros` | `3.24.0` | `MIT OR Apache-2.0` | Jonas Bushart | https://github.com/jonasbb/serde_with/ |
| `serialize-to-javascript` | `0.1.2` | `MIT OR Apache-2.0` | Chip Reed <chip@chip.sh> | https://github.com/chippers/serialize-to-javascript |
| `serialize-to-javascript-impl` | `0.1.2` | `MIT OR Apache-2.0` | Chip Reed <chip@chip.sh> | https://github.com/chippers/serialize-to-javascript |
| `servo_arc` | `0.4.3` | `MIT OR Apache-2.0` | The Servo Project Developers | https://github.com/servo/stylo |
| `servo_arc` | `0.5.0` | `MIT OR Apache-2.0` | The Servo Project Developers | https://github.com/servo/stylo |
| `sha2` | `0.10.9` | `MIT OR Apache-2.0` | RustCrypto Developers | https://github.com/RustCrypto/hashes |
| `shlex` | `2.0.1` | `MIT OR Apache-2.0` | comex <comexk@gmail.com>, Fenhl <fenhl@fenhl.net>, Adrian Taylor <adetaylor@chromium.org>, Alex Touchet <alextouchet@outlook.com>, Daniel Parks <dp+git@oxidized.org>, Garrett Berg <googberg@gmail.com> | https://github.com/comex/rust-shlex |
| `signal-hook-registry` | `1.4.8` | `MIT OR Apache-2.0` | Michal 'vorner' Vaner <vorner@vorner.cz>, Masaki Hara <ackie.h.gmai@gmail.com> | https://github.com/vorner/signal-hook |
| `simd-adler32` | `0.3.10` | `MIT` | Marvin Countryman <me@maar.vin> | https://github.com/mcountryman/simd-adler32 |
| `similar` | `2.7.0` | `Apache-2.0` | Armin Ronacher <armin.ronacher@active-4.com>, Pierre-Étienne Meunier <pe@pijul.org>, Brandon Williams <bwilliams.eng@gmail.com> | https://github.com/mitsuhiko/similar |
| `siphasher` | `1.0.4` | `MIT OR Apache-2.0` | Frank Denis <github@pureftpd.org> | https://github.com/jedisct1/rust-siphash |
| `slab` | `0.4.12` | `MIT` | Carl Lerche <me@carllerche.com> | https://github.com/tokio-rs/slab |
| `smallvec` | `1.16.5` | `MIT OR Apache-2.0` | The Servo Project Developers | https://github.com/servo/rust-smallvec |
| `socket2` | `0.6.5` | `MIT OR Apache-2.0` | Alex Crichton <alex@alexcrichton.com>, Thomas de Zeeuw <thomasdezeeuw@gmail.com> | https://github.com/rust-lang/socket2 |
| `softbuffer` | `0.4.8` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rust-windowing/softbuffer |
| `soup3` | `0.5.0` | `MIT` | See upstream crate manifest | https://gitlab.gnome.org/World/Rust/soup3-rs |
| `soup3-sys` | `0.5.0` | `MIT` | The Gtk-rs Project Developers | https://gitlab.gnome.org/World/Rust/soup3-rs |
| `stable_deref_trait` | `1.2.1` | `MIT OR Apache-2.0` | Robert Grosse <n210241048576@gmail.com> | https://github.com/storyyeller/stable_deref_trait |
| `static_vcruntime` | `3.0.0` | `MIT OR Apache-2.0 OR Zlib` | See upstream crate manifest | https://github.com/ChrisDenton/static_vcruntime |
| `string_cache` | `0.11.0` | `MIT OR Apache-2.0` | The Servo Project Developers | https://github.com/servo/string-cache |
| `string_cache` | `0.9.0` | `MIT OR Apache-2.0` | The Servo Project Developers | https://github.com/servo/string-cache |
| `string_cache_codegen` | `0.11.2` | `MIT OR Apache-2.0` | The Servo Project Developers | https://github.com/servo/string-cache |
| `string_cache_codegen` | `0.6.1` | `MIT OR Apache-2.0` | The Servo Project Developers | https://github.com/servo/string-cache |
| `strsim` | `0.11.1` | `MIT` | Danny Guo <danny@dannyguo.com>, maxbachmann <oss@maxbachmann.de> | https://github.com/rapidfuzz/strsim-rs |
| `subtle` | `2.6.1` | `BSD-3-Clause` | Isis Lovecruft <isis@patternsinthevoid.net>, Henry de Valence <hdevalence@hdevalence.ca> | https://github.com/dalek-cryptography/subtle |
| `swift-rs` | `1.0.9` | `MIT OR Apache-2.0` | The swift-rs contributors | https://github.com/Brendonovich/swift-rs |
| `syn` | `1.0.109` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/syn |
| `syn` | `2.0.119` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/syn |
| `syn` | `3.0.10` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/syn |
| `sync_wrapper` | `1.0.2` | `Apache-2.0` | Actyx AG <developer@actyx.io> | https://github.com/Actyx/sync_wrapper |
| `synstructure` | `0.14.0` | `MIT` | Nika Layzell <nika@thelayzells.com> | https://github.com/mystor/synstructure |
| `system-deps` | `6.2.2` | `MIT OR Apache-2.0` | Guillaume Desmottes <guillaume.desmottes@collabora.com>, Josh Triplett <josh@joshtriplett.org> | https://github.com/gdesmott/system-deps |
| `tao` | `0.37.2` | `Apache-2.0` | Tauri Programme within The Commons Conservancy, The winit contributors | https://github.com/tauri-apps/tao |
| `tao-macros` | `0.1.4` | `MIT OR Apache-2.0` | Tauri Programme within The Commons Conservancy | https://github.com/tauri-apps/tao |
| `target-lexicon` | `0.12.16` | `Apache-2.0 WITH LLVM-exception` | Dan Gohman <sunfish@mozilla.com> | https://github.com/bytecodealliance/target-lexicon |
| `tauri` | `2.12.5` | `Apache-2.0 OR MIT` | Tauri Programme within The Commons Conservancy | https://github.com/tauri-apps/tauri |
| `tauri-build` | `2.7.4` | `Apache-2.0 OR MIT` | Tauri Programme within The Commons Conservancy | https://github.com/tauri-apps/tauri |
| `tauri-codegen` | `2.7.4` | `Apache-2.0 OR MIT` | Tauri Programme within The Commons Conservancy | https://github.com/tauri-apps/tauri |
| `tauri-macros` | `2.7.4` | `Apache-2.0 OR MIT` | Tauri Programme within The Commons Conservancy | https://github.com/tauri-apps/tauri |
| `tauri-runtime` | `2.12.2` | `Apache-2.0 OR MIT` | Tauri Programme within The Commons Conservancy | https://github.com/tauri-apps/tauri |
| `tauri-runtime-wry` | `2.12.3` | `Apache-2.0 OR MIT` | Tauri Programme within The Commons Conservancy | https://github.com/tauri-apps/tauri |
| `tauri-utils` | `2.10.2` | `Apache-2.0 OR MIT` | Tauri Programme within The Commons Conservancy | https://github.com/tauri-apps/tauri |
| `tauri-winres` | `0.3.7` | `MIT` | Tauri Programme within The Commons Conservancy, Max Resch <resch.max@gmail.com> | https://github.com/tauri-apps/winres |
| `tempfile` | `3.27.0` | `MIT OR Apache-2.0` | Steven Allen <steven@stebalien.com>, The Rust Project Developers, Ashley Mannix <ashleymannix@live.com.au>, Jason White <me@jasonwhite.io> | https://github.com/Stebalien/tempfile |
| `tendril` | `0.5.1` | `MIT OR Apache-2.0` | Keegan McAllister <mcallister.keegan@gmail.com>, Simon Sapin <simon.sapin@exyr.org>, Chris Morgan <me@chrismorgan.info> | https://github.com/servo/html5ever |
| `thiserror` | `1.0.69` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/thiserror |
| `thiserror` | `2.0.21` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/thiserror |
| `thiserror-impl` | `1.0.69` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/thiserror |
| `thiserror-impl` | `2.0.21` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/thiserror |
| `time` | `0.3.55` | `MIT OR Apache-2.0` | Jacob Pratt <open-source@jhpratt.dev>, Time contributors | https://github.com/time-rs/time |
| `time-core` | `0.1.9` | `MIT OR Apache-2.0` | Jacob Pratt <open-source@jhpratt.dev>, Time contributors | https://github.com/time-rs/time |
| `time-macros` | `0.2.32` | `MIT OR Apache-2.0` | Jacob Pratt <open-source@jhpratt.dev>, Time contributors | https://github.com/time-rs/time |
| `tinystr` | `0.8.5` | `Unicode-3.0` | The ICU4X Project Developers | https://github.com/unicode-org/icu4x |
| `tinyvec` | `1.13.3` | `Zlib OR Apache-2.0 OR MIT` | Lokathor <zefria@gmail.com> | https://github.com/Lokathor/tinyvec |
| `tokio` | `1.54.1` | `MIT` | Tokio Contributors <team@tokio.rs> | https://github.com/tokio-rs/tokio |
| `tokio-macros` | `2.7.2` | `MIT` | Tokio Contributors <team@tokio.rs> | https://github.com/tokio-rs/tokio |
| `tokio-rustls` | `0.26.6` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/rustls/tokio-rustls |
| `tokio-util` | `0.7.20` | `MIT` | Tokio Contributors <team@tokio.rs> | https://github.com/tokio-rs/tokio |
| `toml` | `0.8.2` | `MIT OR Apache-2.0` | Alex Crichton <alex@alexcrichton.com> | https://github.com/toml-rs/toml |
| `toml` | `1.2.1+spec-1.1.0` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/toml-rs/toml |
| `toml_datetime` | `0.6.3` | `MIT OR Apache-2.0` | Alex Crichton <alex@alexcrichton.com> | https://github.com/toml-rs/toml |
| `toml_datetime` | `1.1.3+spec-1.1.0` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/toml-rs/toml |
| `toml_edit` | `0.19.15` | `MIT OR Apache-2.0` | Andronik Ordian <write@reusable.software>, Ed Page <eopage@gmail.com> | https://github.com/toml-rs/toml |
| `toml_edit` | `0.20.2` | `MIT OR Apache-2.0` | Andronik Ordian <write@reusable.software>, Ed Page <eopage@gmail.com> | https://github.com/toml-rs/toml |
| `toml_edit` | `0.25.19+spec-1.1.0` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/toml-rs/toml |
| `toml_parser` | `1.1.5+spec-1.1.0` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/toml-rs/toml |
| `toml_writer` | `1.2.0+spec-1.1.0` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/toml-rs/toml |
| `tower` | `0.5.3` | `MIT` | Tower Maintainers <team@tower-rs.com> | https://github.com/tower-rs/tower |
| `tower-http` | `0.6.11` | `MIT` | Tower Maintainers <team@tower-rs.com> | https://github.com/tower-rs/tower-http |
| `tower-layer` | `0.3.3` | `MIT` | Tower Maintainers <team@tower-rs.com> | https://github.com/tower-rs/tower |
| `tower-service` | `0.3.3` | `MIT` | Tower Maintainers <team@tower-rs.com> | https://github.com/tower-rs/tower |
| `tracing` | `0.1.44` | `MIT` | Eliza Weisman <eliza@buoyant.io>, Tokio Contributors <team@tokio.rs> | https://github.com/tokio-rs/tracing |
| `tracing-core` | `0.1.36` | `MIT` | Tokio Contributors <team@tokio.rs> | https://github.com/tokio-rs/tracing |
| `tray-icon` | `0.25.1` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/tauri-apps/tray-icon |
| `try-lock` | `0.2.5` | `MIT` | Sean McArthur <sean@seanmonstar.com> | https://github.com/seanmonstar/try-lock |
| `typeid` | `1.0.3` | `MIT OR Apache-2.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/typeid |
| `typenum` | `1.20.1` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/paholg/typenum |
| `unicode-ident` | `1.0.27` | `(MIT OR Apache-2.0) AND Unicode-3.0` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/unicode-ident |
| `unicode-segmentation` | `1.13.3` | `MIT OR Apache-2.0` | kwantam <kwantam@gmail.com>, Manish Goregaokar <manishsmail@gmail.com> | https://github.com/unicode-rs/unicode-segmentation |
| `untrusted` | `0.9.0` | `ISC` | Brian Smith <brian@briansmith.org> | https://github.com/briansmith/untrusted |
| `url` | `2.5.8` | `MIT OR Apache-2.0` | The rust-url developers | https://github.com/servo/rust-url |
| `urlpattern` | `0.6.0` | `MIT` | the Deno authors, crowlKats <crowlkats@toaxl.com> | https://github.com/denoland/rust-urlpattern |
| `utf8_iter` | `1.1.0` | `Apache-2.0 OR MIT` | Henri Sivonen <hsivonen@hsivonen.fi> | https://github.com/hsivonen/utf8_iter |
| `utf8parse` | `0.2.2` | `Apache-2.0 OR MIT` | Joe Wilm <joe@jwilm.com>, Christian Duerr <contact@christianduerr.com> | https://github.com/alacritty/vte |
| `utf_types` | `1.0.0` | `Apache-2.0 OR MIT` | Henri Sivonen <hsivonen@hsivonen.fi> | https://github.com/hsivonen/utf_types |
| `uuid` | `1.29.2` | `Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/uuid-rs/uuid |
| `vcpkg` | `0.2.15` | `MIT/Apache-2.0` | Jim McGrath <jimmc2@gmail.com> | https://github.com/mcgoo/vcpkg-rs |
| `version-compare` | `0.2.1` | `MIT` | Tim Visee <3a4fb3964f@sinenomine.email> | https://gitlab.com/timvisee/version-compare |
| `version_check` | `0.9.5` | `MIT/Apache-2.0` | Sergio Benitez <sb@sergio.bz> | https://github.com/SergioBenitez/version_check |
| `vswhom` | `0.1.0` | `MIT` | nabijaczleweli <nabijaczleweli@gmail.com> | https://github.com/nabijaczleweli/vswhom.rs |
| `vswhom-sys` | `0.1.3` | `MIT` | наб <nabijaczleweli@nabijaczleweli.xyz>, forrestsmithfb <forrest.smith@fb.com> | https://github.com/nabijaczleweli/vswhom-sys.rs |
| `walkdir` | `2.5.0` | `Unlicense/MIT` | Andrew Gallant <jamslam@gmail.com> | https://github.com/BurntSushi/walkdir |
| `want` | `0.3.2` | `MIT` | Sean McArthur <sean@seanmonstar.com> | https://github.com/seanmonstar/want |
| `wasi` | `0.11.1+wasi-snapshot-preview1` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | The Cranelift Project Developers | https://github.com/bytecodealliance/wasi |
| `wasip2` | `1.0.4+wasi-0.2.12` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | See upstream crate manifest | https://github.com/bytecodealliance/wasi-rs |
| `wasm-bindgen` | `0.2.130` | `MIT OR Apache-2.0` | The wasm-bindgen Developers | https://github.com/wasm-bindgen/wasm-bindgen |
| `wasm-bindgen-futures` | `0.4.80` | `MIT OR Apache-2.0` | The wasm-bindgen Developers | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/futures |
| `wasm-bindgen-macro` | `0.2.130` | `MIT OR Apache-2.0` | The wasm-bindgen Developers | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/macro |
| `wasm-bindgen-macro-support` | `0.2.130` | `MIT OR Apache-2.0` | The wasm-bindgen Developers | https://github.com/wasm-bindgen/wasm-bindgen/tree/main/crates/macro-support |
| `wasm-bindgen-shared` | `0.2.130` | `MIT OR Apache-2.0` | The wasm-bindgen Developers | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/shared |
| `wasm-streams` | `0.5.0` | `MIT OR Apache-2.0` | Mattias Buelens <mattias@buelens.com> | https://github.com/MattiasBuelens/wasm-streams/ |
| `web-sys` | `0.3.107` | `MIT OR Apache-2.0` | The wasm-bindgen Developers | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/web-sys |
| `web-time` | `1.1.0` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/daxpedda/web-time |
| `web_atoms` | `0.2.6` | `MIT OR Apache-2.0` | The html5ever Project Developers | https://github.com/servo/html5ever |
| `web_atoms` | `0.3.0` | `MIT OR Apache-2.0` | The html5ever Project Developers | https://github.com/servo/html5ever |
| `webkit2gtk` | `2.0.2` | `MIT` | See upstream crate manifest | https://github.com/tauri-apps/webkit2gtk-rs |
| `webkit2gtk-sys` | `2.0.2` | `MIT` | See upstream crate manifest | https://github.com/tauri-apps/webkit2gtk-rs |
| `webview2-com` | `0.39.1` | `MIT` | See upstream crate manifest | https://github.com/wravery/webview2-rs |
| `webview2-com-macros` | `0.8.1` | `MIT` | See upstream crate manifest | https://github.com/wravery/webview2-rs |
| `webview2-com-sys` | `0.39.1` | `MIT` | See upstream crate manifest | https://github.com/wravery/webview2-rs |
| `winapi` | `0.3.9` | `MIT/Apache-2.0` | Peter Atashian <retep998@gmail.com> | https://github.com/retep998/winapi-rs |
| `winapi-i686-pc-windows-gnu` | `0.4.0` | `MIT/Apache-2.0` | Peter Atashian <retep998@gmail.com> | https://github.com/retep998/winapi-rs |
| `winapi-util` | `0.1.11` | `Unlicense OR MIT` | Andrew Gallant <jamslam@gmail.com> | https://github.com/BurntSushi/winapi-util |
| `winapi-x86_64-pc-windows-gnu` | `0.4.0` | `MIT/Apache-2.0` | Peter Atashian <retep998@gmail.com> | https://github.com/retep998/winapi-rs |
| `window-vibrancy` | `0.8.1` | `Apache-2.0 OR MIT` | Tauri Programme within The Commons Conservancy | https://github.com/tauri-apps/tauri-plugin-vibrancy |
| `windows` | `0.62.2` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/microsoft/windows-rs |
| `windows-collections` | `0.3.2` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/microsoft/windows-rs |
| `windows-core` | `0.62.2` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/microsoft/windows-rs |
| `windows-future` | `0.3.2` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/microsoft/windows-rs |
| `windows-implement` | `0.60.2` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/microsoft/windows-rs |
| `windows-interface` | `0.59.3` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/microsoft/windows-rs |
| `windows-link` | `0.2.1` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/microsoft/windows-rs |
| `windows-numerics` | `0.3.1` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/microsoft/windows-rs |
| `windows-result` | `0.4.1` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/microsoft/windows-rs |
| `windows-strings` | `0.5.1` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/microsoft/windows-rs |
| `windows-sys` | `0.45.0` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows-sys` | `0.52.0` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows-sys` | `0.61.2` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/microsoft/windows-rs |
| `windows-targets` | `0.42.2` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows-targets` | `0.52.6` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows-threading` | `0.2.1` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/microsoft/windows-rs |
| `windows-version` | `0.1.7` | `MIT OR Apache-2.0` | See upstream crate manifest | https://github.com/microsoft/windows-rs |
| `windows_aarch64_gnullvm` | `0.42.2` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows_aarch64_gnullvm` | `0.52.6` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows_aarch64_msvc` | `0.42.2` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows_aarch64_msvc` | `0.52.6` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows_i686_gnu` | `0.42.2` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows_i686_gnu` | `0.52.6` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows_i686_gnullvm` | `0.52.6` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows_i686_msvc` | `0.42.2` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows_i686_msvc` | `0.52.6` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows_x86_64_gnu` | `0.42.2` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows_x86_64_gnu` | `0.52.6` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows_x86_64_gnullvm` | `0.42.2` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows_x86_64_gnullvm` | `0.52.6` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows_x86_64_msvc` | `0.42.2` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `windows_x86_64_msvc` | `0.52.6` | `MIT OR Apache-2.0` | Microsoft | https://github.com/microsoft/windows-rs |
| `winnow` | `0.5.40` | `MIT` | See upstream crate manifest | https://github.com/winnow-rs/winnow |
| `winnow` | `1.0.5` | `MIT` | See upstream crate manifest | https://github.com/winnow-rs/winnow |
| `winreg` | `0.56.0` | `MIT` | Igor Shaula <gentoo90@gmail.com> | https://github.com/gentoo90/winreg-rs |
| `wit-bindgen` | `0.57.1` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | Alex Crichton <alex@alexcrichton.com> | https://github.com/bytecodealliance/wit-bindgen |
| `writeable` | `0.6.4` | `Unicode-3.0` | The ICU4X Project Developers | https://github.com/unicode-org/icu4x |
| `wry` | `0.57.2` | `Apache-2.0 OR MIT` | Tauri Programme within The Commons Conservancy | https://github.com/tauri-apps/wry |
| `x11` | `2.21.0` | `MIT` | daggerbot <daggerbot@gmail.com>, Erle Pereira <erle@erlepereira.com>, AltF02 <contact@altf2.dev> | https://github.com/AltF02/x11-rs.git |
| `x11-dl` | `2.21.0` | `MIT` | daggerbot <daggerbot@gmail.com>, Erle Pereira <erle@erlepereira.com>, AltF02 <contact@altf2.dev> | https://github.com/AltF02/x11-rs.git |
| `yoke` | `0.8.4` | `Unicode-3.0` | Manish Goregaokar <manishsmail@gmail.com> | https://github.com/unicode-org/icu4x |
| `yoke-derive` | `0.8.4` | `Unicode-3.0` | Manish Goregaokar <manishsmail@gmail.com> | https://github.com/unicode-org/icu4x |
| `zerofrom` | `0.1.8` | `Unicode-3.0` | The ICU4X Project Developers | https://github.com/unicode-org/icu4x |
| `zerofrom-derive` | `0.1.8` | `Unicode-3.0` | Manish Goregaokar <manishsmail@gmail.com> | https://github.com/unicode-org/icu4x |
| `zeroize` | `1.9.1` | `Apache-2.0 OR MIT` | The RustCrypto Project Developers | https://github.com/RustCrypto/utils |
| `zerotrie` | `0.2.5` | `Unicode-3.0` | The ICU4X Project Developers | https://github.com/unicode-org/icu4x |
| `zerovec` | `0.11.9` | `Unicode-3.0` | The ICU4X Project Developers | https://github.com/unicode-org/icu4x |
| `zerovec-derive` | `0.11.7` | `Unicode-3.0` | Manish Goregaokar <manishsmail@gmail.com> | https://github.com/unicode-org/icu4x |
| `zlib-rs` | `0.6.8` | `Zlib` | See upstream crate manifest | https://github.com/trifectatechfoundation/zlib-rs |
| `zmij` | `1.0.23` | `MIT` | David Tolnay <dtolnay@gmail.com> | https://github.com/dtolnay/zmij |

## 1b. Desktop Frontend npm Dependencies (167 Resolved Packages)

| Package | Version | License | Package Registry Artifact |
|---|---|---|---|
| `@babel/code-frame` | `7.29.7` | `MIT` | https://registry.npmjs.org/@babel/code-frame/-/code-frame-7.29.7.tgz |
| `@babel/compat-data` | `7.29.7` | `MIT` | https://registry.npmjs.org/@babel/compat-data/-/compat-data-7.29.7.tgz |
| `@babel/core` | `7.29.13` | `MIT` | https://registry.npmjs.org/@babel/core/-/core-7.29.13.tgz |
| `@babel/generator` | `7.29.13` | `MIT` | https://registry.npmjs.org/@babel/generator/-/generator-7.29.13.tgz |
| `@babel/helper-compilation-targets` | `7.29.13` | `MIT` | https://registry.npmjs.org/@babel/helper-compilation-targets/-/helper-compilation-targets-7.29.13.tgz |
| `@babel/helper-globals` | `7.29.7` | `MIT` | https://registry.npmjs.org/@babel/helper-globals/-/helper-globals-7.29.7.tgz |
| `@babel/helper-module-imports` | `7.29.7` | `MIT` | https://registry.npmjs.org/@babel/helper-module-imports/-/helper-module-imports-7.29.7.tgz |
| `@babel/helper-module-transforms` | `7.29.13` | `MIT` | https://registry.npmjs.org/@babel/helper-module-transforms/-/helper-module-transforms-7.29.13.tgz |
| `@babel/helper-plugin-utils` | `7.29.7` | `MIT` | https://registry.npmjs.org/@babel/helper-plugin-utils/-/helper-plugin-utils-7.29.7.tgz |
| `@babel/helper-string-parser` | `7.29.13` | `MIT` | https://registry.npmjs.org/@babel/helper-string-parser/-/helper-string-parser-7.29.13.tgz |
| `@babel/helper-validator-identifier` | `7.29.7` | `MIT` | https://registry.npmjs.org/@babel/helper-validator-identifier/-/helper-validator-identifier-7.29.7.tgz |
| `@babel/helper-validator-option` | `7.29.7` | `MIT` | https://registry.npmjs.org/@babel/helper-validator-option/-/helper-validator-option-7.29.7.tgz |
| `@babel/helpers` | `7.29.13` | `MIT` | https://registry.npmjs.org/@babel/helpers/-/helpers-7.29.13.tgz |
| `@babel/parser` | `7.29.13` | `MIT` | https://registry.npmjs.org/@babel/parser/-/parser-7.29.13.tgz |
| `@babel/plugin-transform-react-jsx-self` | `7.29.7` | `MIT` | https://registry.npmjs.org/@babel/plugin-transform-react-jsx-self/-/plugin-transform-react-jsx-self-7.29.7.tgz |
| `@babel/plugin-transform-react-jsx-source` | `7.29.7` | `MIT` | https://registry.npmjs.org/@babel/plugin-transform-react-jsx-source/-/plugin-transform-react-jsx-source-7.29.7.tgz |
| `@babel/template` | `7.29.7` | `MIT` | https://registry.npmjs.org/@babel/template/-/template-7.29.7.tgz |
| `@babel/traverse` | `7.29.13` | `MIT` | https://registry.npmjs.org/@babel/traverse/-/traverse-7.29.13.tgz |
| `@babel/types` | `7.29.11` | `MIT` | https://registry.npmjs.org/@babel/types/-/types-7.29.11.tgz |
| `@emnapi/core` | `1.11.3` | `MIT` | N/A |
| `@emnapi/runtime` | `1.11.3` | `MIT` | N/A |
| `@emnapi/wasi-threads` | `1.2.3` | `MIT` | N/A |
| `@esbuild/aix-ppc64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/aix-ppc64/-/aix-ppc64-0.25.12.tgz |
| `@esbuild/android-arm` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/android-arm/-/android-arm-0.25.12.tgz |
| `@esbuild/android-arm64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/android-arm64/-/android-arm64-0.25.12.tgz |
| `@esbuild/android-x64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/android-x64/-/android-x64-0.25.12.tgz |
| `@esbuild/darwin-arm64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/darwin-arm64/-/darwin-arm64-0.25.12.tgz |
| `@esbuild/darwin-x64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/darwin-x64/-/darwin-x64-0.25.12.tgz |
| `@esbuild/freebsd-arm64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/freebsd-arm64/-/freebsd-arm64-0.25.12.tgz |
| `@esbuild/freebsd-x64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/freebsd-x64/-/freebsd-x64-0.25.12.tgz |
| `@esbuild/linux-arm` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/linux-arm/-/linux-arm-0.25.12.tgz |
| `@esbuild/linux-arm64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/linux-arm64/-/linux-arm64-0.25.12.tgz |
| `@esbuild/linux-ia32` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/linux-ia32/-/linux-ia32-0.25.12.tgz |
| `@esbuild/linux-loong64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/linux-loong64/-/linux-loong64-0.25.12.tgz |
| `@esbuild/linux-mips64el` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/linux-mips64el/-/linux-mips64el-0.25.12.tgz |
| `@esbuild/linux-ppc64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/linux-ppc64/-/linux-ppc64-0.25.12.tgz |
| `@esbuild/linux-riscv64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/linux-riscv64/-/linux-riscv64-0.25.12.tgz |
| `@esbuild/linux-s390x` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/linux-s390x/-/linux-s390x-0.25.12.tgz |
| `@esbuild/linux-x64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/linux-x64/-/linux-x64-0.25.12.tgz |
| `@esbuild/netbsd-arm64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/netbsd-arm64/-/netbsd-arm64-0.25.12.tgz |
| `@esbuild/netbsd-x64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/netbsd-x64/-/netbsd-x64-0.25.12.tgz |
| `@esbuild/openbsd-arm64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/openbsd-arm64/-/openbsd-arm64-0.25.12.tgz |
| `@esbuild/openbsd-x64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/openbsd-x64/-/openbsd-x64-0.25.12.tgz |
| `@esbuild/openharmony-arm64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/openharmony-arm64/-/openharmony-arm64-0.25.12.tgz |
| `@esbuild/sunos-x64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/sunos-x64/-/sunos-x64-0.25.12.tgz |
| `@esbuild/win32-arm64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/win32-arm64/-/win32-arm64-0.25.12.tgz |
| `@esbuild/win32-ia32` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/win32-ia32/-/win32-ia32-0.25.12.tgz |
| `@esbuild/win32-x64` | `0.25.12` | `MIT` | https://registry.npmjs.org/@esbuild/win32-x64/-/win32-x64-0.25.12.tgz |
| `@jridgewell/gen-mapping` | `0.3.13` | `MIT` | https://registry.npmjs.org/@jridgewell/gen-mapping/-/gen-mapping-0.3.13.tgz |
| `@jridgewell/remapping` | `2.3.5` | `MIT` | https://registry.npmjs.org/@jridgewell/remapping/-/remapping-2.3.5.tgz |
| `@jridgewell/resolve-uri` | `3.1.2` | `MIT` | https://registry.npmjs.org/@jridgewell/resolve-uri/-/resolve-uri-3.1.2.tgz |
| `@jridgewell/sourcemap-codec` | `1.6.0` | `MIT` | https://registry.npmjs.org/@jridgewell/sourcemap-codec/-/sourcemap-codec-1.6.0.tgz |
| `@jridgewell/trace-mapping` | `0.3.31` | `MIT` | https://registry.npmjs.org/@jridgewell/trace-mapping/-/trace-mapping-0.3.31.tgz |
| `@monaco-editor/loader` | `1.7.0` | `MIT` | https://registry.npmjs.org/@monaco-editor/loader/-/loader-1.7.0.tgz |
| `@monaco-editor/react` | `4.7.0` | `MIT` | https://registry.npmjs.org/@monaco-editor/react/-/react-4.7.0.tgz |
| `@napi-rs/wasm-runtime` | `1.2.2` | `MIT` | N/A |
| `@rolldown/pluginutils` | `1.0.0-beta.27` | `MIT` | https://registry.npmjs.org/@rolldown/pluginutils/-/pluginutils-1.0.0-beta.27.tgz |
| `@rollup/rollup-android-arm-eabi` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-android-arm-eabi/-/rollup-android-arm-eabi-4.68.2.tgz |
| `@rollup/rollup-android-arm64` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-android-arm64/-/rollup-android-arm64-4.68.2.tgz |
| `@rollup/rollup-android-x64` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-android-x64/-/rollup-android-x64-4.68.2.tgz |
| `@rollup/rollup-darwin-arm64` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-darwin-arm64/-/rollup-darwin-arm64-4.68.2.tgz |
| `@rollup/rollup-darwin-x64` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-darwin-x64/-/rollup-darwin-x64-4.68.2.tgz |
| `@rollup/rollup-freebsd-arm64` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-freebsd-arm64/-/rollup-freebsd-arm64-4.68.2.tgz |
| `@rollup/rollup-freebsd-x64` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-freebsd-x64/-/rollup-freebsd-x64-4.68.2.tgz |
| `@rollup/rollup-linux-arm-gnueabihf` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-linux-arm-gnueabihf/-/rollup-linux-arm-gnueabihf-4.68.2.tgz |
| `@rollup/rollup-linux-arm-musleabihf` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-linux-arm-musleabihf/-/rollup-linux-arm-musleabihf-4.68.2.tgz |
| `@rollup/rollup-linux-arm64-gnu` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-linux-arm64-gnu/-/rollup-linux-arm64-gnu-4.68.2.tgz |
| `@rollup/rollup-linux-arm64-musl` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-linux-arm64-musl/-/rollup-linux-arm64-musl-4.68.2.tgz |
| `@rollup/rollup-linux-loong64-gnu` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-linux-loong64-gnu/-/rollup-linux-loong64-gnu-4.68.2.tgz |
| `@rollup/rollup-linux-loong64-musl` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-linux-loong64-musl/-/rollup-linux-loong64-musl-4.68.2.tgz |
| `@rollup/rollup-linux-ppc64-gnu` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-linux-ppc64-gnu/-/rollup-linux-ppc64-gnu-4.68.2.tgz |
| `@rollup/rollup-linux-ppc64-musl` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-linux-ppc64-musl/-/rollup-linux-ppc64-musl-4.68.2.tgz |
| `@rollup/rollup-linux-riscv64-gnu` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-linux-riscv64-gnu/-/rollup-linux-riscv64-gnu-4.68.2.tgz |
| `@rollup/rollup-linux-riscv64-musl` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-linux-riscv64-musl/-/rollup-linux-riscv64-musl-4.68.2.tgz |
| `@rollup/rollup-linux-s390x-gnu` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-linux-s390x-gnu/-/rollup-linux-s390x-gnu-4.68.2.tgz |
| `@rollup/rollup-linux-x64-gnu` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-linux-x64-gnu/-/rollup-linux-x64-gnu-4.68.2.tgz |
| `@rollup/rollup-linux-x64-musl` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-linux-x64-musl/-/rollup-linux-x64-musl-4.68.2.tgz |
| `@rollup/rollup-openbsd-x64` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-openbsd-x64/-/rollup-openbsd-x64-4.68.2.tgz |
| `@rollup/rollup-openharmony-arm64` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-openharmony-arm64/-/rollup-openharmony-arm64-4.68.2.tgz |
| `@rollup/rollup-win32-arm64-msvc` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-win32-arm64-msvc/-/rollup-win32-arm64-msvc-4.68.2.tgz |
| `@rollup/rollup-win32-ia32-msvc` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-win32-ia32-msvc/-/rollup-win32-ia32-msvc-4.68.2.tgz |
| `@rollup/rollup-win32-x64-gnu` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-win32-x64-gnu/-/rollup-win32-x64-gnu-4.68.2.tgz |
| `@rollup/rollup-win32-x64-msvc` | `4.68.2` | `MIT` | https://registry.npmjs.org/@rollup/rollup-win32-x64-msvc/-/rollup-win32-x64-msvc-4.68.2.tgz |
| `@tailwindcss/node` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/node/-/node-4.3.5.tgz |
| `@tailwindcss/oxide` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/oxide/-/oxide-4.3.5.tgz |
| `@tailwindcss/oxide-android-arm64` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/oxide-android-arm64/-/oxide-android-arm64-4.3.5.tgz |
| `@tailwindcss/oxide-darwin-arm64` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/oxide-darwin-arm64/-/oxide-darwin-arm64-4.3.5.tgz |
| `@tailwindcss/oxide-darwin-x64` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/oxide-darwin-x64/-/oxide-darwin-x64-4.3.5.tgz |
| `@tailwindcss/oxide-freebsd-x64` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/oxide-freebsd-x64/-/oxide-freebsd-x64-4.3.5.tgz |
| `@tailwindcss/oxide-linux-arm-gnueabihf` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/oxide-linux-arm-gnueabihf/-/oxide-linux-arm-gnueabihf-4.3.5.tgz |
| `@tailwindcss/oxide-linux-arm64-gnu` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/oxide-linux-arm64-gnu/-/oxide-linux-arm64-gnu-4.3.5.tgz |
| `@tailwindcss/oxide-linux-arm64-musl` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/oxide-linux-arm64-musl/-/oxide-linux-arm64-musl-4.3.5.tgz |
| `@tailwindcss/oxide-linux-x64-gnu` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/oxide-linux-x64-gnu/-/oxide-linux-x64-gnu-4.3.5.tgz |
| `@tailwindcss/oxide-linux-x64-musl` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/oxide-linux-x64-musl/-/oxide-linux-x64-musl-4.3.5.tgz |
| `@tailwindcss/oxide-wasm32-wasi` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/oxide-wasm32-wasi/-/oxide-wasm32-wasi-4.3.5.tgz |
| `@tailwindcss/oxide-win32-arm64-msvc` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/oxide-win32-arm64-msvc/-/oxide-win32-arm64-msvc-4.3.5.tgz |
| `@tailwindcss/oxide-win32-x64-msvc` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/oxide-win32-x64-msvc/-/oxide-win32-x64-msvc-4.3.5.tgz |
| `@tailwindcss/vite` | `4.3.5` | `MIT` | https://registry.npmjs.org/@tailwindcss/vite/-/vite-4.3.5.tgz |
| `@tauri-apps/api` | `2.12.4` | `Apache-2.0 OR MIT` | https://registry.npmjs.org/@tauri-apps/api/-/api-2.12.4.tgz |
| `@tybys/wasm-util` | `0.10.3` | `MIT` | N/A |
| `@types/babel__core` | `7.20.5` | `MIT` | https://registry.npmjs.org/@types/babel__core/-/babel__core-7.20.5.tgz |
| `@types/babel__generator` | `7.27.0` | `MIT` | https://registry.npmjs.org/@types/babel__generator/-/babel__generator-7.27.0.tgz |
| `@types/babel__template` | `7.4.4` | `MIT` | https://registry.npmjs.org/@types/babel__template/-/babel__template-7.4.4.tgz |
| `@types/babel__traverse` | `7.28.0` | `MIT` | https://registry.npmjs.org/@types/babel__traverse/-/babel__traverse-7.28.0.tgz |
| `@types/estree` | `1.0.10` | `MIT` | https://registry.npmjs.org/@types/estree/-/estree-1.0.10.tgz |
| `@types/react` | `19.3.0` | `MIT` | https://registry.npmjs.org/@types/react/-/react-19.3.0.tgz |
| `@types/react-dom` | `19.3.0` | `MIT` | https://registry.npmjs.org/@types/react-dom/-/react-dom-19.3.0.tgz |
| `@vitejs/plugin-react` | `4.7.0` | `MIT` | https://registry.npmjs.org/@vitejs/plugin-react/-/plugin-react-4.7.0.tgz |
| `@xterm/addon-fit` | `0.10.0` | `MIT` | https://registry.npmjs.org/@xterm/addon-fit/-/addon-fit-0.10.0.tgz |
| `@xterm/xterm` | `5.5.0` | `MIT` | https://registry.npmjs.org/@xterm/xterm/-/xterm-5.5.0.tgz |
| `baseline-browser-mapping` | `2.11.35` | `Apache-2.0` | https://registry.npmjs.org/baseline-browser-mapping/-/baseline-browser-mapping-2.11.35.tgz |
| `browserslist` | `4.29.5` | `MIT` | https://registry.npmjs.org/browserslist/-/browserslist-4.29.5.tgz |
| `caniuse-lite` | `1.0.30001826` | `CC-BY-4.0` | https://registry.npmjs.org/caniuse-lite/-/caniuse-lite-1.0.30001826.tgz |
| `convert-source-map` | `2.0.0` | `MIT` | https://registry.npmjs.org/convert-source-map/-/convert-source-map-2.0.0.tgz |
| `csstype` | `3.2.3` | `MIT` | https://registry.npmjs.org/csstype/-/csstype-3.2.3.tgz |
| `debug` | `4.4.3` | `MIT` | https://registry.npmjs.org/debug/-/debug-4.4.3.tgz |
| `detect-libc` | `2.1.2` | `Apache-2.0` | https://registry.npmjs.org/detect-libc/-/detect-libc-2.1.2.tgz |
| `electron-to-chromium` | `1.5.481` | `ISC` | https://registry.npmjs.org/electron-to-chromium/-/electron-to-chromium-1.5.481.tgz |
| `enhanced-resolve` | `5.26.1` | `MIT` | https://registry.npmjs.org/enhanced-resolve/-/enhanced-resolve-5.26.1.tgz |
| `esbuild` | `0.25.12` | `MIT` | https://registry.npmjs.org/esbuild/-/esbuild-0.25.12.tgz |
| `escalade` | `3.2.0` | `MIT` | https://registry.npmjs.org/escalade/-/escalade-3.2.0.tgz |
| `fdir` | `6.5.0` | `MIT` | https://registry.npmjs.org/fdir/-/fdir-6.5.0.tgz |
| `fsevents` | `2.3.3` | `MIT` | https://registry.npmjs.org/fsevents/-/fsevents-2.3.3.tgz |
| `gensync` | `1.0.0-beta.2` | `MIT` | https://registry.npmjs.org/gensync/-/gensync-1.0.0-beta.2.tgz |
| `graceful-fs` | `4.2.11` | `ISC` | https://registry.npmjs.org/graceful-fs/-/graceful-fs-4.2.11.tgz |
| `jiti` | `2.7.0` | `MIT` | https://registry.npmjs.org/jiti/-/jiti-2.7.0.tgz |
| `js-tokens` | `4.0.0` | `MIT` | https://registry.npmjs.org/js-tokens/-/js-tokens-4.0.0.tgz |
| `jsesc` | `3.1.0` | `MIT` | https://registry.npmjs.org/jsesc/-/jsesc-3.1.0.tgz |
| `json5` | `2.2.3` | `MIT` | https://registry.npmjs.org/json5/-/json5-2.2.3.tgz |
| `lightningcss` | `1.33.0` | `MPL-2.0` | https://registry.npmjs.org/lightningcss/-/lightningcss-1.33.0.tgz |
| `lightningcss-android-arm64` | `1.33.0` | `MPL-2.0` | https://registry.npmjs.org/lightningcss-android-arm64/-/lightningcss-android-arm64-1.33.0.tgz |
| `lightningcss-darwin-arm64` | `1.33.0` | `MPL-2.0` | https://registry.npmjs.org/lightningcss-darwin-arm64/-/lightningcss-darwin-arm64-1.33.0.tgz |
| `lightningcss-darwin-x64` | `1.33.0` | `MPL-2.0` | https://registry.npmjs.org/lightningcss-darwin-x64/-/lightningcss-darwin-x64-1.33.0.tgz |
| `lightningcss-freebsd-x64` | `1.33.0` | `MPL-2.0` | https://registry.npmjs.org/lightningcss-freebsd-x64/-/lightningcss-freebsd-x64-1.33.0.tgz |
| `lightningcss-linux-arm-gnueabihf` | `1.33.0` | `MPL-2.0` | https://registry.npmjs.org/lightningcss-linux-arm-gnueabihf/-/lightningcss-linux-arm-gnueabihf-1.33.0.tgz |
| `lightningcss-linux-arm64-gnu` | `1.33.0` | `MPL-2.0` | https://registry.npmjs.org/lightningcss-linux-arm64-gnu/-/lightningcss-linux-arm64-gnu-1.33.0.tgz |
| `lightningcss-linux-arm64-musl` | `1.33.0` | `MPL-2.0` | https://registry.npmjs.org/lightningcss-linux-arm64-musl/-/lightningcss-linux-arm64-musl-1.33.0.tgz |
| `lightningcss-linux-x64-gnu` | `1.33.0` | `MPL-2.0` | https://registry.npmjs.org/lightningcss-linux-x64-gnu/-/lightningcss-linux-x64-gnu-1.33.0.tgz |
| `lightningcss-linux-x64-musl` | `1.33.0` | `MPL-2.0` | https://registry.npmjs.org/lightningcss-linux-x64-musl/-/lightningcss-linux-x64-musl-1.33.0.tgz |
| `lightningcss-win32-arm64-msvc` | `1.33.0` | `MPL-2.0` | https://registry.npmjs.org/lightningcss-win32-arm64-msvc/-/lightningcss-win32-arm64-msvc-1.33.0.tgz |
| `lightningcss-win32-x64-msvc` | `1.33.0` | `MPL-2.0` | https://registry.npmjs.org/lightningcss-win32-x64-msvc/-/lightningcss-win32-x64-msvc-1.33.0.tgz |
| `lru-cache` | `5.1.1` | `ISC` | https://registry.npmjs.org/lru-cache/-/lru-cache-5.1.1.tgz |
| `lucide-react` | `0.511.0` | `ISC` | https://registry.npmjs.org/lucide-react/-/lucide-react-0.511.0.tgz |
| `magic-string` | `1.4.5` | `MIT` | https://registry.npmjs.org/magic-string/-/magic-string-1.4.5.tgz |
| `monaco-editor` | `0.52.2` | `MIT` | https://registry.npmjs.org/monaco-editor/-/monaco-editor-0.52.2.tgz |
| `ms` | `2.1.3` | `MIT` | https://registry.npmjs.org/ms/-/ms-2.1.3.tgz |
| `nanoid` | `3.3.20` | `MIT` | https://registry.npmjs.org/nanoid/-/nanoid-3.3.20.tgz |
| `node-releases` | `2.0.63` | `MIT` | https://registry.npmjs.org/node-releases/-/node-releases-2.0.63.tgz |
| `picocolors` | `1.1.1` | `ISC` | https://registry.npmjs.org/picocolors/-/picocolors-1.1.1.tgz |
| `picomatch` | `4.0.7` | `MIT` | https://registry.npmjs.org/picomatch/-/picomatch-4.0.7.tgz |
| `postcss` | `8.5.32` | `MIT` | https://registry.npmjs.org/postcss/-/postcss-8.5.32.tgz |
| `react` | `19.3.1` | `MIT` | https://registry.npmjs.org/react/-/react-19.3.1.tgz |
| `react-dom` | `19.3.1` | `MIT` | https://registry.npmjs.org/react-dom/-/react-dom-19.3.1.tgz |
| `react-refresh` | `0.17.0` | `MIT` | https://registry.npmjs.org/react-refresh/-/react-refresh-0.17.0.tgz |
| `rollup` | `4.68.2` | `MIT` | https://registry.npmjs.org/rollup/-/rollup-4.68.2.tgz |
| `scheduler` | `0.28.0` | `MIT` | https://registry.npmjs.org/scheduler/-/scheduler-0.28.0.tgz |
| `semver` | `6.3.2` | `ISC` | https://registry.npmjs.org/semver/-/semver-6.3.2.tgz |
| `source-map-js` | `1.2.2` | `BSD-3-Clause` | https://registry.npmjs.org/source-map-js/-/source-map-js-1.2.2.tgz |
| `state-local` | `1.0.7` | `MIT` | https://registry.npmjs.org/state-local/-/state-local-1.0.7.tgz |
| `tailwindcss` | `4.3.5` | `MIT` | https://registry.npmjs.org/tailwindcss/-/tailwindcss-4.3.5.tgz |
| `tapable` | `2.3.3` | `MIT` | https://registry.npmjs.org/tapable/-/tapable-2.3.3.tgz |
| `tinyglobby` | `0.2.17` | `MIT` | https://registry.npmjs.org/tinyglobby/-/tinyglobby-0.2.17.tgz |
| `tslib` | `2.8.1` | `0BSD` | N/A |
| `typescript` | `5.9.3` | `Apache-2.0` | https://registry.npmjs.org/typescript/-/typescript-5.9.3.tgz |
| `update-browserslist-db` | `1.3.4` | `MIT` | https://registry.npmjs.org/update-browserslist-db/-/update-browserslist-db-1.3.4.tgz |
| `vite` | `6.4.6` | `MIT` | https://registry.npmjs.org/vite/-/vite-6.4.6.tgz |
| `yallist` | `3.1.1` | `ISC` | https://registry.npmjs.org/yallist/-/yallist-3.1.1.tgz |

## 2. Bundled & Composite Notices

### `ring` (`Apache-2.0 AND ISC`)

```text
*ring* uses an "ISC" license, like BoringSSL used to use, for new code
files. See LICENSE-other-bits for the text of that license.

See LICENSE-BoringSSL for code that was sourced from BoringSSL under the
Apache 2.0 license. Some code that was sourced from BoringSSL under the ISC
license. In each case, the license info is at the top of the file.

See src/polyfill/once_cell/LICENSE-APACHE and src/polyfill/once_cell/LICENSE-MIT
for the license to code that was sourced from the once_cell project.
```

### `SQLite` (bundled via `libsqlite3-sys`)

SQLite source code is in the public domain (`https://sqlite.org/copyright.html`).

## 3. Representative License Texts (from Resolved Crate Registry)

### MIT License (`tokio`)

```text
MIT License

Copyright (c) Tokio Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### Apache-2.0 License (`serde`)

```text
Apache License
                        Version 2.0, January 2004
                     http://www.apache.org/licenses/

TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION

1. Definitions.

   "License" shall mean the terms and conditions for use, reproduction,
   and distribution as defined by Sections 1 through 9 of this document.

   "Licensor" shall mean the copyright owner or entity authorized by
   the copyright owner that is granting the License.

   "Legal Entity" shall mean the union of the acting entity and all
   other entities that control, are controlled by, or are under common
   control with that entity. For the purposes of this definition,
   "control" means (i) the power, direct or indirect, to cause the
   direction or management of such entity, whether by contract or
   otherwise, or (ii) ownership of fifty percent (50%) or more of the
   outstanding shares, or (iii) beneficial ownership of such entity.

   "You" (or "Your") shall mean an individual or Legal Entity
   exercising permissions granted by this License.

   "Source" form shall mean the preferred form for making modifications,
   including but not limited to software source code, documentation
   source, and configuration files.

   "Object" form shall mean any form resulting from mechanical
   transformation or translation of a Source form, including but
   not limited to compiled object code, generated documentation,
   and conversions to other media types.

   "Work" shall mean the work of authorship, whether in Source or
   Object form, made available under the License, as indicated by a
   copyright notice that is included in or attached to the work
   (an example is provided in the Appendix below).

   "Derivative Works" shall mean any work, whether in Source or Object
   form, that is based on (or derived from) the Work and for which the
   editorial revisions, annotations, elaborations, or other modifications
   represent, as a whole, an original work of authorship. For the purposes
   of this License, Derivative Works shall not include works that remain
   separable from, or merely link (or bind by name) to the interfaces of,
   the Work and Derivative Works thereof.

   "Contribution" shall mean any work of authorship, including
   the original version of the Work and any modifications or additions
   to that Work or Derivative Works thereof, that is intentionally
   submitted to Licensor for inclusion in the Work by the copyright owner
   or by an individual or Legal Entity authorized to submit on behalf of
   the copyright owner. For the purposes of this definition, "submitted"
   means any form of electronic, verbal, or written communication sent
   to the Licensor or its representatives, including but not limited to
   communication on electronic mailing lists, source code control systems,
   and issue tracking systems that are managed by, or on behalf of, the
   Licensor for the purpose of discussing and improving the Work, but
   excluding communication that is conspicuously marked or otherwise
   designated in writing by the copyright owner as "Not a Contribution."

   "Contributor" shall mean Licensor and any individual or Legal Entity
   on behalf of whom a Contribution has been received by Licensor and
   subsequently incorporated within the Work.

2. Grant of Copyright License. Subject to the terms and conditions of
   this License, each Contributor hereby grants to You a perpetual,
   worldwide, non-exclusive, no-charge, royalty-free, irrevocable
   copyright license to reproduce, prepare Derivative Works of,
   publicly display, publicly perform, sublicense, and distribute the
   Work and such Derivative Works in Source or Object form.

3. Grant of Patent License. Subject to the terms and conditions of
   this License, each Contributor hereby grants to You a perpetual,
   worldwide, non-exclusive, no-charge, royalty-free, irrevocable
   (except as stated in this section) patent license to make, have made,
   use, offer to sell, sell, import, and otherwise transfer the Work,
   where such license applies only to those patent claims licensable
   by such Contributor that are necessarily infringed by their
   Contribution(s) alone or by combination of their Contribution(s)
   with the Work to which such Contribution(s) was submitted. If You
   institute patent litigation against any entity (including a
   cross-claim or counterclaim in a lawsuit) alleging that the Work
   or a Contribution incorporated within the Work constitutes direct
   or contributory patent infringement, then any patent licenses
   granted to You under this License for that Work shall terminate
   as of the date such litigation is filed.

4. Redistribution. You may reproduce and distribute copies of the
   Work or Derivative Works thereof in any medium, with or without
   modifications, and in Source or Object form, provided that You
   meet the following conditions:

   (a) You must give any other recipients of the Work or
       Derivative Works a copy of this License; and

   (b) You must cause any modified files to carry prominent notices
       stating that You changed the files; and

   (c) You must retain, in the Source form of any Derivative Works
       that You distribute, all copyright, patent, trademark, and
       attribution notices from the Source form of the Work,
       excluding those notices that do not pertain to any part of
       the Derivative Works; and

   (d) If the Work includes a "NOTICE" text file as part of its
       distribution, then any Derivative Works that You distribute must
       include a readable copy of the attribution notices contained
       within such NOTICE file, excluding those notices that do not
       pertain to any part of the Derivative Works, in at least one
       of the following places: within a NOTICE text file distributed
       as part of the Derivative Works; within the Source form or
       documentation, if provided along with the Derivative Works; or,
       within a display generated by the Derivative Works, if and
       wherever such third-party notices normally appear. The contents
       of the NOTICE file are for informational purposes only and
       do not modify the License. You may add Your own attribution
       notices within Derivative Works that You distribute, alongside
       or as an addendum to the NOTICE text from the Work, provided
       that such additional attribution notices cannot be construed
       as modifying the License.

   You may add Your own copyright statement to Your modifications and
   may provide additional or different license terms and conditions
   for use, reproduction, or distribution of Your modifications, or
   for any such Derivative Works as a whole, provided Your use,
   reproduction, and distribution of the Work otherwise complies with
   the conditions stated in this License.

5. Submission of Contributions. Unless You explicitly state otherwise,
   any Contribution intentionally submitted for inclusion in the Work
   by You to the Licensor shall be under the terms and conditions of
   this License, without any additional terms or conditions.
   Notwithstanding the above, nothing herein shall supersede or modify
   the terms of any separate license agreement you may have executed
   with Licensor regarding such Contributions.

6. Trademarks. This License does not grant permission to use the trade
   names, trademarks, service marks, or product names of the Licensor,
   except as required for reasonable and customary use in describing the
   origin of the Work and reproducing the content of the NOTICE file.

7. Disclaimer of Warranty. Unless required by applicable law or
   agreed to in writing, Licensor provides the Work (and each
   Contributor provides its Contributions) on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or
   implied, including, without limitation, any warranties or conditions
   of TITLE, NON-INFRINGEMENT, MERCHANTABILITY, or FITNESS FOR A
   PARTICULAR PURPOSE. You are solely responsible for determining the
   appropriateness of using or redistributing the Work and assume any
   risks associated with Your exercise of permissions under this License.

8. Limitation of Liability. In no event and under no legal theory,
   whether in tort (including negligence), contract, or otherwise,
   unless required by applicable law (such as deliberate and grossly
   negligent acts) or agreed to in writing, shall any Contributor be
   liable to You for damages, including any direct, indirect, special,
   incidental, or consequential damages of any character arising as a
   result of this License or out of the use or inability to use the
   Work (including but not limited to damages for loss of goodwill,
   work stoppage, computer failure or malfunction, or any and all
   other commercial damages or losses), even if such Contributor
   has been advised of the possibility of such damages.

9. Accepting Warranty or Additional Liability. While redistributing
   the Work or Derivative Works thereof, You may choose to offer,
   and charge a fee for, acceptance of support, warranty, indemnity,
   or other liability obligations and/or rights consistent with this
   License. However, in accepting such obligations, You may act only
   on Your own behalf and on Your sole responsibility, not on behalf
   of any other Contributor, and only if You agree to indemnify,
   defend, and hold each Contributor harmless for any liability
   incurred by, or claims asserted against, such Contributor by reason
   of your accepting any such warranty or additional liability.

END OF TERMS AND CONDITIONS
```

### BSD-3-Clause License (`subtle`)

```text
Copyright (c) 2016-2017 Isis Agora Lovecruft, Henry de Valence. All rights reserved.
Copyright (c) 2016-2024 Isis Agora Lovecruft. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

1. Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS
IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### Zlib License (`foldhash`)

```text
Copyright (c) 2024 Orson Peters

This software is provided 'as-is', without any express or implied warranty. In
no event will the authors be held liable for any damages arising from the use of
this software.

Permission is granted to anyone to use this software for any purpose, including
commercial applications, and to alter it and redistribute it freely, subject to
the following restrictions:

1. The origin of this software must not be misrepresented; you must not claim
    that you wrote the original software. If you use this software in a product,
    an acknowledgment in the product documentation would be appreciated but is
    not required.

2. Altered source versions must be plainly marked as such, and must not be
    misrepresented as being the original software.

3. This notice may not be removed or altered from any source distribution.
```

### Unlicense (`memchr`)

```text
This is free and unencumbered software released into the public domain.

Anyone is free to copy, modify, publish, use, compile, sell, or
distribute this software, either in source code form or as a compiled
binary, for any purpose, commercial or non-commercial, and by any
means.

In jurisdictions that recognize copyright laws, the author or authors
of this software dedicate any and all copyright interest in the
software to the public domain. We make this dedication for the benefit
of the public at large and to the detriment of our heirs and
successors. We intend this dedication to be an overt act of
relinquishment in perpetuity of all present and future rights to this
software under copyright law.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
OTHER DEALINGS IN THE SOFTWARE.

For more information, please refer to <http://unlicense.org/>
```

### Unicode-3.0 License (`icu_collections`)

```text
UNICODE LICENSE V3

COPYRIGHT AND PERMISSION NOTICE

Copyright © 2020-2024 Unicode, Inc.

NOTICE TO USER: Carefully read the following legal agreement. BY
DOWNLOADING, INSTALLING, COPYING OR OTHERWISE USING DATA FILES, AND/OR
SOFTWARE, YOU UNEQUIVOCALLY ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE
TERMS AND CONDITIONS OF THIS AGREEMENT. IF YOU DO NOT AGREE, DO NOT
DOWNLOAD, INSTALL, COPY, DISTRIBUTE OR USE THE DATA FILES OR SOFTWARE.

Permission is hereby granted, free of charge, to any person obtaining a
copy of data files and any associated documentation (the "Data Files") or
software and any associated documentation (the "Software") to deal in the
Data Files or Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, and/or sell
copies of the Data Files or Software, and to permit persons to whom the
Data Files or Software are furnished to do so, provided that either (a)
this copyright and permission notice appear with all copies of the Data
Files or Software, or (b) this copyright and permission notice appear in
associated Documentation.

THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF
THIRD PARTY RIGHTS.

IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS NOTICE
BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL DAMAGES,
OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THE DATA
FILES OR SOFTWARE.

Except as contained in this notice, the name of a copyright holder shall
not be used in advertising or otherwise to promote the sale, use or other
dealings in these Data Files or Software without prior written
authorization of the copyright holder.

SPDX-License-Identifier: Unicode-3.0

—

Portions of ICU4X may have been adapted from ICU4C and/or ICU4J.
ICU 1.8.1 to ICU 57.1 © 1995-2016 International Business Machines Corporation and others.
```
