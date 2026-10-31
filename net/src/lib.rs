//! CM Network: a stand-in wsock32.dll for CM 01/02 3.9.68 network games over the internet,
//! separate from cmui. The game's only network library is wsock32 (plain TCP to the host on
//! port 6667, one connection per player); this DLL passes everything on to Windows (see
//! wsock32.def) and changes three things:
//! - bind on 6667: all addresses (the game binds the one from ~net.out, a file it writes with
//!   ipconfig: a stale or locked one breaks hosting), so Hamachi, LAN and internet work at once;
//! - connect on 6667: when the typed host is a name ("me.duckdns.org"), it is looked up;
//! - listen on 6667 (hosting): TCP 6667 is mapped on the router by UPnP, and the public
//!   address written to cmnet.log.
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::OnceLock;

const GAME_PORT: u16 = 6667;
/// The game's host address text (sscanf'd "%d.%d.%d.%d" before bind and connect): the client's
/// typed host, the host's own address from ~net.out.
const HOST_TEXT: usize = 0xa8c6fc;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryA(name: *const u8) -> usize;
    fn GetProcAddress(module: usize, name: *const u8) -> usize;
    fn GetSystemDirectoryA(buf: *mut u8, size: u32) -> u32;
    fn VirtualQuery(addr: usize, info: *mut [usize; 7], size: usize) -> usize;
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: usize, flags: u32) -> i32;
    fn CoCreateInstance(clsid: &Guid, outer: usize, context: u32, iid: &Guid, out: *mut usize) -> i32;
}

#[link(name = "oleaut32")]
extern "system" {
    fn SysAllocString(s: *const u16) -> usize;
    fn SysFreeString(s: usize);
}

/// cmnet.log in the game folder, new each start.
fn log(line: &str) {
    static FILE: OnceLock<std::sync::Mutex<std::fs::File>> = OnceLock::new();
    if let Some(f) = FILE.get_or_init(|| std::sync::Mutex::new(std::fs::File::create("cmnet.log").expect("cmnet.log"))).lock().ok().as_deref_mut() {
        let _ = writeln!(f, "{line}");
    }
}

/// Windows' own wsock32.dll (its four functions of its own) and ws2_32.dll (the rest).
fn wsock() -> usize {
    static M: OnceLock<usize> = OnceLock::new();
    *M.get_or_init(|| unsafe {
        let mut dir = [0u8; 260];
        let n = GetSystemDirectoryA(dir.as_mut_ptr(), 260) as usize;
        let path = format!("{}\\wsock32.dll\0", String::from_utf8_lossy(&dir[..n]));
        LoadLibraryA(path.as_ptr())
    })
}

fn ws2() -> usize {
    static M: OnceLock<usize> = OnceLock::new();
    *M.get_or_init(|| unsafe { LoadLibraryA(b"ws2_32.dll\0".as_ptr()) })
}

/// A function of a module, looked up once.
fn proc_of(slot: &AtomicUsize, module: fn() -> usize, name: &[u8]) -> usize {
    match slot.load(Ordering::Relaxed) {
        0 => {
            let p = unsafe { GetProcAddress(module(), name.as_ptr()) };
            slot.store(p, Ordering::Relaxed);
            p
        }
        p => p,
    }
}

macro_rules! real {
    ($module:ident, $name:literal, $ty:ty) => {{
        static SLOT: AtomicUsize = AtomicUsize::new(0);
        unsafe { std::mem::transmute::<usize, $ty>(proc_of(&SLOT, $module, concat!($name, "\0").as_bytes())) }
    }};
}

type Socket = usize;

#[repr(C)]
#[derive(Clone, Copy)]
struct SockaddrIn {
    family: u16,
    port: u16, // network byte order
    addr: [u8; 4],
    zero: [u8; 8],
}

/// The AF_INET address a winsock call got, when it is one.
unsafe fn inet(name: *const u8, len: i32) -> Option<SockaddrIn> {
    (!name.is_null() && len >= 16 && *(name as *const u16) == 2).then(|| *(name as *const SockaddrIn))
}

