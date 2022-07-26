[![Our chat room](https://img.shields.io/matrix/fractal-gtk:matrix.org?color=blue&label=%23fractal%3Agnome.org&logo=matrix)](https://matrix.to/#/#fractal:gnome.org)
[![Our Gitlab project](https://img.shields.io/badge/gitlab.gnome.org%2F-GNOME%2FFractal-green?logo=gitlab)](https://gitlab.gnome.org/GNOME/fractal/)
[![Our documentation](https://img.shields.io/badge/%F0%9F%95%AE-Docs-B7410E?logo=rust)](https://gnome.pages.gitlab.gnome.org/fractal/)

# Fractal

Fractal is a Matrix messaging app for GNOME written in Rust. Its interface is optimized for
collaboration in large groups, such as free software projects.

![screenshot](https://gitlab.gnome.org/GNOME/fractal/raw/main/screenshots/fractal.png)

## Work in Progress

We already talked several times in the past about rewriting the application, but for different
reasons we didn't do it. Now that the [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk)
exists, which does a lot of the heavy lifting for us, we have a good starting point to build Fractal
without the need to implement every single feature from the Matrix API. Finally with the release of
GTK4 we would need to rework most of Fractal's code anyways. Therefore, it just makes sense to start
over and build Fractal with all the features (e.g end-to-end encryption) we have in mind.

A year ago we started working on rewriting [Fractal](https://gitlab.gnome.org/GNOME/fractal/) from
scratch using [GTK4](https://www.gtk.org/) and the [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk).
This effort was called Fractal Next.

Fractal Next now replaced our previous codebase, and has become the new nightly version. It isn't
yet ready for a release and you can follow along our progress towards it by looking at the
[feature parity](https://gitlab.gnome.org/GNOME/fractal/-/milestones/18) and
[Fractal v5 (fractal-next)](https://gitlab.gnome.org/GNOME/fractal/-/milestones/21) milestones.

## Installation instructions

### Stable version

The current stable version is 4.4.0 (released August 2020).

Flatpak is the recommended installation method.
Until our next iteration is ready, you can get the official Fractal Flatpak on Flathub.

<a href="https://flathub.org/apps/details/org.gnome.Fractal">
<img
    src="https://flathub.org/assets/badges/flathub-badge-i-en.png"
    alt="Download Fractal on Flathub"
    width="240px"
/>
</a>

### Development version

If you want to try Fractal Next without building it yourself, it is available as a nightly Flatpak
in the gnome-nightly repo.

```sh
# Add the gnome-nightly repo
flatpak remote-add --user --if-not-exists gnome-nightly https://nightly.gnome.org/gnome-nightly.flatpakrepo

# Install the nightly build
flatpak install --user gnome-nightly org.gnome.Fractal.Devel
```

## Build Instructions

### Minimum Rust version

To build Fractal, Rust 1.60 is required. For development, you'll need to install the nightly
toolchain to be able to run our pre-commit hook that validates the formatting and lints the Rust
code.

### Flatpak

Flatpak is the recommended way of building and installing Fractal.

First you need to make sure you have the GNOME SDK and Rust toolchain installed.

```sh
# Add Flathub and the gnome-nightly repo
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak remote-add --user --if-not-exists gnome-nightly https://nightly.gnome.org/gnome-nightly.flatpakrepo

# Install the gnome-nightly Sdk and Platform runtime
flatpak install --user gnome-nightly org.gnome.Sdk org.gnome.Platform

# Install the required rust-stable extension from Flathub
flatpak install --user flathub org.freedesktop.Sdk.Extension.rust-stable//21.08

# Install the required llvm extension from Flathub
flatpak install --user flathub org.freedesktop.Sdk.Extension.llvm12//21.08
```

<table><tr><td>
<p>ℹ️ The instructions below will build the same binary as the one available on the GNOME nightly
repo. This is an optimised build so it can take a few minutes.</p>

<p>If you're building Fractal for development, use the <code>org.gnome.Fractal.Hack.json</code> manifest
instead.</p>
</td></tr></table>

Move inside the `build-aux` folder and then build and install the app:

```sh
cd build-aux
flatpak-builder --user --install app org.gnome.Fractal.Devel.json
```

Fractal Next can then be entirely removed from your system with:

```sh
flatpak remove --delete-data org.gnome.Fractal.Devel
```

### GNU/Linux

If you decide to ignore our recommendation and build on your host system,
outside of Flatpak, you will need Meson and Ninja (as well as Rust and Cargo).

```sh
meson . _build --prefix=/usr/local
ninja -C _build
sudo ninja -C _build install
```

### Translations

Fractal is translated by the GNOME translation team on [Damned lies](https://l10n.gnome.org/).

Find your language in the list on [the Fractal module page on Damned lies](https://l10n.gnome.org/module/fractal/).

### Password Storage

Fractal uses [Secret Service](https://www.freedesktop.org/wiki/Specifications/secret-storage-spec/)
to store the password so you should have something providing that service on your system. If you're
using GNOME or KDE this should work for you out of the box with gnome-keyring or ksecretservice.

## Frequently Asked Questions

* Does Fractal have encryption support? Will it ever?

Yes, the current development version (`main` branch) has encryption support using Cross-Signing. See
<https://gitlab.gnome.org/GNOME/fractal/-/issues/717> for more info on the state of encryption.

* Can I run Fractal with the window closed?

Currently Fractal does not support this. Fractal is a GNOME application, and accordingly adheres GNOME
guidelines and paradigms. This will be revisited if or when GNOME gets a "Do Not Disturb" feature.

## The origin of Fractal

The development version is a complete rewrite of Fractal built on top of the
[matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk) using [GTK4](https://gtk.org/).

The previous version of Fractal was using GTK3 and its own backend to talk to a matrix homeserver,
the code can be found in the [`legacy` branch](https://gitlab.gnome.org/GNOME/fractal/-/tree/legacy).

Initial versions were based on Fest <https://github.com/fest-im/fest>, formerly called ruma-gtk.
In the origins of the project it was called guillotine, based on French revolution, in relation with
the Riot client name, but it's a negative name so we decide to change for a math one.

The name Fractal was proposed by Regina Bíró.

## Code of Conduct

Fractal follows the official GNOME Foundation code of conduct. You can read it [here](/code-of-conduct.md).
