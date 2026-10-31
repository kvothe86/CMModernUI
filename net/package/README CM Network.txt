CM Network for Championship Manager 01/02 (3.9.68) - test version
=================================================================

Network games over the internet, not only Hamachi or LAN. One file, wsock32.dll, that sits
between the game and Windows' network. It does not change your exe and works next to cmui,
cnc-ddraw and cmfix.

What it does
- The host listens on all its addresses: Hamachi, LAN and internet at the same time. A wrong
  or locked ~net.out no longer stops hosting.
- The host's "Server IP Address" line shows the two addresses to join with:
      HAMACHI 25.x.x.x / INTERNET x.x.x.x
- When you host, it asks your router (UPnP) to open TCP ports 6667 and 6668 to your PC.
- In the join screen you can type a name (for example yourname.duckdns.org) instead of an IP.
- Faster on the internet: no Nagle delay on clicks, and Windows tunes the network buffers
  instead of the game's small 15 KB ones.
- Compressed (since 2026-10-29): when host and player both have this version, everything the
  game sends between them is packed, about 7 times smaller for screens and 4 times for match
  text. With an older version or none on either side the game simply runs uncompressed.
  The host and player agree on it over TCP port 6668 (see Hosting).
- cmnet.log in the game folder: what it did, and timing lines per click (see below).

Install (EVERYONE: the host and every player who joins)
1. Close the game.
2. Copy wsock32.dll into the game folder (the folder with cm0102.exe / cm0102_ui.exe).
3. Start the game as usual. If Windows Firewall asks about the game, allow it.

Hosting
- Host as usual. On the Manager Status screen the Server IP Address line shows the addresses.
  Give your friends the INTERNET one (or the HAMACHI one if you all use Hamachi).
- No INTERNET address in the line: your router doesn't do UPnP. Look in cmnet.log: it says
  which address to forward TCP ports 6667 and 6668 to; do that once in your router (with
  only 6667 forwarded the game works, uncompressed).
- cmnet.log says per player "player connected from x (packed)" or "(unpacked)".
- Still no luck from outside: your internet provider may not give you a real public address
  (CGNAT). Then Hamachi stays the way.

Joining
- Type the host's INTERNET (or HAMACHI) address, or a name, as usual.

The timing log (for the test)
Each line in cmnet.log like
    to host 203.0.113.7: 62 B out, 13879 B back, first after 45 ms, all after 61 ms
is one click and the screen that came back: "first after" is how long until the answer
started, "all after" until all of it was in. On a compressed connection the line ends with
    | packed on the wire: 230 B out, 3900 B back
what really went over the network. Play a little over Hamachi and a little over the
INTERNET address, and keep both cmnet.log files (they are made new at every start: copy the
file before you start again).

Good to know
- CM Network writes ~net.out itself and makes it read-only (the game uses that file for its
  address list).
- Testing with two copies of the game on one PC: start the host's copy first (only one copy
  per PC can answer on port 6668).
- The router keeps the port 6667 / 6668 mappings ("CM 01/02 network game") after the game closes;
  remove it in the router's UPnP or port forwarding page if you want.

Remove: delete wsock32.dll from the game folder (and ~net.out if you like; the game makes a
new one).

Unofficial fan mod, made with the cmui project. Separate from cmui.