// wsock32's four functions of its own: Windows' wsock32 does them.
/// On a packed connection the bytes that came are unpacked first; what doesn't fit the game's
/// buffer waits for its next recv (the game polls its non-blocking sockets, no select).
#[no_mangle]
pub extern "system" fn cmnet_recv(s: Socket, buf: *mut u8, len: i32, flags: i32) -> i32 {
    let recv = real!(wsock, "recv", extern "system" fn(Socket, *mut u8, i32, i32) -> i32);
    let Ok(mut packed) = PACKED.lock() else { return -1 };
    let Some((_, p)) = packed.iter_mut().find(|p| p.0 == s) else {
        drop(packed);
        let r = recv(s, buf, len, flags);
        if r > 0 {
            timing(s, 0, r as usize, 0, r as usize);
        }
        return r;
    };
    let mut wire = 0;
    if p.ready.is_empty() {
        let mut got = vec![0u8; 65536];
        let r = recv(s, got.as_mut_ptr(), got.len() as i32, flags);
        if r <= 0 {
            return r; // closed, or nothing yet (WSAEWOULDBLOCK) / an error, as the game expects
        }
        wire = r as usize;
        if p.dec.write_all(&got[..wire]).and_then(|_| p.dec.flush()).is_err() {
            log("packing: the stream from the other side doesn't unpack; connection dropped");
            set_last_error(10054); // WSAECONNRESET
            return -1;
        }
        p.ready.append(p.dec.get_mut());
        if p.ready.is_empty() {
            set_last_error(10035); // WSAEWOULDBLOCK: only part of a packed piece so far
            return -1;
        }
    }
    let n = p.ready.len().min(len.max(0) as usize);
    unsafe { std::ptr::copy_nonoverlapping(p.ready.as_ptr(), buf, n) };
    p.ready.drain(..n);
    drop(packed);
    timing(s, 0, n, 0, wire);
    n as i32
}

/// On a packed connection the game's bytes go packed, all of them before returning (the game's
/// own send loop retries a busy socket the same way).
#[no_mangle]
pub extern "system" fn cmnet_send(s: Socket, buf: *const u8, len: i32, flags: i32) -> i32 {
    let send = real!(ws2, "send", extern "system" fn(Socket, *const u8, i32, i32) -> i32);
    let data = if len > 0 && !buf.is_null() { unsafe { std::slice::from_raw_parts(buf, len as usize) } } else { &[][..] };
    let wire = PACKED.lock().ok().and_then(|mut packed| {
        let (_, p) = packed.iter_mut().find(|p| p.0 == s)?;
        p.enc.write_all(data).ok()?;
        // send_packet 0x77ac30 sends a packet's four length bytes one by one: they wait for the
        // packet (each its own block and send: 2.2 x the raw bytes on the wire, night 4 run E)
        if len == 1 && p.head < 4 {
            p.head += 1;
            return Some(Vec::new());
        }
        p.head = 0;
        p.enc.flush().ok()?;
        Some(std::mem::take(p.enc.get_mut()))
    });
    let Some(wire) = wire else {
        let r = send(s, buf, len, flags);
        if r > 0 {
            timing(s, r as usize, 0, r as usize, 0);
        }
        return r;
    };
    let wsa_get_last_error = real!(ws2, "WSAGetLastError", extern "system" fn() -> i32);
    let mut done = 0;
    while done < wire.len() {
        match send(s, wire[done..].as_ptr(), (wire.len() - done) as i32, flags) {
            r if r > 0 => done += r as usize,
            -1 if wsa_get_last_error() == 10035 => wait_writable(s),
            _ => return -1, // (the connection is gone; its error stays for the game)
        }
    }
    timing(s, data.len(), 0, wire.len(), 0);
    len
}

#[repr(C)]
struct FdSet {
    count: u32,
    socket: Socket,
}

/// Until `s` takes more bytes, 20 ms at most. (A sleep of 1 ms lasts a timer tick here, ~15 ms, for
/// every busy socket: a match day's small packets held the host 10 minutes, night 4 run E.)
fn wait_writable(s: Socket) {
    let select = real!(ws2, "select", extern "system" fn(i32, *mut u8, *mut FdSet, *mut u8, *const [i32; 2]) -> i32);
    let mut set = FdSet { count: 1, socket: s };
    select(0, std::ptr::null_mut(), &mut set, std::ptr::null_mut(), &[0, 20_000]);
}

/// A player connecting to the host: no Nagle delay on it, packed when its hello came, and timed
/// when from another machine.
#[no_mangle]
pub extern "system" fn cmnet_accept(s: Socket, addr: *mut u8, addrlen: *mut i32) -> Socket {
    let r = real!(ws2, "accept", extern "system" fn(Socket, *mut u8, *mut i32) -> Socket)(s, addr, addrlen);
    if r != usize::MAX && local_port(s) == Some(GAME_PORT) {
        no_delay(r);
        let peer = peer_address(r).filter(|p| p[0] != 127);
        let hello = peer.is_some_and(said_hello);
        set_packed(r, hello);
        if let Some(peer) = peer {
            log(&format!("player connected from {}{}", ip(peer), if hello { " (packed)" } else { " (unpacked)" }));
            watch(r, &format!("to {}", ip(peer)));
        }
    }
    r
}

fn set_last_error(e: i32) {
    real!(ws2, "WSASetLastError", extern "system" fn(i32))(e);
}

