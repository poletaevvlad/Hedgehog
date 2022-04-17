# Hedgehog

Hedgehog is a podcast player and organizer with terminal-based user interface. 

 * *Subscription management and automatic feed updates.* Using Hedgehog you can
   subscribe to RSS feeds, they will automatically be updated on startup and on
   request.
 * *Keeping track of the status of each episode.* Hedgehog remembers whether
   you have played each episode before, whether you finished, or where you
   stopped. It can then resume playback from that point.
 * *Online search for new feeds to subscribe to.* Hedgehog can search for
 * podcasts by their titles online, so there is no need to look for the RSS
   link.
 * *Flexible theming.* The visual style of any component of the Hedgehog's UI
   can be recolored in a state-dependant manner. There are several built-in
   themes: `default`, `nord`, `solarized-dark`, `solarized-light`,
   `gruvbox-dark`, `gruvbox-light` which you can activate using 
   `theme load <name>` command. The manual contains a detailed reference for
   creating your own custom themes.
 * *Fully configurable keybindings.* Hedgehog is almost exclusively controlled
   through issuing commands either directly or through key bindings. All of
   these keybindings can be changed by user's configuration.
 * *Integration with external programs through MPRIS.* On Linux and other
   operating systems with dbus integration, Hedgehog reports its status through
   MPRIS, and accepts commands from external software.
 * *Mouse support in the terminal-based UI.* Hedgehog is a terminal
   application, and it's designed to be fully controlled through keyboard. But
   much of its functionality (playback control, navigation, episode and feed
   selection, etc.) can be accessed through mouse actions.


## Manual

Hedgehog comes with a user manual in the form of a man page that can be viewed
via `man hedgehog 1`. It's also available online at
[poletaevvlad.github.io/Hedgehog/hedgehog.1.html](https://poletaevvlad.github.io/Hedgehog/hedgehog.1.html).


## Installation

### Dependencies

Hedgehog has some runtime dependencies that must be installed before Hedgehog
can be either compiled from source or installed from binary distribution.

* [SQLite libs](https://www.sqlite.org/download.html)
* [dbus](https://www.freedesktop.org/wiki/Software/dbus/#download) (For MPRIS support)
* [OpenSSL](https://www.openssl.org/source/)
* [GStreamer](https://gstreamer.freedesktop.org/download/) and Gstreamer's good plugins

Please note, that these libraries are common enough for you to be able to
install them through your operating system package manager.

### Building from source

To build Hedgehog from source you will need a rust compiler, cargo package
manager. You should also have all dependencies installed before compiling
Hedgehog. Otherwise, there is no additional configuration required.

This repository contains a makefile with building and installation targets. It
will install Hedgehog binary, default configuration files and themes, and a 
manpage into standard directories (`/usr/bin`, `/usr/share/`, etc.)

```bash
$ git clone https://github.com/poletaevvlad/Hedgehog.git
$ cd Hedgehog   
$ make
$ sudo make install
```

### Using pre-compiled binaries

You may wish to avoid installing all necessary build tools and libraries needed
to compile Hedgehog from source. In such cases you may use one of the
pre-compiled packages from "Releases" section. Make sure to choose the correct
package for your operating system and processor architecture.

For Linux, the pre-compiled archives contain an executable `install.sh` script
intended to install binaries, configuration, and documentation into the sytem
directory. You can change the installation destination by setting the `PREFIX`
environment variable. Installing Hedgehog this way also creates a script 
`/usr/share/hedgehog/uninstall.sh` which removes all installed files but keeps
user-specific data and configuration. It requires the same `PREFIX` as was
passed to `install.sh`.


## Bugs and contribution

If you found a bug or have a suggestion for a feature you'd like to see
implemented, don't hesitate to open an issue on GitHub at
<https://github.com/poletaevvlad/Hedgehog/issues>.

You are also invited to participate in the development of Hedgehog if you want
to make a small improvement to the code or documentation, by making a
contrinution in the form of a pull request into this repository. If you wish to
work on the more substaintial change, you are also welcomed to do so, but
filing an isssue first would be best.

Hedgehog is an open source software and its source code is published under
Apache License 2.0. All contibutions are assumed to made under this license.
