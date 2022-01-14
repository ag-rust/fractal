#!/bin/sh
# Source: https://gitlab.gnome.org/GNOME/fractal/blob/master/hooks/pre-commit.hook

install_rustfmt() {
    if ! which rustup &> /dev/null; then
        curl https://sh.rustup.rs -sSf  | sh -s -- -y
        export PATH=$PATH:$HOME/.cargo/bin
        if ! which rustup &> /dev/null; then
            echo "Failed to install rustup."
            exit 2
        fi
    fi

    if ! rustup component list|grep rustfmt &> /dev/null; then
        echo "Installing rustfmt…"
        rustup component add rustfmt
    fi
}

if ! which cargo >/dev/null 2>&1 || ! cargo fmt --help >/dev/null 2>&1; then
    echo "Unable to check Fractal’s code style, because rustfmt could not be run."

    if [ ! -t 1 ]; then
        # No input is possible
        echo "Performing commit."
        exit 0
    fi

    echo ""
    echo "y: Install rustfmt via rustup"
    echo "N: Don't install rustfmt"

    echo ""
    while true
    do
        echo -n "Install rustfmt? [y/N]: "; read yn < /dev/tty
        case $yn in
            [Yy]* ) install_rustfmt; break;;
            [Nn]* | "" ) exit 2 >/dev/null 2>&1;;
            * ) echo "Invalid input";;
        esac
    done

fi

echo "--Checking style--"
cargo fmt --all -- --check
if test $? != 0; then
    echo "--Checking style fail--"
    echo "Please fix the above issues, either manually or by running: cargo fmt --all"

    exit 1
else
    echo "--Checking style pass--"
fi