/// Packing: a connection between two copies of this DLL goes through zstd, one stream each way
/// that remembers the last ~2 MB (a screen like an earlier one costs little), flushed at every
/// send. Measured 2026-10-29 on a game day: screens 7x smaller, match text 4x. Whether the other
/// side packs is settled before the game's connection by a hello on PROBE_PORT (client asks,
/// host answers and remembers the address): a copy without it, an older cmnet or none, never
/// answers or asks, and that connection stays as the game made it.
const PROBE_PORT: u16 = 6668;
const HELLO: &[u8; 8] = b"CMNETZ1\n";

struct Packed {
    enc: zstd::stream::write::Encoder<'static, Vec<u8>>,
    dec: zstd::stream::write::Decoder<'static, Vec<u8>>,
    /// unpacked bytes not yet given to the game
    ready: Vec<u8>,
    /// length bytes of the next packet taken so far
    head: u8,
}

static PACKED: std::sync::Mutex<Vec<(Socket, Packed)>> = std::sync::Mutex::new(Vec::new());
/// Players' addresses whose hello came, and when.
static HELLOS: std::sync::Mutex<Vec<([u8; 4], std::time::Instant)>> = std::sync::Mutex::new(Vec::new());

/// A new connection packed or not. (Set at every connect and accept on the game's port: a
/// socket number freed by closesocket comes back from one of them, never with an old stream.)
fn set_packed(s: Socket, on: bool) {
    let Ok(mut packed) = PACKED.lock() else { return };
    packed.retain(|p| p.0 != s);
    if on {
        if let (Ok(enc), Ok(dec)) = (zstd::stream::write::Encoder::new(Vec::new(), 3), zstd::stream::write::Decoder::new(Vec::new())) {
            packed.push((s, Packed { enc, dec, ready: Vec::new(), head: 0 }));
        }
    }
}

/// Host: a hello from `peer` in the last 30 s, used up.
fn said_hello(peer: [u8; 4]) -> bool {
    let Ok(mut hellos) = HELLOS.lock() else { return false };
    hellos.retain(|h| h.1.elapsed().as_secs() < 30);
    let found = hellos.iter().position(|h| h.0 == peer);
    found.map(|i| hellos.remove(i)).is_some()
}

/// Host: hellos answered on PROBE_PORT (one copy per PC gets the port: for a test with two
/// copies on one PC the host is started first).
fn answer_hellos() {
    use std::io::Read;
    let Ok(listener) = std::net::TcpListener::bind(("0.0.0.0", PROBE_PORT)) else {
        log(&format!("packing: TCP {PROBE_PORT} is taken (another copy of the game?): players connect unpacked"));
        return;
    };
    for mut c in listener.incoming().flatten() {
        let _ = c.set_read_timeout(Some(std::time::Duration::from_secs(2)));
        let mut got = [0u8; 8];
        if c.read_exact(&mut got).is_ok() && &got == HELLO {
            if let (Ok(std::net::SocketAddr::V4(peer)), Ok(mut hellos)) = (c.peer_addr(), HELLOS.lock()) {
                hellos.push((peer.ip().octets(), std::time::Instant::now()));
            }
            let _ = c.write_all(HELLO);
        }
    }
}

/// Client: the host at `addr` packs (its PROBE_PORT answered the hello). Refused at once by a
/// host without it; up to 1.5 s when a router drops the port.
fn host_packs(addr: [u8; 4]) -> bool {
    use std::io::Read;
    let timeout = std::time::Duration::from_millis(1500);
    let Ok(mut c) = std::net::TcpStream::connect_timeout(&(addr, PROBE_PORT).into(), timeout) else { return false };
    let _ = c.set_read_timeout(Some(timeout));
    let mut got = [0u8; 8];
    c.write_all(HELLO).is_ok() && c.read_exact(&mut got).is_ok() && &got == HELLO
}

/// Nagle off (TCP_NODELAY): the game's small messages (a click, a short answer) go at once
/// instead of waiting for the other side's acknowledgement, up to ~200 ms each.
fn no_delay(s: Socket) {
    let setsockopt = real!(ws2, "setsockopt", extern "system" fn(Socket, i32, i32, *const u8, i32) -> i32);
    let on: i32 = 1;
    setsockopt(s, 6, 1, &on as *const i32 as *const u8, 4); // IPPROTO_TCP, TCP_NODELAY
}

fn peer_address(s: Socket) -> Option<[u8; 4]> {
    let getpeername = real!(ws2, "getpeername", extern "system" fn(Socket, *mut u8, *mut i32) -> i32);
    let mut a = SockaddrIn { family: 0, port: 0, addr: [0; 4], zero: [0; 8] };
    let mut len = 16;
    (getpeername(s, &mut a as *mut SockaddrIn as *mut u8, &mut len) == 0 && a.family == 2).then_some(a.addr)
}

