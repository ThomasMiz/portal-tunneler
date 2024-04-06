# portal-tunneler

Create SSH-like TCP tunnels over a hole-punched QUIC connection.

Portal peers can run in two modes:
- Client: Connects to the server and requests tunnels
- Server: Listens for incoming connections and overall just does what the client asks

And separately, portal peers can connect to each other in two modes:
- Direct: A typical connection. The server listens for incoming connections and the client talks to it
- Hole-punched: Both the client and the server create "connection codes" and ask the users to exchange them. Once each side is given the other side's connection code, [hole-punching](https://en.wikipedia.org/wiki/Hole_punching_(networking)) is used to establish a direct connection.

Tunnels are specified through arguments in much the same way as in SSH, so look at `-L`, `-R` and `-D` options in the [SSH manual](https://www.man7.org/linux/man-pages/man1/ssh.1.html) for more information. The only difference is that local dynamic tunnels that in SSH are specified with `-D`, in portal they are specified with `-L` instead for simplicity.

# THIS PROJECT IS IN BETA!

While the project has most of the main features implemented, some things are still missing (most notably is the help menu, which simply prints "I need somebody") and the project needs some cleanup and refactoring to pretty it up before release. It is also covered in unnecessary verbose prints.

Additionally, this beta version makes no guarantees on stability, security nor compatibility with other versions.

# Installation
The recommended way to install is with `cargo` from crates.io:
```
cargo install portal-tunneler
```

Or directly from GitHub:
```
cargo install --git https://github.com/ThomasMiz/portal-tunneler.git portal-tunneler
```

Either one of these will download and compile portal's code and all its dependencies. Once this is done, the portal executable will become available under the name `portal`.

## Downloading binaries
If you don't have `cargo` installed, pre-compiled binaries are available for x84_64 Windows and Linux [in the releases page](https://github.com/ThomasMiz/portal-tunneler/releases).

# Usage examples

Given that there's no help menu yet, here are some usage examples. Running portal with no arguments will start a server listening on 0.0.0.0:5995 and [::]:5995.

Start a server running on 192.168.1.100:5995 (the port is implicit, given that 5995 is the default port for portal):
```sh
portal --listen 192.168.1.100
```

Start a client that connects to the server at 192.168.1.100:5995 and opens a local tunnel that listens locally on port 4444 and tunnels connections to the server towards localhost:5555:
```sh
portal --connect 192.168.1.100 -L4444:localhost:5555
```

Same as before, but instead of pointing the tunnel towards localhost:5555, it's a dynamic tunnel that uses SOCKS4/SOCKS5 for proxying:
```sh
portal --connect 192.168.1.100 -L5555
```

If instead of a direct connection you want a hole-punched connection, instead of `--connect` or `--listen` you should use `--punch`:

The server runs:
```sh
portal --punch
```

The client runs:
```sh
portal --punch -L4444:localhost:5555
```

Now we're getting to the relevant part. If you want to play Minecraft, then whoever is hosting the server should run:
```sh
portal --punch
```

And the client should run:
```sh
portal --punch -L25565:localhost:25565
```

Once the connection is established, you should tell the Minecraft client to connect to localhost:25565.