/// The timing log: per connection to another machine, from one message sent to the next, the
/// game's bytes out and back and when the answer's first and last bytes came (on a client: a
/// click and the new screen; on the host: a screen sent and the next click), with the bytes on
/// the wire when the connection is packed.
#[derive(Default)]
struct Round {
    name: String,
    start: Option<std::time::Instant>,
    sent: usize,
    got: usize,
    wire_out: usize,
    wire_back: usize,
    first: Option<std::time::Duration>,
    last: Option<std::time::Duration>,
}

static ROUNDS: std::sync::Mutex<Vec<(Socket, Round)>> = std::sync::Mutex::new(Vec::new());

fn watch(s: Socket, name: &str) {
    if let Ok(mut rounds) = ROUNDS.lock() {
        rounds.retain(|r| r.0 != s);
        rounds.push((s, Round { name: name.to_string(), ..Default::default() }));
    }
}

fn timing(s: Socket, sent: usize, got: usize, wire_out: usize, wire_back: usize) {
    let Ok(mut rounds) = ROUNDS.lock() else { return };
    let Some((_, r)) = rounds.iter_mut().find(|r| r.0 == s) else { return };
    let now = std::time::Instant::now();
    if sent > 0 {
        // a new message out after an answer came: the last round logged, a new one begun
        if r.got > 0 {
            let ms = |d: Option<std::time::Duration>| d.map_or("-".into(), |d| d.as_millis().to_string());
            let wire = if (r.wire_out, r.wire_back) != (r.sent, r.got) { format!(" | packed on the wire: {} B out, {} B back", r.wire_out, r.wire_back) } else { String::new() };
            log(&format!("{}: {} B out, {} B back, first after {} ms, all after {} ms{wire}", r.name, r.sent, r.got, ms(r.first), ms(r.last)));
            r.start = None;
        }
        if r.start.is_none() {
            (r.start, r.sent, r.got, r.wire_out, r.wire_back, r.first, r.last) = (Some(now), 0, 0, 0, 0, None, None);
        }
        r.sent += sent;
        r.wire_out += wire_out;
    } else if let Some(start) = r.start {
        r.got += got;
        r.wire_back += wire_back;
        if got > 0 {
            r.first.get_or_insert(now - start);
            r.last = Some(now - start);
        }
    }
}

#[no_mangle]
pub extern "system" fn cmnet_recvfrom(s: Socket, buf: *mut u8, len: i32, flags: i32, from: *mut u8, fromlen: *mut i32) -> i32 {
    real!(wsock, "recvfrom", extern "system" fn(Socket, *mut u8, i32, i32, *mut u8, *mut i32) -> i32)(s, buf, len, flags, from, fromlen)
}

#[no_mangle]
pub extern "system" fn cmnet_getsockopt(s: Socket, level: i32, name: i32, value: *mut u8, len: *mut i32) -> i32 {
    real!(wsock, "getsockopt", extern "system" fn(Socket, i32, i32, *mut u8, *mut i32) -> i32)(s, level, name, value, len)
}

/// The game's 15000-byte send and receive buffers (SO_SNDBUF, SO_RCVBUF) not set: a buffer set
/// by the program switches Windows' own tuning off, ~15 KB per round trip, slow for a whole screen
/// over the internet.
#[no_mangle]
pub extern "system" fn cmnet_setsockopt(s: Socket, level: i32, name: i32, value: *const u8, len: i32) -> i32 {
    if level == 0xffff && (name == 0x1001 || name == 0x1002) {
        return 0;
    }
    real!(wsock, "setsockopt", extern "system" fn(Socket, i32, i32, *const u8, i32) -> i32)(s, level, name, value, len)
}

/// The game's port bound on all addresses instead of the one from ~net.out.
#[no_mangle]
pub extern "system" fn cmnet_bind(s: Socket, name: *const u8, len: i32) -> i32 {
    // (the game's first network call, at its start: the address list prepared from here on)
    static PREPARED: AtomicBool = AtomicBool::new(false);
    if !PREPARED.swap(true, Ordering::Relaxed) {
        std::thread::spawn(prepare);
    }
    let bind = real!(ws2, "bind", extern "system" fn(Socket, *const u8, i32) -> i32);
    unsafe {
        // (not 127.x: every copy of the game runs an internal server of its own on
        // 127.0.0.1:6667, which must not take the players' connections)
        // (the address the game asks for first: every copy listens on its ~net.out addresses,
        // client too, and two copies on one PC on "all addresses" took each other's players;
        // only an address this PC hasn't got, a stale one or the public one, becomes all)
        if let Some(mut a) = inet(name, len).filter(|a| u16::from_be(a.port) == GAME_PORT && a.addr != [0; 4] && a.addr[0] != 127) {
            let r = bind(s, name, len);
            let wsa_get_last_error = real!(ws2, "WSAGetLastError", extern "system" fn() -> i32);
            if r == 0 || wsa_get_last_error() != 10049 {
                return r; // (bound, or failed for another reason than WSAEADDRNOTAVAIL)
            }
            log(&format!("bind {}:{GAME_PORT}: not an address of this PC -> all addresses", ip(a.addr)));
            a.addr = [0; 4];
            return bind(s, &a as *const SockaddrIn as *const u8, 16);
        }
    }
    bind(s, name, len)
}

/// A host typed as a name ("me.duckdns.org") looked up; a typed address goes as it is.
#[no_mangle]
pub extern "system" fn cmnet_connect(s: Socket, name: *const u8, len: i32) -> i32 {
    let connect = real!(ws2, "connect", extern "system" fn(Socket, *const u8, i32) -> i32);
    unsafe {
        // (not 127.x: the game's connection to its own internal server; the two text places
        // hold whatever was typed or clicked last, "Restore Saved Game" at that point)
        if inet(name, len).is_some_and(|a| u16::from_be(a.port) == GAME_PORT) {
            no_delay(s);
        }
        if let Some(mut a) = inet(name, len).filter(|a| u16::from_be(a.port) == GAME_PORT && a.addr[0] != 127) {
            // (the join screen's typed host, found by scanning for a typed "localhost": the game
            // keeps it at these two places; the one with letters is a name)
            let typed = TYPED_HOST.iter().map(|&at| text_at(at)).find(|t| t.bytes().any(|c| c.is_ascii_alphabetic())).unwrap_or_else(|| text_at(HOST_TEXT));
            log(&format!("connect to \"{typed}\" ({}:{GAME_PORT})", ip(a.addr)));
            // (not the game's own connection to itself at Network Play)
            if a.addr != [127, 0, 0, 1] && std::path::Path::new("cmnet.scan").exists() {
                scan(b"localhost");
            }
            if typed.bytes().any(|c| c.is_ascii_alphabetic()) {
                match resolve(&typed) {
                    Some(addr) => {
                        log(&format!("  {typed} is {}", ip(addr)));
                        a.addr = addr;
                    }
                    None => log(&format!("  {typed}: no such name")),
                }
            }
            let packs = host_packs(a.addr);
            log(if packs { "  the host packs: connection packed" } else { "  the host doesn't pack (no answer on TCP 6668): connection unpacked" });
            set_packed(s, packs);
            watch(s, &format!("to host {}", ip(a.addr)));
            return connect(s, &a as *const SockaddrIn as *const u8, 16);
        }
    }
    set_packed(s, false);
    connect(s, name, len)
}

/// Hosting (listening on the game's port): the port mapped on the router, once.
#[no_mangle]
pub extern "system" fn cmnet_listen(s: Socket, backlog: i32) -> i32 {
    static MAPPED: AtomicBool = AtomicBool::new(false);
    let r = real!(ws2, "listen", extern "system" fn(Socket, i32) -> i32)(s, backlog);
    if r == 0 && local_port(s) == Some(GAME_PORT) && !MAPPED.swap(true, Ordering::Relaxed) {
        log(&format!("hosting on TCP {GAME_PORT}"));
        std::thread::spawn(|| upnp(true));
        std::thread::spawn(answer_hellos);
    }
    r
}

/// This PC's Hamachi address (Hamachi hands out 25.x.x.x) and the router's public address.
static HAMACHI: OnceLock<[u8; 4]> = OnceLock::new();
static INTERNET: OnceLock<[u8; 4]> = OnceLock::new();

/// At the game's start: the list the host shows as "Server IP Address" made the two addresses
/// players join with, labelled. The game builds it from ~net.out (ipconfig's output, which it
/// writes itself unless the file is read-only) and shows each address through 0x8ae140.
fn prepare() {
    if let Some(h) = local_addresses().into_iter().find(|a| a[0] == 25) {
        let _ = HAMACHI.set(h);
    }
    write_net_out();
    label_addresses();
    if let Some(public) = upnp(false).and_then(|p| parse_ip(&p)) {
        let _ = INTERNET.set(public);
        write_net_out();
    }
}

fn parse_ip(s: &str) -> Option<[u8; 4]> {
    let parts: Vec<u8> = s.trim().split('.').filter_map(|p| p.parse().ok()).collect();
    (parts.len() == 4).then(|| [parts[0], parts[1], parts[2], parts[3]])
}

/// This PC's IPv4 addresses (its name looked up).
fn local_addresses() -> Vec<[u8; 4]> {
    let gethostname = real!(ws2, "gethostname", extern "system" fn(*mut u8, i32) -> i32);
    let gethostbyname = real!(ws2, "gethostbyname", extern "system" fn(*const u8) -> *const Hostent);
    let mut name = [0u8; 256];
    let mut out = Vec::new();
    unsafe {
        if gethostname(name.as_mut_ptr(), 256) != 0 {
            return out;
        }
        let h = gethostbyname(name.as_ptr());
        if h.is_null() || (*h).addrtype != 2 || (*h).addr_list.is_null() {
            return out;
        }
        let mut p = (*h).addr_list;
        while !(*p).is_null() {
            out.push(**p);
            p = p.add(1);
        }
    }
    out
}

/// ~net.out in the old ipconfig layout the game reads: it takes every 4th line with a colon from
/// the 2nd (or every 5th from the 3rd, by Windows version), so lines 2 and 3 are Hamachi's and 6
/// and 8 the internet's; read-only, so the game's own ipconfig run leaves it.
fn write_net_out() {
    let (h, i) = (HAMACHI.get().copied(), INTERNET.get().copied());
    let Some(first) = h.or(i) else { return };
    let second = if h.is_some() { i } else { None };
    let line = |a: Option<[u8; 4]>| match a {
        Some(a) => format!("        IP Address. . . . . . . . . : {}", ip(a)),
        None => "        Subnet Mask . . . . . . . . : 255.255.255.0".to_string(),
    };
    let text = [None, Some(first), Some(first), None, None, second, None, second].map(line).join("\r\n") + "\r\n";
    let path = std::path::Path::new("~net.out");
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        let _ = std::fs::set_permissions(path, perms);
    }
    if std::fs::write(path, text).is_ok() {
        if let Ok(meta) = std::fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_readonly(true);
            let _ = std::fs::set_permissions(path, perms);
        }
        log(&format!("~net.out: HAMACHI {} INTERNET {}", h.map_or("-".into(), ip), i.map_or("-".into(), ip)));
    }
}

/// The game's text buffer (g_text_buffer, 2000 bytes) that the Manager Status screen builds
/// "Server IP Address : a / b" in (0x81ffa2), and its width measured right after (0x820073:
/// font_text_width(2, buffer)) before the line goes on screen. Only the finished line is
/// labelled: the address texts themselves (0x8ae140, 20-byte slots) also feed the network logic.
const TEXT_BUFFER: usize = 0xde1f64;
const WIDTH_CALL: usize = 0x820073;
const FONT_TEXT_WIDTH: usize = 0x5e5ad0;

fn label_addresses() {
    #[link(name = "kernel32")]
    extern "system" {
        fn VirtualProtect(addr: *mut u8, size: usize, new: u32, old: *mut u32) -> i32;
    }
    unsafe {
        let code = WIDTH_CALL as *mut u8;
        let rel = |to: usize| (to as i64 - (WIDTH_CALL as i64 + 5)) as i32;
        if *code != 0xe8 || (code.add(1) as *const i32).read_unaligned() != rel(FONT_TEXT_WIDTH) {
            return log("address labels: not the 3.9.68 code at 0x820073, left alone");
        }
        let mut old = 0;
        VirtualProtect(code, 5, 0x40, &mut old);
        (code.add(1) as *mut i32).write_unaligned(rel(labelled_width as *const () as usize));
        VirtualProtect(code, 5, old, &mut old);
    }
}

/// font_text_width(font, text) as the game calls it for the "Server IP Address" line, the
/// Hamachi and internet addresses in it labelled first.
extern "C" fn labelled_width(font: i32, text: *mut u8) -> i32 {
    if text as usize == TEXT_BUFFER {
        unsafe {
            let line = std::ffi::CStr::from_ptr(text as *const i8).to_string_lossy().into_owned();
            if let Some((head, list)) = line.split_once(" : ") {
                let label = |s: &str| match parse_ip(s) {
                    Some(a) if HAMACHI.get() == Some(&a) => format!("HAMACHI {s}"),
                    Some(a) if INTERNET.get() == Some(&a) => format!("INTERNET {s}"),
                    _ => s.to_string(),
                };
                let new = format!("{head} : {}\0", list.split(" / ").map(label).collect::<Vec<_>>().join(" / "));
                if new.len() < 2000 {
                    std::ptr::copy_nonoverlapping(new.as_ptr(), text, new.len());
                }
            }
        }
    }
    unsafe { std::mem::transmute::<usize, extern "C" fn(i32, *mut u8) -> i32>(FONT_TEXT_WIDTH)(font, text) }
}

/// A diagnostic (with a cmnet.scan file in the game folder): where in this process's memory a
/// text is, to find where the game keeps the host typed on the join screen.
fn scan(needle: &[u8]) {
    let mut at = 0x10000usize;
    let mut hits = 0;
    while at < 0x7fff_0000 && hits < 40 {
        let mut info = [0usize; 7]; // base, alloc base, alloc protect, size, state, protect, type
        if unsafe { VirtualQuery(at, &mut info, 28) } == 0 || info[3] == 0 {
            break;
        }
        // (committed, readable, not a guard page, not uncached or write-combined graphics memory,
        // which reads at a crawl; private memory and the exe's own data only)
        let readable = info[4] == 0x1000 && info[5] & 0x66 != 0 && info[5] & 0x700 == 0;
        let ours = info[6] == 0x20000 || (info[0] >= 0x400000 && info[0] < 0x1000000);
        if readable && ours && info[3] < 0x1000_0000 {
            let region = unsafe { std::slice::from_raw_parts(info[0] as *const u8, info[3]) };
            for (k, w) in region.windows(needle.len()).enumerate() {
                if w == needle {
                    let start = info[0] + k;
                    let around = &region[k.saturating_sub(16)..(k + 48).min(region.len())];
                    log(&format!("  \"{}\" at {start:#x} (region {:#x} type {:#x}): {}", String::from_utf8_lossy(needle), info[0], info[6], String::from_utf8_lossy(around).replace('\0', "|")));
                    hits += 1;
                }
            }
        }
        at = info[0] + info[3];
    }
    log(&format!("  scan done, {hits} found"));
}

fn ip(a: [u8; 4]) -> String {
    format!("{}.{}.{}.{}", a[0], a[1], a[2], a[3])
}

/// The host typed on the join screen: the game keeps it at both (0xdd70dc, 0xc33776).
const TYPED_HOST: [usize; 2] = [0xdd70dc, 0xc33776];

/// A text of the game's (empty when not readable).
fn text_at(at: usize) -> String {
    let mut info = [0usize; 7];
    unsafe {
        if VirtualQuery(at, &mut info, 28) == 0 || info[4] != 0x1000 || at + 64 > info[0] + info[3] {
            return String::new();
        }
        let bytes = std::slice::from_raw_parts(at as *const u8, 64);
        String::from_utf8_lossy(bytes.split(|&c| c == 0).next().unwrap_or(&[])).trim().to_string()
    }
}

#[repr(C)]
struct Hostent {
    name: *const u8,
    aliases: *const *const u8,
    addrtype: i16,
    length: i16,
    addr_list: *const *const [u8; 4],
}

fn resolve(host: &str) -> Option<[u8; 4]> {
    let gethostbyname = real!(ws2, "gethostbyname", extern "system" fn(*const u8) -> *const Hostent);
    let name = std::ffi::CString::new(host).ok()?;
    unsafe {
        let h = gethostbyname(name.as_ptr() as *const u8);
        if h.is_null() || (*h).addrtype != 2 || (*h).addr_list.is_null() || (*(*h).addr_list).is_null() {
            return None;
        }
        Some(**(*h).addr_list)
    }
}

fn local_port(s: Socket) -> Option<u16> {
    let getsockname = real!(ws2, "getsockname", extern "system" fn(Socket, *mut u8, *mut i32) -> i32);
    let mut a = SockaddrIn { family: 0, port: 0, addr: [0; 4], zero: [0; 8] };
    let mut len = 16;
    (getsockname(s, &mut a as *mut SockaddrIn as *mut u8, &mut len) == 0 && a.family == 2).then(|| u16::from_be(a.port))
}

/// This PC's address towards the router (the route a packet to the internet would take; a UDP
/// connect sends nothing).
fn lan_address() -> Option<[u8; 4]> {
    let socket = real!(ws2, "socket", extern "system" fn(i32, i32, i32) -> Socket);
    let connect = real!(ws2, "connect", extern "system" fn(Socket, *const u8, i32) -> i32);
    let getsockname = real!(ws2, "getsockname", extern "system" fn(Socket, *mut u8, *mut i32) -> i32);
    let closesocket = real!(ws2, "closesocket", extern "system" fn(Socket) -> i32);
    let s = socket(2, 2, 17);
    if s == usize::MAX {
        return None;
    }
    let to = SockaddrIn { family: 2, port: 53u16.to_be(), addr: [198, 51, 100, 1], zero: [0; 8] };
    let mut me = SockaddrIn { family: 0, port: 0, addr: [0; 4], zero: [0; 8] };
    let mut len = 16;
    let ok = connect(s, &to as *const SockaddrIn as *const u8, 16) == 0
        && getsockname(s, &mut me as *mut SockaddrIn as *mut u8, &mut len) == 0;
    closesocket(s);
    (ok && me.addr != [0; 4]).then_some(me.addr)
}

#[repr(C)]
struct Guid(u32, u16, u16, [u8; 8]);

const CLSID_UPNPNAT: Guid = Guid(0xae1e00aa, 0x3fd5, 0x403c, [0x8a, 0x27, 0x2b, 0xbd, 0xc3, 0x0c, 0xd0, 0xe1]);
const IID_IUPNPNAT: Guid = Guid(0xb171c812, 0xcc76, 0x485a, [0x94, 0xd8, 0xb6, 0xb3, 0xa2, 0x79, 0x4e, 0x99]);

/// A COM object's method `index` (IDispatch's seven first).
unsafe fn method(obj: usize, index: usize) -> usize {
    *(*(obj as *const *const usize)).add(index)
}

unsafe fn release(obj: usize) {
    if obj != 0 {
        std::mem::transmute::<usize, extern "system" fn(usize) -> u32>(method(obj, 2))(obj);
    }
}

fn bstr(s: &str) -> usize {
    let wide: Vec<u16> = s.encode_utf16().chain([0]).collect();
    unsafe { SysAllocString(wide.as_ptr()) }
}

unsafe fn from_bstr(b: usize) -> String {
    if b == 0 {
        return String::new();
    }
    let mut n = 0;
    while *(b as *const u16).add(n) != 0 {
        n += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(b as *const u16, n))
}

/// TCP 6667 mapped on the router by Windows' UPnP NAT (IUPnPNAT, IStaticPortMappingCollection),
/// the router's public address logged and returned. `keep` false: only to learn the public
/// address, a mapping made here removed again (one already there stays). Without UPnP the port
/// has to be forwarded by hand.
fn upnp(keep: bool) -> Option<String> {
    let Some(lan) = lan_address() else {
        log("UPnP: no address towards the router");
        return None;
    };
    let mut result = None;
    unsafe {
        CoInitializeEx(0, 0);
        let mut nat = 0;
        if CoCreateInstance(&CLSID_UPNPNAT, 0, 1 | 4, &IID_IUPNPNAT, &mut nat) < 0 || nat == 0 {
            log("UPnP: Windows' UPnP NAT is not available");
            return None;
        }
        // (the collection is null until Windows has found the router: asked a few times)
        let get_collection = std::mem::transmute::<usize, extern "system" fn(usize, *mut usize) -> i32>(method(nat, 7));
        let mut maps = 0;
        for _ in 0..10 {
            if get_collection(nat, &mut maps) >= 0 && maps != 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        if maps == 0 {
            release(nat);
            log(&format!("UPnP: no router answered; forward TCP {GAME_PORT} to {} by hand", ip(lan)));
            return None;
        }
        let (proto, client, desc) = (bstr("TCP"), bstr(&ip(lan)), bstr("CM 01/02 network game"));
        // (mapped already, by an earlier start or by hand: that one is used and kept)
        let item = std::mem::transmute::<usize, extern "system" fn(usize, i32, usize, *mut usize) -> i32>(method(maps, 8));
        let add = std::mem::transmute::<usize, extern "system" fn(usize, i32, usize, i32, usize, i16, usize, *mut usize) -> i32>(method(maps, 11));
        let mut map = 0;
        item(maps, GAME_PORT as i32, proto, &mut map);
        let existed = map != 0;
        let mut hr = 0;
        if !existed {
            hr = add(maps, GAME_PORT as i32, proto, GAME_PORT as i32, client, -1, desc, &mut map);
        }
        // (the packing hello's port beside it; without it players join unpacked)
        if keep {
            let mut probe = 0;
            item(maps, PROBE_PORT as i32, proto, &mut probe);
            if probe == 0 {
                add(maps, PROBE_PORT as i32, proto, PROBE_PORT as i32, client, -1, desc, &mut probe);
            }
            if probe != 0 {
                release(probe);
            } else {
                log(&format!("UPnP: TCP {PROBE_PORT} not mapped: players over the internet join unpacked"));
            }
        }
        if map == 0 {
            log(&format!("UPnP: the router refused TCP {GAME_PORT} ({hr:#x}); forward it to {} by hand", ip(lan)));
        } else {
            let mut public = 0;
            let get_ip = std::mem::transmute::<usize, extern "system" fn(usize, *mut usize) -> i32>(method(map, 7));
            get_ip(map, &mut public);
            let address = from_bstr(public);
            SysFreeString(public);
            release(map);
            if keep || existed {
                log(&format!("UPnP: TCP {GAME_PORT} -> {}; others join with {address}", ip(lan)));
            } else {
                let remove = std::mem::transmute::<usize, extern "system" fn(usize, i32, usize) -> i32>(method(maps, 10));
                remove(maps, GAME_PORT as i32, proto);
                log(&format!("UPnP: public address {address}"));
            }
            result = Some(address);
        }
        SysFreeString(proto);
        SysFreeString(client);
        SysFreeString(desc);
        release(maps);
        release(nat);
    }
    result
}
